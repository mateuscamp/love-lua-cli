//! The full-screen panel (author, 28/10): the list of games with the chosen one's details and,
//! while a game runs in its window, the live state of what it writes. This file is the loop and the
//! list, the only one that talks to the terminal; the running game lives in `game_process`, signals
//! in `signals`, drawing in `list_screen` and `live_screen`, colors in `theme`, keys in `keys`.
//!
//! The game starts without a terminal (its output in a pipe), so the panel stays on screen the
//! whole time: there is no suspending and resuming raw mode. When the game's window closes, the
//! list comes back with a notice.

#[cfg(test)]
mod draw_test_support;
mod formatting;
mod game_process;
mod keys;
mod list_screen;
mod live_screen;
mod signals;
mod theme;

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEvent};

use crate::games::{self, Game};
use crate::shared_launcher::{self, LauncherCopy};
use crate::{git_state, x_display};
use game_process::RunningGame;
use keys::Action;
use list_screen::{GameEntry, ListView, Notice};
use signals::{exit_by_signal, install_exit_signals};

#[cfg(test)]
pub(crate) use draw_test_support::{find_text, screen_text, test_screen};

/// How often the panel wakes up without a key, so the scoreboard moves.
const TICK: Duration = Duration::from_millis(100);
/// How often the list rereads the folders' git and the X display.
const REREAD_EVERY: Duration = Duration::from_secs(2);

/// Opens the panel and only returns when the author quits: 0 with Esc or q, 130 with Ctrl+C,
/// 128 + signal with SIGHUP or SIGTERM, 2 without a terminal.
pub fn menu(root: &Path) -> ExitCode {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("jogar: o menu precisa de um terminal; sem ele, jogar <jogo> abre direto");
        return ExitCode::from(2);
    }
    install_exit_signals();
    let mut terminal = ratatui::init();
    let exit = event_loop(&mut terminal, root);
    ratatui::restore();
    exit.unwrap_or_else(|error| {
        eprintln!("jogar: o terminal falhou: {error}");
        ExitCode::from(2)
    })
}

/// The loop leaves through `return`, and `running` drops with it: with a game open, its `Drop`
/// closes the game.
fn event_loop(terminal: &mut DefaultTerminal, root: &Path) -> io::Result<ExitCode> {
    let mut list = GameList::load(root);
    let mut running: Option<RunningGame> = None;
    loop {
        if let Some(exit) = exit_by_signal() {
            return Ok(exit);
        }
        tick(&mut list, &mut running);
        terminal.draw(|frame| match &running {
            Some(playing) => live_screen::draw_live(frame, &playing.live_view()),
            None => list_screen::draw_list(frame, &list.list_view()),
        })?;
        let Some(key) = next_key()? else {
            continue;
        };
        if let Some(exit) = react(keys::action_for(key), &mut list, &mut running) {
            return Ok(exit);
        }
    }
}

/// The key that arrives within `TICK`. A signal that interrupts the wait (the SIGHUP of a terminal
/// that closes) is not an error: the loop comes back and sees the recorded signal.
fn next_key() -> io::Result<Option<KeyEvent>> {
    let interrupted = |error: &io::Error| error.kind() == io::ErrorKind::Interrupted;
    match event::poll(TICK) {
        Ok(true) => {}
        Ok(false) => return Ok(None),
        Err(error) if interrupted(&error) => return Ok(None),
        Err(error) => return Err(error),
    }
    match event::read() {
        Ok(Event::Key(key)) => Ok(Some(key)),
        Ok(_) => Ok(None),
        Err(error) if interrupted(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// What moves without a key: the scoreboard collects what the game wrote and, when the game
/// closes, the list comes back with the notice; with no game, the list rereads git and the display
/// from time to time.
fn tick(list: &mut GameList, running: &mut Option<RunningGame>) {
    let Some(playing) = running else {
        list.reload_if_stale();
        return;
    };
    playing.drain_lines();
    if let Some(notice) = playing.poll_end() {
        *running = None;
        list.reload();
        list.notice = Some(notice);
    }
}

/// The action in context. In the list, arrows choose, Enter opens and Esc or q quit; in the running
/// game, arrows scroll and `x` stops the game. Ctrl+C quits from anywhere, closing the game first.
/// `Some` is quitting.
fn react(
    action: Action,
    list: &mut GameList,
    running: &mut Option<RunningGame>,
) -> Option<ExitCode> {
    match (action, running.as_mut()) {
        (Action::Interrupt, playing) => {
            playing.map(RunningGame::stop_and_wait);
            return Some(ExitCode::from(130));
        }
        (Action::MoveUp, Some(playing)) => playing.scroll_by(-1),
        (Action::MoveDown, Some(playing)) => playing.scroll_by(1),
        (Action::StopGame, Some(playing)) => playing.stop(),
        (_, Some(_)) => {}
        (Action::MoveUp, None) => list.move_choice(-1),
        (Action::MoveDown, None) => list.move_choice(1),
        (Action::Open, None) => *running = list.open_chosen(),
        (Action::Quit, None) => return Some(ExitCode::SUCCESS),
        (_, None) => {}
    }
    None
}

/// The list: what is known about each game and what the author chose.
struct GameList {
    root: PathBuf,
    shown_root: String,
    chosen: usize,
    entries: Vec<GameEntry>,
    display_check: Result<(), String>,
    display: String,
    notice: Option<Notice>,
    read_at: Instant,
}

impl GameList {
    fn load(root: &Path) -> GameList {
        let mut list = GameList::blank(root);
        list.reload();
        list
    }

    /// The list before reading git and the X display: the tests start from it, without touching
    /// the author's X.
    fn blank(root: &Path) -> GameList {
        GameList {
            root: root.to_path_buf(),
            shown_root: tilde_root(root),
            chosen: 0,
            entries: Vec::new(),
            display_check: Ok(()),
            display: String::new(),
            notice: None,
            read_at: Instant::now(),
        }
    }

    /// Rediscovers the games and rereads each folder's git and the X display (about 15 ms for three
    /// games, measured before Gods and before discovery).
    fn reload(&mut self) {
        self.reload_games();
        self.display_check = x_display::check_this_display();
        self.display = std::env::var("DISPLAY").unwrap_or_default();
        self.read_at = Instant::now();
    }

    /// The games in the root, with each one's git and launcher copy; the chosen one stays in the
    /// list.
    fn reload_games(&mut self) {
        let games = games::discover(&self.root);
        let copies: Vec<LauncherCopy> = games
            .iter()
            .map(|game| shared_launcher::read_copy(&self.root.join(&game.folder)))
            .collect();
        self.entries = games
            .into_iter()
            .zip(shared_launcher::which_differ(&copies))
            .map(|(game, launcher_differs)| game_entry(&self.root, game, launcher_differs))
            .collect();
        self.chosen = self.chosen.min(self.entries.len().saturating_sub(1));
    }

    fn reload_if_stale(&mut self) {
        if self.read_at.elapsed() >= REREAD_EVERY {
            self.reload();
        }
    }

    /// Moves along the list, wrapping around at the ends; in the empty list, stays put.
    fn move_choice(&mut self, step: isize) {
        let total = self.entries.len() as isize;
        if total == 0 {
            return;
        }
        self.chosen = (self.chosen as isize + step).rem_euclid(total) as usize;
    }

    /// Starts the chosen game; without an X display or if it does not start, the notice stays and
    /// nothing opens. The display warning is already in the footer, so the notice does not repeat
    /// it. In the empty list, nothing: the details already say what a game is.
    fn open_chosen(&mut self) -> Option<RunningGame> {
        let game = &self.entries.get(self.chosen)?.game;
        if self.display_check.is_err() {
            self.notice = Some(Notice {
                text: format!("{} não abre sem tela X", game.folder),
                is_problem: true,
            });
            return None;
        }
        match RunningGame::start(&self.root, game) {
            Ok(playing) => {
                self.notice = None;
                Some(playing)
            }
            Err(error) => {
                self.notice = Some(Notice {
                    text: error,
                    is_problem: true,
                });
                None
            }
        }
    }

    fn list_view(&self) -> ListView<'_> {
        ListView {
            root: &self.shown_root,
            entries: &self.entries,
            chosen: self.chosen,
            display_check: &self.display_check,
            display: &self.display,
            notice: self.notice.as_ref(),
            now: unix_seconds(),
        }
    }
}

fn game_entry(root: &Path, game: Game, launcher_differs: bool) -> GameEntry {
    let repo = git_state::read_repo(&root.join(&game.folder));
    let unplayable = games::unplayable_reason(root, &game);
    GameEntry {
        game,
        repo,
        launcher_differs,
        unplayable,
    }
}

/// The root with `HOME` written as `~`, as the author writes it.
fn tilde_root(root: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.and_then(|home| root.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => root.display().to_string(),
    }
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        LAUNCHER_WITH_GRANDCHILD, dies_within, fake_game, test_folder, wait_for_pid,
    };
    use std::thread;

    // Audit of 28/10, finding A2: the panel's loop without a terminal. The keys come in through
    // `react`, time through `tick`, and the games are fake.

    /// Runs `tick` until the game closes and the list comes back.
    fn wait_for_the_end(list: &mut GameList, running: &mut Option<RunningGame>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while running.is_some() {
            assert!(Instant::now() < deadline, "the fake game did not close");
            tick(list, running);
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn notice(list: &GameList) -> (&str, bool) {
        let notice = list.notice.as_ref().expect("the list has no notice");
        (notice.text.as_str(), notice.is_problem)
    }

    /// The list of the games discovered in `root`, without reading the author's X display.
    fn list_of(root: &Path) -> GameList {
        let mut list = GameList::blank(root);
        list.reload_games();
        list
    }

    #[test]
    fn in_the_list_the_arrows_wrap_around_and_esc_or_ctrl_c_quit() {
        let root = test_folder("list-keys");
        for folder in ["a", "b", "c"] {
            fake_game(&root, folder, "exit 0\n");
        }
        let mut list = list_of(&root);
        let mut running = None;
        assert_eq!(react(Action::MoveUp, &mut list, &mut running), None);
        assert_eq!(list.chosen, 2, "from the first, up to the last");
        assert_eq!(react(Action::MoveDown, &mut list, &mut running), None);
        assert_eq!(list.chosen, 0, "from the last, down to the first");
        assert_eq!(react(Action::StopGame, &mut list, &mut running), None);
        assert_eq!(
            react(Action::Quit, &mut list, &mut running),
            Some(ExitCode::SUCCESS)
        );
        assert_eq!(
            react(Action::Interrupt, &mut list, &mut running),
            Some(ExitCode::from(130))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The folder that vanishes between discovery and Enter leaves the notice, and nothing opens.
    #[test]
    fn enter_without_a_display_or_the_folder_opens_nothing_and_leaves_the_notice() {
        let root = test_folder("list-open");
        fake_game(&root, "bomberboom", "exit 0\n");
        let mut list = list_of(&root);
        let mut running = None;
        list.display_check = Err("sem tela X: o DISPLAY está vazio".to_string());
        react(Action::Open, &mut list, &mut running);
        assert!(running.is_none());
        assert_eq!(notice(&list), ("bomberboom não abre sem tela X", true));

        list.display_check = Ok(());
        std::fs::remove_dir_all(root.join("bomberboom")).unwrap();
        react(Action::Open, &mut list, &mut running);
        assert!(running.is_none());
        let (text, is_problem) = notice(&list);
        assert!(
            text.contains("bomberboom não existe") && is_problem,
            "{text}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A root with no game (someone else, or a wrong `$JOGAR_RAIZ`): the keys do not break and
    /// nothing opens.
    #[test]
    fn the_empty_list_does_not_break_and_enter_opens_nothing() {
        let root = test_folder("list-empty");
        let mut list = list_of(&root);
        let mut running = None;
        for action in [Action::MoveUp, Action::MoveDown, Action::Open] {
            assert_eq!(react(action, &mut list, &mut running), None);
        }
        assert!(running.is_none());
        assert_eq!(list.chosen, 0);
        assert!(list.notice.is_none(), "{:?}", list.notice.map(|n| n.text));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The list rediscovers itself with the panel open; a chosen game that vanishes moves the
    /// choice to the last one left, and not out of the list.
    #[test]
    fn the_choice_stays_in_the_list_when_a_game_vanishes() {
        let root = test_folder("list-vanishes");
        for folder in ["a", "b", "c"] {
            fake_game(&root, folder, "exit 0\n");
        }
        let mut list = list_of(&root);
        list.chosen = 2;
        std::fs::remove_dir_all(root.join("c")).unwrap();
        list.reload_games();
        assert_eq!(list.chosen, 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The game whose copy of the shared launcher differs from the others' is marked (author,
    /// 31/10).
    #[test]
    fn the_list_marks_the_game_whose_launcher_differs() {
        let root = test_folder("list-differs");
        fake_game(&root, "a", "exec love .\n");
        fake_game(&root, "b", "exec love .\n");
        fake_game(&root, "c", "exec love . # another copy\n");
        let list = list_of(&root);
        let marks: Vec<(&str, bool)> = list
            .entries
            .iter()
            .map(|entry| (entry.game.folder.as_str(), entry.launcher_differs))
            .collect();
        assert_eq!(marks, [("a", false), ("b", false), ("c", true)]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn x_stops_the_game_and_the_list_comes_back_with_the_notice() {
        let root = test_folder("running-x");
        let body = format!("echo 'BOMBERBOOM INICIO motor=love'\n{LAUNCHER_WITH_GRANDCHILD}");
        fake_game(&root, "bomberboom", &body);
        let mut list = list_of(&root);
        let mut running = None;
        react(Action::Open, &mut list, &mut running);
        assert!(
            running.is_some(),
            "did not open: {:?}",
            list.notice.map(|n| n.text)
        );
        let grandchild = wait_for_pid(&root.join("bomberboom/grandchild"), Duration::from_secs(3));
        let deadline = Instant::now() + Duration::from_secs(3);
        while running.as_ref().unwrap().scoreboard.total() == 0 {
            assert!(
                Instant::now() < deadline,
                "the fake game's fact did not arrive"
            );
            tick(&mut list, &mut running);
            thread::sleep(Duration::from_millis(20));
        }
        react(Action::MoveDown, &mut list, &mut running);
        react(Action::MoveDown, &mut list, &mut running);
        assert_eq!(
            running.as_ref().unwrap().scroll,
            0,
            "a single type: no scrolling"
        );

        react(Action::StopGame, &mut list, &mut running);
        wait_for_the_end(&mut list, &mut running);
        let (text, is_problem) = notice(&list);
        assert!(
            text.starts_with("bomberboom encerrado depois de 00:0"),
            "{text}"
        );
        assert!(text.ends_with("· 1 fatos") && !is_problem, "{text}");
        assert!(
            dies_within(grandchild, Duration::from_secs(3)),
            "the grandchild {grandchild} was left over"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_game_that_dies_leaves_its_code_and_its_error_in_the_notice() {
        let root = test_folder("running-dies");
        fake_game(
            &root,
            "bomberboom",
            "echo 'Error: no runtime' >&2\nexit 2\n",
        );
        let mut list = list_of(&root);
        let mut running = None;
        react(Action::Open, &mut list, &mut running);
        wait_for_the_end(&mut list, &mut running);
        assert_eq!(
            notice(&list),
            ("bomberboom saiu com 2 · Error: no runtime", true)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn ctrl_c_with_the_game_open_closes_the_game_and_exits_with_130() {
        let root = test_folder("running-ctrl-c");
        fake_game(&root, "bomberboom", LAUNCHER_WITH_GRANDCHILD);
        let mut list = list_of(&root);
        let mut running = None;
        react(Action::Open, &mut list, &mut running);
        assert!(
            running.is_some(),
            "did not open: {:?}",
            list.notice.map(|n| n.text)
        );
        let grandchild = wait_for_pid(&root.join("bomberboom/grandchild"), Duration::from_secs(3));
        let exit = react(Action::Interrupt, &mut list, &mut running);
        assert_eq!(exit, Some(ExitCode::from(130)));
        assert!(
            dies_within(grandchild, Duration::from_secs(1)),
            "the grandchild {grandchild} was left over"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
