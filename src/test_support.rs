//! Test helpers: only disposable things (a temporary folder, a temporary git repository, a fake
//! game), never `~/lua`, `~/.cargo` or the author's display. The integration test in `tests/`
//! includes this same file through `#[path]`, because the crate is binary-only.

#![allow(dead_code)] // each test crate uses a part

use std::env;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

/// Runs git in `folder` without the user's configuration (signature, hooks), so the test does not
/// depend on it.
pub fn test_git(folder: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(folder)
        .args(["-c", "user.name=test", "-c", "user.email=test@jogar"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

/// An empty folder for this test only (`name` tells apart the tests that run in parallel).
pub fn test_folder(name: &str) -> PathBuf {
    let folder = env::temp_dir().join(format!("jogar-{name}-{}", process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A fake game in `root/folder`, with `tools/love` (the games' shared launcher) made from `body`
/// in sh. It runs in the game's folder, like the real one.
///
/// Another process writes the script. If this one did, a fork from another test running in
/// parallel would inherit the write descriptor, and exec'ing the launcher would fail with "Text
/// file busy" (ETXTBSY: 5 in 40 runs of the suite with 32 threads, 28/10).
pub fn fake_game(root: &Path, folder: &str, body: &str) {
    let tools = root.join(folder).join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    let launcher = tools.join("love");
    let ok = Command::new("sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
            "sh",
        ])
        .arg(&launcher)
        .arg(format!("#!/bin/sh\n{body}"))
        .status()
        .unwrap()
        .success();
    assert!(ok, "did not write {}", launcher.display());
}

/// The pid the fake game wrote to `file`, waiting up to `timeout` for it to appear.
pub fn wait_for_pid(file: &Path, timeout: Duration) -> i32 {
    let end = Instant::now() + timeout;
    loop {
        let read = std::fs::read_to_string(file).unwrap_or_default();
        if let Ok(pid) = read.trim().parse() {
            return pid;
        }
        assert!(Instant::now() < end, "{} did not appear", file.display());
        thread::sleep(Duration::from_millis(20));
    }
}

/// Whether process `pid` is running (a zombie counts as dead: it already exited, it only waits for
/// someone to reap it).
pub fn pid_alive(pid: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit_once(") ")
            .is_some_and(|(_, rest)| rest.starts_with('Z')),
        Err(_) => false,
    }
}

/// Whether `pid` dies within `timeout`.
pub fn dies_within(pid: i32, timeout: Duration) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if !pid_alive(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

/// The fake launcher leaves a grandchild (like a launcher that opened LÖVE as a child, rather than
/// in its own place), writes down its pid and waits.
pub const LAUNCHER_WITH_GRANDCHILD: &str = "sleep 30 &\necho $! > grandchild\nwait\n";
