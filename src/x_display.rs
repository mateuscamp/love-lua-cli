//! The X display before opening. Every game runs on X11 (`SDL_VIDEO_DRIVER=x11`, a README rule),
//! and without X it dies at boot with a stack trace ("x11 not available"); this warns first, in
//! one line.

use std::env;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::path::Path;

/// The local display number in `value` (`:0`, `:1.0`, `unix:0`). `Ok(None)` is a remote display
/// (`localhost:10.0`, the one from `ssh -X`), which cannot be checked from here; `Err` is a value
/// that is not a display.
fn local_display_number(value: &str) -> Result<Option<u32>, ()> {
    let (host, rest) = value.rsplit_once(':').ok_or(())?;
    let number = rest
        .split_once('.')
        .map_or(rest, |(number, _screen)| number);
    let number: u32 = number.parse().map_err(|_| ())?;
    match host {
        "" | "unix" => Ok(Some(number)),
        _ => Ok(None),
    }
}

/// Whether someone answers on the socket at `path`: the file one or, on Linux, the abstract one of
/// the same name. It does not speak the X protocol, it only sees whether the connection is
/// accepted.
fn socket_answers(path: &Path) -> bool {
    UnixStream::connect(path).is_ok()
        || SocketAddr::from_abstract_name(path.as_os_str().as_bytes())
            .and_then(|address| UnixStream::connect_addr(&address))
            .is_ok()
}

/// Checks whether an X server answers on `display`, looking for its socket in `sockets` (on the
/// system, `/tmp/.X11-unix`).
pub fn check_display(display: Option<&str>, sockets: &Path) -> Result<(), String> {
    let value = display.unwrap_or("");
    if value.is_empty() {
        return Err("sem tela X: o DISPLAY está vazio".to_string());
    }
    let number = match local_display_number(value) {
        Ok(Some(number)) => number,
        Ok(None) => return Ok(()),
        Err(()) => return Err(format!("sem tela X: DISPLAY={value:?} não é um display")),
    };
    let socket = sockets.join(format!("X{number}"));
    if socket_answers(&socket) {
        return Ok(());
    }
    // Hyprland's XWayland answers on `X<n>_`; without `X<n>`, no new client connects, and a hard
    // link fixes it (28/10: an `Xvfb -displayfd` took `:0` and deleted `X0`).
    let underscored = sockets.join(format!("X{number}_"));
    if UnixStream::connect(&underscored).is_ok() {
        return Err(format!(
            "sem tela X: o socket {} sumiu, mas o X{number}_ atende; refaça com: ln {} {}",
            socket.display(),
            underscored.display(),
            socket.display()
        ));
    }
    Err(format!("sem tela X: ninguém atende no DISPLAY={value:?}"))
}

/// `check_display` for this process.
pub fn check_this_display() -> Result<(), String> {
    check_display(
        env::var("DISPLAY").ok().as_deref(),
        Path::new("/tmp/.X11-unix"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_folder;
    use std::os::unix::net::UnixListener;

    #[test]
    fn local_display_number_reads_the_number_and_leaves_the_remote_out() {
        assert_eq!(local_display_number(":0"), Ok(Some(0)));
        assert_eq!(local_display_number(":1.0"), Ok(Some(1)));
        assert_eq!(local_display_number("unix:2"), Ok(Some(2)));
        assert_eq!(local_display_number("localhost:10.0"), Ok(None));
        for wrong in ["", ":", "0", ":abc", ":0x"] {
            assert_eq!(local_display_number(wrong), Err(()), "{wrong:?}");
        }
    }

    #[test]
    fn check_display_passes_only_with_someone_answering_on_the_socket() {
        let sockets = test_folder("x-display");
        let _x5 = UnixListener::bind(sockets.join("X5")).unwrap();

        assert_eq!(check_display(Some(":5"), &sockets), Ok(()));
        assert_eq!(check_display(Some("localhost:10.0"), &sockets), Ok(()));
        let nobody = check_display(Some(":6"), &sockets).unwrap_err();
        assert!(nobody.contains(r#"DISPLAY=":6""#), "{nobody}");
        assert!(check_display(None, &sockets).unwrap_err().contains("vazio"));
        assert!(
            check_display(Some(""), &sockets)
                .unwrap_err()
                .contains("vazio")
        );
        assert!(
            check_display(Some(":abc"), &sockets)
                .unwrap_err()
                .contains("não é um display")
        );

        std::fs::remove_dir_all(&sockets).unwrap();
    }

    /// Hyprland's XWayland answers only on `X0_`; when the test tools' `Xvfb -displayfd` takes
    /// `:0`, it deletes `X0`, and no new X client connects (28/10, 19:02; the author's fix was the
    /// hard link). The warning gives the command.
    #[test]
    fn check_display_without_x_but_with_x_underscore_gives_the_fix() {
        let sockets = test_folder("x-display-underscore");
        let _x7 = UnixListener::bind(sockets.join("X7_")).unwrap();
        let x7 = sockets.join("X7");
        let x7_ = sockets.join("X7_");

        let warning = check_display(Some(":7"), &sockets).unwrap_err();
        let fix = format!("ln {} {}", x7_.display(), x7.display());
        assert!(warning.contains(&fix), "{warning}");
        assert!(warning.contains("sumiu"), "{warning}");

        std::fs::hard_link(&x7_, &x7).unwrap();
        assert_eq!(check_display(Some(":7"), &sockets), Ok(()), "the fix works");

        std::fs::remove_dir_all(&sockets).unwrap();
    }
}
