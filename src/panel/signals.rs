//! SIGHUP (the terminal that closes) and SIGTERM in the panel. The handler closes the open game
//! itself, without depending on the loop: with the terminal closed, the tty returns 0 on `read`,
//! crossterm's inner read loop only leaves on `WouldBlock` and spins idly, and the panel's loop
//! never comes back (re-audit of 28/10). On SIGHUP jogar exits right there, because there is no
//! terminal to restore; on SIGTERM, with the terminal alive, the loop sees the recorded signal and
//! leaves the normal way.

use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};

/// The signal (SIGTERM) that asked the panel to quit; 0 while none has come.
static EXIT_SIGNAL: AtomicI32 = AtomicI32::new(0);

/// The open game's process group; 0 with no game. It is the launcher's pid (`games::spawn_piped`).
static GAME_GROUP: AtomicI32 = AtomicI32::new(0);

/// The group of the game that just started, or 0 when it closes and is reaped.
pub(super) fn set_game_group(group: i32) {
    GAME_GROUP.store(group, Ordering::SeqCst);
}

/// Only calls allowed inside a signal handler: atomics, `killpg` and `_exit`.
extern "C" fn on_exit_signal(signal: libc::c_int) {
    let group = GAME_GROUP.load(Ordering::SeqCst);
    // SAFETY: killpg and _exit are async-signal-safe; the group is the one of the game the panel
    // started.
    unsafe {
        if group > 0 {
            libc::killpg(group, libc::SIGTERM);
        }
        if signal == libc::SIGHUP {
            libc::_exit(128 + signal);
        }
    }
    EXIT_SIGNAL.store(signal, Ordering::SeqCst);
}

/// Installs the handler on SIGHUP and SIGTERM. Without it jogar died on the spot and the game, in
/// its own process group, was left orphaned with its window open (audit of 28/10, A1).
pub(super) fn install_exit_signals() {
    for signal in [libc::SIGHUP, libc::SIGTERM] {
        // SAFETY: the handler only uses async-signal-safe calls; the struct is zeroed and filled in
        // before the call.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction =
                on_exit_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
    }
}

/// jogar's exit if a signal asked for it: 128 + signal, as the shell would give it.
pub(super) fn exit_by_signal() -> Option<ExitCode> {
    let signal = EXIT_SIGNAL.load(Ordering::SeqCst);
    (signal != 0).then(|| ExitCode::from(128 + signal as u8))
}
