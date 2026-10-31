//! The running game's drawing, the "live state" (author, 28/10): one line per fact type, updated
//! in place, with how many times it came, how long ago and the last value; the line that just
//! changed is marked. It does not scroll by itself, so it stays calm even at 50 facts per second.
//! The colors come from `theme`.

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

use super::formatting;
use super::theme::{self, dimmed, key_hint};
use crate::facts::{ERROR_KIND, FactKind, Scoreboard, TEXT_KIND};

/// Up to how long after arriving the line is marked.
const RECENT: Duration = Duration::from_secs(1);
/// How many seconds the title's sparkline covers.
const SPARKLINE_WINDOWS: usize = 12;

pub struct LiveView<'a> {
    pub name: &'a str,
    pub scoreboard: &'a Scoreboard,
    /// Since the game started.
    pub elapsed: Duration,
    /// How many types to skip from the top, with the arrows.
    pub scroll: usize,
}

pub fn draw_live(frame: &mut Frame, view: &LiveView) {
    let area = frame.area();
    let title = Line::from(vec![
        theme::tag(format!(" {} ", view.name)),
        Span::raw(format!("· jogando há {} ", formatting::clock(view.elapsed))),
    ]);
    let outer = theme::outer_block()
        .title(title)
        .title_top(facts_and_sparkline(view).right_aligned());
    let inside = outer.inner(area);
    frame.render_widget(outer, area);
    let footer = live_footer(view.scoreboard);
    let [middle, bottom] =
        Layout::vertical([Constraint::Min(2), Constraint::Length(footer.len() as u16)])
            .areas(inside);
    frame.render_widget(facts_table(view), middle);
    frame.render_widget(Paragraph::new(footer), bottom);
    theme::paint_border(frame.buffer_mut(), area);
}

/// How many facts and the rate's sparkline, each block from indigo (few) to magenta (the maximum).
fn facts_and_sparkline(view: &LiveView) -> Line<'static> {
    let rate = view.scoreboard.recent_rate(view.elapsed, SPARKLINE_WINDOWS);
    let mut parts = vec![Span::raw(format!(" {} fatos ", view.scoreboard.total()))];
    parts.extend(formatting::levels(&rate).into_iter().map(|level| {
        let color = theme::gradient(theme::INDIGO, theme::MAGENTA, level as f32 / 7.0);
        Span::raw(formatting::BLOCKS[level].to_string()).fg(color)
    }));
    parts.push(Span::raw(" "));
    Line::from(parts)
}

fn facts_table(view: &LiveView) -> Table<'static> {
    let header =
        Row::new([" TIPO", "VEZES", "HÁ", "O ÚLTIMO"]).style(Style::new().bold().fg(theme::DIM));
    let rows = view
        .scoreboard
        .kinds()
        .iter()
        .skip(view.scroll)
        .map(|kind| kind_row(kind, view))
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Length(13),
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Min(10),
    ];
    Table::new(rows, widths).header(header).column_spacing(1)
}

/// A type's row: marked with `▸` if it changed less than `RECENT` ago.
fn kind_row(kind: &FactKind, view: &LiveView) -> Row<'static> {
    let since = view.elapsed.saturating_sub(kind.last_at);
    let mark = if since < RECENT { "▸" } else { " " };
    let name = Line::from(vec![
        Span::raw(mark).fg(theme::MAGENTA),
        Span::raw(kind.name.clone()).fg(kind_color(kind)),
    ]);
    let last = if kind.name == "CONFIG" {
        config_summary(view.scoreboard)
    } else {
        formatting::fields_summary(&kind.fields)
    };
    let cells = [
        Cell::from(name),
        Cell::from(Line::from(kind.count.to_string()).right_aligned()),
        Cell::from(formatting::short_duration(since.as_secs())),
        Cell::from(last),
    ];
    Row::new(cells).style(row_style(kind, since))
}

fn row_style(kind: &FactKind, since: Duration) -> Style {
    match kind.name.as_str() {
        ERROR_KIND => Style::new().fg(theme::CORAL),
        TEXT_KIND => Style::new().fg(theme::DIM),
        _ if since < RECENT => Style::new().bold(),
        _ => Style::new(),
    }
}

/// The type's name: in orchid, except `erro` and `texto`, which keep the line's color.
fn kind_color(kind: &FactKind) -> Color {
    match kind.name.as_str() {
        ERROR_KIND => theme::CORAL,
        TEXT_KIND => theme::DIM,
        _ => theme::ORCHID,
    }
}

/// `CONFIG` comes once per key at boot; what matters is what is off the default.
fn config_summary(scoreboard: &Scoreboard) -> String {
    let off = scoreboard.off_default();
    if off.is_empty() {
        return "todas no padrão".to_string();
    }
    let pairs: Vec<String> = off
        .iter()
        .map(|(key, value)| format!("{key} {value}"))
        .collect();
    format!("{} fora do padrão: {}", off.len(), pairs.join(", "))
}

fn live_footer(scoreboard: &Scoreboard) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(error) = scoreboard.first_error() {
        lines.push(Line::from(format!(" erro: {error}")).fg(theme::CORAL));
    }
    lines.push(Line::from(vec![
        Span::raw(" "),
        key_hint("x"),
        dimmed(" encerra o jogo · "),
        key_hint("↑↓"),
        dimmed(" rola · o painel volta quando a janela fechar"),
    ]));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::Source;
    use crate::panel::{find_text, screen_text, test_screen, theme};

    fn scoreboard() -> Scoreboard {
        let mut scoreboard = Scoreboard::new("CampCrawler");
        let lines = [
            (
                0.1,
                "CAMPCRAWLER PARTIDA semente=1 idioma=pt_BR ritmo=normal",
            ),
            (
                0.1,
                "CAMPCRAWLER CONFIG chave=seed valor=17 origem=argumento",
            ),
            (
                0.1,
                "CAMPCRAWLER CONFIG chave=speed valor=normal origem=padrao",
            ),
            (2.0, "CAMPCRAWLER MAPA numero=1 linhas=9"),
            (120.0, "CAMPCRAWLER VEZ soldado=1 texto=1º"),
            (133.8, "CAMPCRAWLER VEZ soldado=2 texto=2º marcado=sim"),
        ];
        for (seconds, line) in lines {
            scoreboard.record(Source::Stdout, line, Duration::from_secs_f64(seconds));
        }
        scoreboard
    }

    fn view(scoreboard: &Scoreboard) -> LiveView<'_> {
        LiveView {
            name: "CampCrawler",
            scoreboard,
            elapsed: Duration::from_secs_f64(134.2),
            scroll: 0,
        }
    }

    #[test]
    fn the_live_screen_shows_each_type_and_marks_what_just_changed() {
        let scoreboard = scoreboard();
        let text = screen_text(80, 12, |frame| draw_live(frame, &view(&scoreboard)));
        for expected in [
            "CampCrawler · jogando há 02:14",
            "6 fatos",
            "TIPO",
            " PARTIDA ",
            "semente 1 · idioma pt_BR · ritmo normal",
            "1 fora do padrão: seed 17",
            "▸VEZ ",
            "soldado 2 · texto 2º · marcado sim",
            "x encerra o jogo",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert!(
            !text.contains("▸MAPA"),
            "MAPA has not changed for 2 min\n{text}"
        );
        let turn = text.lines().find(|line| line.contains("▸VEZ")).unwrap();
        assert!(turn.contains(" 2 ") && turn.contains("0s"), "{turn}");
    }

    #[test]
    fn the_live_screen_paints_the_tag_the_mark_and_the_sparkline() {
        let scoreboard = scoreboard();
        let painted = test_screen(80, 12, |frame| draw_live(frame, &view(&scoreboard)));
        let color = |text: &str| {
            let (x, y) = find_text(&painted, text);
            (painted[(x, y)].fg, painted[(x, y)].bg)
        };
        assert_eq!(color("CampCrawler"), (theme::NIGHT, theme::MAGENTA));
        assert_eq!(
            painted[(0, 0)].fg,
            theme::INDIGO,
            "the border in a gradient"
        );
        assert_eq!(
            color("▸VEZ").0,
            theme::MAGENTA,
            "the mark of what just changed"
        );
        assert_eq!(color("VEZ ").0, theme::ORCHID, "the type");
        // The sparkline goes from indigo, on the low block, to magenta, on the high one.
        assert_eq!(color("▁").0, theme::INDIGO);
        assert_eq!(color("█").0, theme::MAGENTA);
    }

    #[test]
    fn the_live_screen_scrolls_and_shows_the_first_error() {
        let mut scoreboard = scoreboard();
        scoreboard.record(
            Source::Stderr,
            "Error: Could not initialize SDL video subsystem",
            Duration::from_secs(134),
        );
        let mut view = view(&scoreboard);
        view.scroll = 1;
        let text = screen_text(80, 12, |frame| draw_live(frame, &view));
        assert!(!text.contains("PARTIDA"), "scrolled by one\n{text}");
        assert!(text.contains("CONFIG"), "{text}");
        assert!(
            text.contains("erro: Error: Could not initialize SDL"),
            "{text}"
        );
    }
}
