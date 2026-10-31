//! The running game, on the process side: the launcher in its own group, both pipes read in
//! threads, the signal to the whole group (`x`, Ctrl+C and the `Drop`) and the ending notice. Talks
//! to processes, never to the terminal.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::list_screen::Notice;
use super::live_screen::LiveView;
use super::{formatting, signals};
use crate::facts::{Scoreboard, Source};
use crate::games::{self, Game};

/// How long Ctrl+C waits for the game to close after SIGTERM, before SIGKILL.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// A line from the game, which pipe it came from and when it arrived.
type Arrival = (Source, String, Instant);

/// The running game, with the scoreboard of what it writes.
pub(super) struct RunningGame {
    game: Game,
    child: Child,
    arrivals: Receiver<Arrival>,
    started: Instant,
    pub(super) scoreboard: Scoreboard,
    pub(super) scroll: usize,
    /// The author asked to close it (`x`): the SIGTERM that follows is not the game's error.
    stopped: bool,
}

impl RunningGame {
    pub(super) fn start(root: &Path, game: &Game) -> Result<RunningGame, String> {
        let mut child = games::spawn_piped(root, game)?;
        let (sender, arrivals) = mpsc::channel();
        let stdout = child
            .stdout
            .take()
            .expect("games::spawn_piped puts stdout in a pipe");
        let stderr = child
            .stderr
            .take()
            .expect("games::spawn_piped puts stderr in a pipe");
        listen(stdout, Source::Stdout, sender.clone());
        listen(stderr, Source::Stderr, sender);
        signals::set_game_group(child.id() as i32);
        Ok(RunningGame {
            game: game.clone(),
            child,
            arrivals,
            started: Instant::now(),
            scoreboard: Scoreboard::new(&game.folder),
            scroll: 0,
            stopped: false,
        })
    }

    /// Hands the scoreboard the lines that arrived since the last time.
    pub(super) fn drain_lines(&mut self) {
        for (source, line, instant) in self.arrivals.try_iter() {
            let at = instant.saturating_duration_since(self.started);
            self.scoreboard.record(source, &line, at);
        }
    }

    /// The ending notice, if the game has already closed.
    pub(super) fn poll_end(&mut self) -> Option<Notice> {
        let status = self.child.try_wait().ok().flatten()?;
        // Once reaped, the pid may go back to another process: the signal handler no longer aims at
        // it.
        signals::set_game_group(0);
        // The pipe threads may still hold the last lines; the boot error is usually among them.
        thread::sleep(Duration::from_millis(50));
        self.drain_lines();
        let ending = Ending {
            code: games::exit_code(status),
            stopped: self.stopped,
            elapsed: self.started.elapsed(),
        };
        Some(ending_notice(&self.game.folder, &ending, &self.scoreboard))
    }

    pub(super) fn scroll_by(&mut self, step: isize) {
        let last = self.scoreboard.kinds().len().saturating_sub(1);
        self.scroll = self.scroll.saturating_add_signed(step).min(last);
    }

    /// Asks the whole game (the launcher's group: the launcher and LÖVE) to close, with SIGTERM,
    /// which LÖVE treats as closing the window.
    pub(super) fn stop(&mut self) {
        self.stopped = true;
        signal_group(&self.child, libc::SIGTERM);
    }

    /// Stops and waits up to `STOP_GRACE`; after that, SIGKILL. It is for quitting jogar without
    /// leaving an orphaned game.
    pub(super) fn stop_and_wait(&mut self) {
        self.stop();
        let deadline = Instant::now() + STOP_GRACE;
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        signal_group(&self.child, libc::SIGKILL);
        let _ = self.child.wait();
    }

    pub(super) fn live_view(&self) -> LiveView<'_> {
        LiveView {
            name: &self.game.folder,
            scoreboard: &self.scoreboard,
            elapsed: self.started.elapsed(),
            scroll: self.scroll,
        }
    }
}

/// How the game closed.
struct Ending {
    /// The launcher's code, 128 + signal if it died by a signal.
    code: u8,
    /// The author asked to close it (`x`).
    stopped: bool,
    elapsed: Duration,
}

/// The list's notice when the game closes: closed (code 0), stopped (by `x`), or exited with the
/// code and, if there is one, the first stderr line, which in a game that dies at boot says why.
fn ending_notice(name: &str, ending: &Ending, scoreboard: &Scoreboard) -> Notice {
    let duration = formatting::clock(ending.elapsed);
    let facts = format!("{} fatos", scoreboard.total());
    let code = ending.code;
    let (text, is_problem) = match (code, ending.stopped, scoreboard.first_error()) {
        (0, _, _) => (
            format!("{name} fechou depois de {duration} · {facts}"),
            false,
        ),
        (_, true, _) => (
            format!("{name} encerrado depois de {duration} · {facts}"),
            false,
        ),
        (_, false, Some(error)) => (format!("{name} saiu com {code} · {error}"), true),
        (_, false, None) => (format!("{name} saiu com {code} depois de {duration}"), true),
    };
    Notice { text, is_problem }
}

/// A `RunningGame` dropped with the game still open (jogar exiting by a signal, or by a panic)
/// closes the whole game, which would otherwise be orphaned in its own group. A game that already
/// closed and was reaped gets no signal: `try_wait` returns the stored status, and the pid may
/// already belong to another process.
impl Drop for RunningGame {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            signal_group(&self.child, libc::SIGTERM);
        }
        signals::set_game_group(0);
    }
}

/// Sends `signal` to the process group of `child`, which `games::spawn_piped` creates with its pid.
fn signal_group(child: &Child, signal: libc::c_int) {
    let group = child.id() as libc::pid_t;
    // SAFETY: killpg only sends a signal; it does not touch this process's memory.
    let _ = unsafe { libc::killpg(group, signal) };
}

/// Reads `pipe` in a thread, line by line (bytes that are not UTF-8 become �), and sends each line
/// with the time it arrived. Ends when the pipe closes or the panel goes away.
fn listen(pipe: impl Read + Send + 'static, source: Source, sender: Sender<Arrival>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        let mut bytes = Vec::new();
        while reader.read_until(b'\n', &mut bytes).unwrap_or(0) > 0 {
            let line = String::from_utf8_lossy(&bytes)
                .trim_end_matches(['\n', '\r'])
                .to_string();
            if sender.send((source, line, Instant::now())).is_err() {
                return;
            }
            bytes.clear();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::Game;
    use crate::test_support::{
        LAUNCHER_WITH_GRANDCHILD, dies_within, fake_game, test_folder, wait_for_pid,
    };

    /// Audit of 28/10, finding A1: the game starts in its own group, so, if jogar exits without
    /// closing it (signal, panic), nobody else closes it.
    #[test]
    fn a_dropped_running_game_with_the_game_open_closes_the_whole_game() {
        let root = test_folder("running-dropped");
        fake_game(&root, "bomberboom", LAUNCHER_WITH_GRANDCHILD);
        let game = Game {
            folder: "bomberboom".to_string(),
        };
        let running = RunningGame::start(&root, &game).unwrap();
        let grandchild = wait_for_pid(&root.join("bomberboom/grandchild"), Duration::from_secs(3));

        drop(running);
        let died = dies_within(grandchild, Duration::from_secs(3));
        // SAFETY: only sends a signal to the grandchild the test created, in case it was left over.
        let _ = unsafe { libc::kill(grandchild, libc::SIGKILL) };
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            died,
            "the grandchild {grandchild} survived the dropped RunningGame"
        );
    }

    #[test]
    fn ending_notice_says_how_the_game_closed() {
        let mut scoreboard = Scoreboard::new("CampCrawler");
        scoreboard.record(Source::Stdout, "CAMPCRAWLER MAPA numero=1", Duration::ZERO);
        let ending = |code, stopped| Ending {
            code,
            stopped,
            elapsed: Duration::from_secs(134),
        };
        let notice = |ending: Ending, scoreboard: &Scoreboard| {
            let notice = ending_notice("CampCrawler", &ending, scoreboard);
            (notice.text, notice.is_problem)
        };
        let closed = "CampCrawler fechou depois de 02:14 · 1 fatos";
        assert_eq!(
            notice(ending(0, false), &scoreboard),
            (closed.to_string(), false)
        );
        let stopped = "CampCrawler encerrado depois de 02:14 · 1 fatos";
        assert_eq!(
            notice(ending(143, true), &scoreboard),
            (stopped.to_string(), false)
        );
        let exited = "CampCrawler saiu com 1 depois de 02:14";
        assert_eq!(
            notice(ending(1, false), &scoreboard),
            (exited.to_string(), true)
        );
        scoreboard.record(Source::Stderr, "Error: x11 not available", Duration::ZERO);
        let error = "CampCrawler saiu com 1 · Error: x11 not available";
        assert_eq!(
            notice(ending(1, false), &scoreboard),
            (error.to_string(), true)
        );
    }
}
