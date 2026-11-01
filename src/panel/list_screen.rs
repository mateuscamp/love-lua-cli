//! The list's drawing: the games on the left, the chosen one's details on the right, the X display
//! in the title and the notice at the bottom. It only draws; what to show arrives ready in
//! `ListView`, and the colors come from `theme`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::formatting;
use super::theme::{self, dimmed, key_hint};
use crate::games::{Game, LAUNCHER};
use crate::git_state::Repo;

/// The width of the details' labels (`lançador  `).
const LABEL_WIDTH: usize = 10;
/// The shortest name the games column considers, so that its title fits.
const MIN_NAME_WIDTH: usize = 4;

/// What is known about a discovered game.
pub struct GameEntry {
    pub game: Game,
    /// `None` with a folder without git, or with a git that did not answer.
    pub repo: Option<Repo>,
    /// The copy of the shared launcher differs from the other games' (`shared_launcher`).
    pub launcher_differs: bool,
    /// Why the game cannot be played yet, when its folder declares it (`NAO_JOGAVEL`).
    pub unplayable: Option<String>,
}

/// The sentence from the last attempt: the game that closed, the one that did not open.
pub struct Notice {
    pub text: String,
    /// Shown in coral: the game did not open or exited with an error.
    pub is_problem: bool,
}

pub struct ListView<'a> {
    /// The root as the author writes it (`~/lua`).
    pub root: &'a str,
    /// One per discovered game, in order; empty when the root has no game.
    pub entries: &'a [GameEntry],
    pub chosen: usize,
    pub display_check: &'a Result<(), String>,
    pub display: &'a str,
    pub notice: Option<&'a Notice>,
    /// Unix seconds now, for the "há" of the last commit.
    pub now: i64,
}

pub fn draw_list(frame: &mut Frame, view: &ListView) {
    let area = frame.area();
    let outer = theme::outer_block()
        .title(theme::tag(" jogar "))
        .title_top(display_title(view).right_aligned());
    let inside = outer.inner(area);
    frame.render_widget(outer, area);
    let footer = list_footer(view);
    let [panes, bottom] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(footer.len() as u16)])
            .areas(inside);
    let width = view
        .entries
        .iter()
        .map(|entry| entry.game.folder.chars().count())
        .max()
        .unwrap_or(0)
        // In the empty list, or one of short names, the ` Jogos ` title still fits in the frame.
        .max(MIN_NAME_WIDTH) as u16
        + 8;
    let [left, right] =
        Layout::horizontal([Constraint::Length(width), Constraint::Min(10)]).areas(panes);
    draw_games(frame, left, view);
    draw_details(frame, right, view);
    frame.render_widget(Paragraph::new(footer), bottom);
    theme::paint_border(frame.buffer_mut(), area);
}

fn display_title(view: &ListView) -> Line<'static> {
    match view.display_check {
        Ok(()) => Line::from(format!(" tela X {} ✓ ", view.display)).fg(theme::CYAN),
        Err(_) => Line::from(" sem tela X ").fg(theme::CORAL).bold(),
    }
}

/// The display warning, the notice and the keys' help, one per line.
fn list_footer(view: &ListView) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Err(warning) = view.display_check {
        lines.push(Line::from(format!(" {warning}")).fg(theme::CORAL));
    }
    if let Some(notice) = view.notice {
        let color = if notice.is_problem {
            theme::CORAL
        } else {
            theme::CYAN
        };
        lines.push(Line::from(format!(" {}", notice.text)).fg(color));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        key_hint("↑↓"),
        dimmed(" escolhe   "),
        key_hint("Enter"),
        dimmed(" abre   "),
        key_hint("Esc"),
        dimmed(" sai"),
    ]));
    lines
}

fn is_in_progress(entry: &GameEntry) -> bool {
    entry
        .repo
        .as_ref()
        .is_some_and(|repo| repo.work_in_progress().is_some())
}

/// The list of games, with the gradient bar on the chosen one's line.
fn draw_games(frame: &mut Frame, area: Rect, view: &ListView) {
    let items: Vec<ListItem> = view
        .entries
        .iter()
        .map(|entry| {
            let mut parts = vec![Span::raw(entry.game.folder.clone())];
            if is_in_progress(entry) {
                parts.push(" ●".fg(theme::ORCHID));
            }
            if entry.launcher_differs {
                parts.push(" ≠".fg(theme::CORAL));
            }
            if entry.unplayable.is_some() {
                parts.push(" ✕".fg(theme::CORAL));
            }
            ListItem::new(Line::from(parts))
        })
        .collect();
    let block = theme::inner_block(" Jogos ".fg(theme::DIM).bold());
    let inside = block.inner(area);
    let list = List::new(items)
        .block(block)
        .highlight_symbol(Line::from("▶ ").fg(theme::MAGENTA))
        .highlight_style(Modifier::BOLD);
    let mut state = ListState::default().with_selected(Some(view.chosen));
    frame.render_stateful_widget(list, area, &mut state);
    // The list scrolls when it does not fit: the bar goes on the line where the chosen one landed.
    let line = view.chosen.saturating_sub(state.offset()) as u16;
    if line < inside.height {
        let strip = Rect::new(inside.x, inside.y + line, inside.width, 1);
        theme::paint_bar(frame.buffer_mut(), strip);
    }
}

fn draw_details(frame: &mut Frame, area: Rect, view: &ListView) {
    let Some(entry) = view.entries.get(view.chosen) else {
        draw_empty(frame, area, view.root);
        return;
    };
    let game = &entry.game;
    let mut launcher = vec![Span::raw(LAUNCHER.join(" "))];
    if entry.launcher_differs {
        launcher.push(" ≠ difere dos outros jogos".fg(theme::CORAL));
    }
    let mut lines = vec![
        labeled("pasta", folder_spans(view.root, game)),
        labeled("lançador", launcher),
    ];
    if let Some(reason) = &entry.unplayable {
        lines.push(labeled(
            "estado",
            vec![
                "✕ não jogável: ".fg(theme::CORAL),
                Span::raw(reason.clone()),
            ],
        ));
    }
    lines.extend(git_lines(entry.repo.as_ref(), view.now));
    let title = Span::raw(format!(" {} ", game.folder))
        .fg(theme::MAGENTA)
        .bold();
    let block = theme::inner_block(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// No game in the root: where it looked and what turns a folder into a game.
fn draw_empty(frame: &mut Frame, area: Rect, root: &str) {
    let lines = vec![
        Line::from(format!(" nenhum jogo em {root}")),
        Line::from(dimmed(format!(
            " um jogo é uma pasta com {} executável",
            LAUNCHER[0]
        ))),
    ];
    let block = theme::inner_block(" jogar ".fg(theme::MAGENTA).bold());
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn labeled(label: &str, value: Vec<Span<'static>>) -> Line<'static> {
    let mut parts = vec![dimmed(format!(" {label:<LABEL_WIDTH$}"))];
    parts.extend(value);
    Line::from(parts)
}

fn folder_spans(root: &str, game: &Game) -> Vec<Span<'static>> {
    vec![Span::raw(format!("{root}/{}", game.folder))]
}

/// Branch, changes and last commit; the main branch in cyan, work in progress in orchid.
fn git_lines(repo: Option<&Repo>, now: i64) -> Vec<Line<'static>> {
    let Some(repo) = repo else {
        return vec![labeled("branch", vec![dimmed("— sem git")])];
    };
    let name = match &repo.branch {
        Some(branch) => branch.clone(),
        None => format!("HEAD solto em {}", repo.commit),
    };
    let branch = if repo.off_main().is_some() {
        vec![Span::raw(name), " ● em obra".fg(theme::ORCHID)]
    } else {
        vec![Span::raw(name).fg(theme::CYAN)]
    };
    let changes = match repo.changes_in_words() {
        Some(text) => Span::raw(text).fg(theme::ORCHID),
        None => dimmed("nenhuma"),
    };
    let mut lines = vec![
        labeled("branch", branch),
        labeled("mudanças", vec![changes]),
    ];
    lines.extend(last_commit_lines(repo, now));
    lines
}

fn last_commit_lines(repo: &Repo, now: i64) -> Vec<Line<'static>> {
    let Some(commit) = &repo.last_commit else {
        return vec![labeled("último", vec![dimmed("—")])];
    };
    let ago = formatting::short_duration((now - commit.timestamp).max(0) as u64);
    vec![
        labeled(
            "último",
            vec![Span::raw(format!("{} · há {ago}", repo.commit))],
        ),
        labeled("", vec![dimmed(commit.subject.clone())]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_state::Commit;
    use crate::panel::{find_text, screen_text, test_screen, theme};

    const NOW: i64 = 1_793_200_000;

    fn entries() -> Vec<GameEntry> {
        let clean = Repo {
            branch: Some("main".to_string()),
            main_branch: "main".to_string(),
            commit: "a988f4e".to_string(),
            changes: 0,
            last_commit: None,
        };
        let in_progress = Repo {
            branch: Some("r0-pendencias".to_string()),
            commit: "48f0ad1".to_string(),
            changes: 5,
            last_commit: Some(Commit {
                timestamp: NOW - 12 * 60,
                subject: "O mostrado acompanha as passagens".to_string(),
            }),
            ..clean.clone()
        };
        let game = |folder: &str| Game {
            folder: folder.to_string(),
        };
        vec![
            GameEntry {
                game: game("bomberboom"),
                repo: Some(clean),
                launcher_differs: false,
                unplayable: None,
            },
            GameEntry {
                game: game("boomlitude"),
                repo: Some(in_progress),
                launcher_differs: false,
                unplayable: None,
            },
            GameEntry {
                game: game("campcrawler"),
                repo: None,
                launcher_differs: true,
                unplayable: None,
            },
        ]
    }

    fn view<'a>(entries: &'a [GameEntry], display_check: &'a Result<(), String>) -> ListView<'a> {
        ListView {
            root: "~/lua",
            entries,
            chosen: 1,
            display_check,
            display: ":0",
            notice: None,
            now: NOW,
        }
    }

    #[test]
    fn the_list_marks_work_in_progress_and_details_the_chosen_one() {
        let entries = entries();
        let display_check = Ok(());
        let text = screen_text(80, 14, |frame| {
            draw_list(frame, &view(&entries, &display_check))
        });
        for expected in [
            "jogar",
            "tela X :0 ✓",
            "▶ boomlitude ●",
            "bomberboom ",
            "~/lua/boomlitude",
            "lançador  tools/love .",
            "r0-pendencias ● em obra",
            "5 mudanças sem commit",
            "48f0ad1 · há 12m",
            "O mostrado acompanha",
            "↑↓ escolhe",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert!(!text.contains("bomberboom ●"), "{text}");
        assert!(
            text.contains("campcrawler ≠"),
            "the one that differs, in the list: {text}"
        );
        assert!(
            !text.contains("difere"),
            "the chosen one does not differ: {text}"
        );
    }

    /// A game that declares itself unplayable is marked in the list and says why in the details,
    /// even on the main branch (author, 01/11: Gods showed `main` and gave no hint).
    #[test]
    fn the_list_marks_the_unplayable_game_and_gives_the_reason() {
        let mut entries = entries();
        entries[0].unplayable = Some("só o domínio, sem telas".to_string());
        let display_check = Ok(());
        let mut shown = view(&entries, &display_check);
        shown.chosen = 0;
        let text = screen_text(80, 14, |frame| draw_list(frame, &shown));
        for expected in ["bomberboom ✕", "✕ não jogável: só o domínio, sem telas"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert!(!text.contains("boomlitude ✕"), "{text}");
    }

    #[test]
    fn the_list_paints_in_indigo_and_magenta() {
        let entries = entries();
        let display_check = Ok(());
        let painted = test_screen(80, 14, |frame| {
            draw_list(frame, &view(&entries, &display_check))
        });
        assert_eq!(
            painted[(60, 9)].bg,
            theme::NIGHT,
            "the background is the indigo night"
        );
        let corners = (painted[(0, 0)].fg, painted[(79, 13)].fg);
        assert_eq!(
            corners,
            (theme::INDIGO, theme::MAGENTA),
            "the border in a gradient"
        );
        let color = |text: &str| {
            let (x, y) = find_text(&painted, text);
            (painted[(x, y)].fg, painted[(x, y)].bg)
        };
        assert_eq!(
            color("jogar"),
            (theme::NIGHT, theme::MAGENTA),
            "the title is a tag"
        );
        assert_eq!(color("✓"), (theme::CYAN, theme::NIGHT));
        assert_eq!(color("▶ boomlitude"), (theme::MAGENTA, theme::BAR.0));
        assert_eq!(
            color("●").0,
            theme::ORCHID,
            "the work-in-progress mark in the list"
        );
        assert_eq!(color("● em obra").0, theme::ORCHID);
        assert_eq!(color("≠").0, theme::CORAL, "the launcher that differs");
        assert_eq!(color("Enter").0, theme::MAGENTA, "the help's key");
    }

    /// In a short terminal the list scrolls, and the bar goes with the chosen one.
    #[test]
    fn the_bar_follows_the_list_that_scrolled() {
        let entries = entries();
        let display_check = Ok(());
        let mut last = view(&entries, &display_check);
        last.chosen = 2;
        let painted = test_screen(80, 7, |frame| draw_list(frame, &last));
        let (x, y) = find_text(&painted, "▶ campcrawler");
        assert_eq!(painted[(x, y)].bg, theme::BAR.0);
    }

    /// The chosen one whose launcher differs says so in the details, in coral.
    #[test]
    fn the_list_without_a_display_warns_and_shows_the_launcher_that_differs() {
        let entries = entries();
        let display_check = Err("sem tela X: ninguém atende no DISPLAY=\":77\"".to_string());
        let notice = Notice {
            text: "Boomlitude saiu com 1 depois de 00:03".to_string(),
            is_problem: true,
        };
        let mut view = view(&entries, &display_check);
        view.chosen = 2;
        view.notice = Some(&notice);
        let text = screen_text(100, 14, |frame| draw_list(frame, &view));
        for expected in [
            "sem tela X",
            "ninguém atende no DISPLAY=\":77\"",
            "lançador  tools/love . ≠ difere dos outros jogos",
            "— sem git",
            "Boomlitude saiu com 1",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert!(!text.contains("✓"), "{text}");
    }

    /// A root with no game: the details say where it looked and what a game is.
    #[test]
    fn the_empty_list_says_where_it_looked_and_what_a_game_is() {
        let display_check = Ok(());
        let mut empty = view(&[], &display_check);
        empty.chosen = 0;
        let text = screen_text(100, 12, |frame| draw_list(frame, &empty));
        for expected in [
            "╭ Jogos ",
            "nenhum jogo em ~/lua",
            "um jogo é uma pasta com tools/love executável",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
    }
}
