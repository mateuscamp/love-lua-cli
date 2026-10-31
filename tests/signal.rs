//! The whole jogar, the binary, in a pseudo-terminal (`script`, from util-linux), with a fake
//! `~/lua`: Enter opens the fake game through the panel, and a jogar that exits by a signal or
//! because the terminal closed has to take the game with it. Audit of 28/10, finding A1: the game
//! starts in its own process group and, without this, was left orphaned with its window open.

#[path = "../src/test_support.rs"]
mod test_support;

use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::Duration;

use test_support::{dies_within, fake_game, test_folder, wait_for_pid};

/// The panel open in a terminal, with the fake game opened by Enter.
struct OpenPanel {
    terminal: Child,
    /// Holds `script`'s stdin: without it, the terminal would close too early.
    _keys: ChildStdin,
    jogar: i32,
    grandchild: i32,
}

/// Opens the panel with `HOME=home` and presses Enter. The launcher writes down who opened it
/// (jogar) and a grandchild, like a launcher that opened LÖVE as a child.
fn open_through_the_panel(home: &Path) -> OpenPanel {
    fake_game(
        &home.join("lua"),
        "bomberboom",
        "echo $PPID > jogar\nsleep 30 &\necho $! > grandchild\nwait\n",
    );
    let mut terminal = Command::new("script")
        .args(["-qfec", env!("CARGO_BIN_EXE_jogar"), "/dev/null"])
        .env("HOME", home)
        // The `$JOGAR_RAIZ` of whoever runs the suite would take the panel out of the fake home.
        .env_remove("JOGAR_RAIZ")
        // A remote display: jogar does not check it, and the fake game needs no X.
        .env("DISPLAY", "localhost:10.0")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the test needs script, from util-linux");
    let mut keys = terminal.stdin.take().unwrap();
    thread::sleep(Duration::from_millis(800));
    keys.write_all(b"\r").unwrap();
    let folder = home.join("lua/bomberboom");
    OpenPanel {
        jogar: wait_for_pid(&folder.join("jogar"), Duration::from_secs(5)),
        grandchild: wait_for_pid(&folder.join("grandchild"), Duration::from_secs(5)),
        terminal,
        _keys: keys,
    }
}

/// Kills what is left and deletes the home; returns whether the grandchild and jogar had died
/// before that.
fn close(mut panel: OpenPanel, home: &Path) -> (bool, bool) {
    let grandchild_died = dies_within(panel.grandchild, Duration::from_secs(3));
    let jogar_exited = dies_within(panel.jogar, Duration::from_secs(3));
    // SAFETY: the signals go only to the processes this test created.
    unsafe {
        libc::kill(panel.grandchild, libc::SIGKILL);
        libc::kill(panel.jogar, libc::SIGKILL);
    }
    let _ = panel.terminal.kill();
    let _ = panel.terminal.wait();
    std::fs::remove_dir_all(home).unwrap();
    (grandchild_died, jogar_exited)
}

#[test]
fn sighup_on_jogar_closes_the_game_opened_by_the_panel() {
    let home = test_folder("sighup");
    let panel = open_through_the_panel(&home);
    // SAFETY: the signal goes only to the jogar this test opened.
    unsafe { libc::kill(panel.jogar, libc::SIGHUP) };
    let (grandchild, _) = close(panel, &home);
    assert!(grandchild, "the game survived SIGHUP on jogar");
}

/// The terminal that really closes (the pseudo-terminal's owner dies). Unlike a direct SIGHUP: the
/// tty stays dead, its `read` returns 0, and crossterm spins idly in there without giving the loop
/// back; the signal handler is the one that has to close the game and exit (re-audit of 28/10).
#[test]
fn a_closing_terminal_takes_the_game_and_jogar_with_it() {
    let home = test_folder("terminal-closes");
    let mut panel = open_through_the_panel(&home);
    panel.terminal.kill().unwrap();
    let (grandchild, jogar) = close(panel, &home);
    assert!(grandchild, "the game survived the closed terminal");
    assert!(jogar, "jogar kept running with the terminal closed");
}

#[test]
fn sigterm_on_jogar_closes_the_game_and_jogar_exits() {
    let home = test_folder("sigterm");
    let panel = open_through_the_panel(&home);
    // SAFETY: the signal goes only to the jogar this test opened.
    unsafe { libc::kill(panel.jogar, libc::SIGTERM) };
    let (grandchild, jogar) = close(panel, &home);
    assert!(grandchild, "the game survived SIGTERM on jogar");
    assert!(jogar, "jogar did not exit with SIGTERM");
}
