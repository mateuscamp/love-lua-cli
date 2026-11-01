//! `jogar <game>` on a game that declares itself unplayable (a `NAO_JOGAVEL` file, author, 01/11):
//! it warns on stderr with the reason and still opens the game, because the program shows and does
//! not forbid. Audit of 01/11, finding A1: the warning had no test.

#[path = "../src/test_support.rs"]
mod test_support;

use std::process::Command;

use test_support::{fake_game, test_folder};

/// Runs `jogar <folder>` in a fake root; returns the exit code, stderr and what the game wrote.
fn run_directly(folder: &str, marker: Option<&str>) -> (i32, String, String) {
    let root = test_folder(&format!("unplayable-warning-{folder}"));
    fake_game(&root, folder, "echo opened > opened\n");
    if let Some(text) = marker {
        std::fs::write(root.join(folder).join("NAO_JOGAVEL"), text).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_jogar"))
        .arg(folder)
        .env("JOGAR_RAIZ", &root)
        // A remote display: jogar does not check it, and the fake game needs no X.
        .env("DISPLAY", "localhost:10.0")
        .output()
        .unwrap();
    let opened = std::fs::read_to_string(root.join(folder).join("opened")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.code().unwrap(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        opened,
    )
}

#[test]
fn the_direct_mode_warns_with_the_reason_and_still_opens_the_game() {
    let (code, stderr, opened) = run_directly("gods", Some("só o domínio, sem telas\n"));
    assert!(
        stderr.contains("gods ainda não é jogável: só o domínio, sem telas"),
        "{stderr}"
    );
    assert_eq!((code, opened.trim()), (0, "opened"), "{stderr}");
}

#[test]
fn the_direct_mode_does_not_warn_about_a_playable_game() {
    let (code, stderr, opened) = run_directly("pronto", None);
    assert!(!stderr.contains("jogável"), "{stderr}");
    assert_eq!((code, opened.trim()), (0, "opened"), "{stderr}");
}
