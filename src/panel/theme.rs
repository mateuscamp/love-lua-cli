//! The panel's colors, indigo and magenta (author, 30/10): the indigo night as the background,
//! magenta on what calls for the eye, and the gradient from one to the other on the outer border,
//! on the chosen one's bar and on the sparkline. Only color and painting; what to draw comes ready
//! from `list_screen.rs` and `live_screen.rs`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType};

/// The background of everything: an almost-black indigo.
pub const NIGHT: Color = Color::Rgb(17, 13, 43);
/// The inner borders and the start of the gradients.
pub const INDIGO: Color = Color::Rgb(75, 63, 196);
/// Labels, help, what is secondary.
pub const DIM: Color = Color::Rgb(122, 113, 204);
/// The text.
pub const LAVENDER: Color = Color::Rgb(226, 222, 255);
/// The highlight: the title, the key, the mark of what just changed, the end of the gradients.
pub const MAGENTA: Color = Color::Rgb(255, 46, 160);
/// Work in progress: a folder off the main branch or with changes.
pub const ORCHID: Color = Color::Rgb(214, 120, 255);
/// All good: the X display, the main branch, the game that closed well.
pub const CYAN: Color = Color::Rgb(98, 214, 255);
/// Trouble: no display, a missing folder, a game that died, an error.
pub const CORAL: Color = Color::Rgb(255, 92, 122);
/// The background of the chosen one's bar, from dark magenta, on the left, to dark indigo.
pub const BAR: (Color, Color) = (Color::Rgb(112, 24, 122), Color::Rgb(38, 30, 98));

/// The strokes of the rounded border; the rest of the border is title.
const STROKES: [&str; 6] = ["─", "│", "╭", "╮", "╰", "╯"];

/// The color at `fraction` (from 0 to 1) of the way from `from` to `to`. A color that is not RGB
/// stays `from`.
pub fn gradient(from: Color, to: Color, fraction: f32) -> Color {
    let (Color::Rgb(r0, g0, b0), Color::Rgb(r1, g1, b1)) = (from, to) else {
        return from;
    };
    let fraction = fraction.clamp(0.0, 1.0);
    let mix =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * fraction).round() as u8;
    Color::Rgb(mix(r0, r1), mix(g0, g1), mix(b0, b1))
}

/// Where `x` falls in the width of `area`: 0 on the first column, 1 on the last.
fn position_in_width(area: Rect, x: u16) -> f32 {
    let last = area.width.saturating_sub(1).max(1);
    f32::from(x.saturating_sub(area.x)) / f32::from(last)
}

/// Paints the strokes of the border of `area` from `INDIGO`, on the left, to `MAGENTA`, on the
/// right. The title on the border keeps its own color.
pub fn paint_border(screen: &mut Buffer, area: Rect) {
    let area = area.intersection(screen.area);
    if area.is_empty() {
        return;
    }
    let top_and_bottom =
        (area.left()..area.right()).flat_map(|x| [(x, area.top()), (x, area.bottom() - 1)]);
    let sides = (area.top()..area.bottom()).flat_map(|y| [(area.left(), y), (area.right() - 1, y)]);
    for (x, y) in top_and_bottom.chain(sides) {
        let cell = &mut screen[(x, y)];
        if STROKES.contains(&cell.symbol()) {
            cell.set_fg(gradient(INDIGO, MAGENTA, position_in_width(area, x)));
        }
    }
}

/// Paints the background of `line` with the `BAR` gradient, without touching the text or its
/// color.
pub fn paint_bar(screen: &mut Buffer, line: Rect) {
    let line = line.intersection(screen.area);
    for x in line.left()..line.right() {
        screen[(x, line.y)].set_bg(gradient(BAR.0, BAR.1, position_in_width(line, x)));
    }
}

/// The outer border, which paints the background in night and the text in lavender. The strokes
/// get the gradient from `paint_border`, after the rest is already drawn.
pub fn outer_block<'a>() -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .style(Style::new().fg(LAVENDER).bg(NIGHT))
}

/// An inner border, in indigo, with `title` painted as it comes.
pub fn inner_block(title: Span) -> Block {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(INDIGO))
        .title(title)
}

/// The name as a tag: night on magenta, in the outer border's title.
pub fn tag<'a>(text: impl Into<Cow<'a, str>>) -> Span<'a> {
    Span::raw(text).fg(NIGHT).bg(MAGENTA).bold()
}

/// The key in the footer's help.
pub fn key_hint(text: &'static str) -> Span<'static> {
    Span::raw(text).fg(MAGENTA).bold()
}

/// The secondary text: label, help, commit subject.
pub fn dimmed<'a>(text: impl Into<Cow<'a, str>>) -> Span<'a> {
    Span::raw(text).fg(DIM)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::{find_text, test_screen};

    #[test]
    fn gradient_goes_from_one_color_to_the_other() {
        let (black, white) = (Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255));
        assert_eq!(gradient(black, white, 0.0), black);
        assert_eq!(gradient(black, white, 1.0), white);
        assert_eq!(gradient(black, white, 0.5), Color::Rgb(128, 128, 128));
        assert_eq!(gradient(black, white, 7.0), white, "a fraction beyond 1");
        assert_eq!(gradient(Color::Reset, white, 0.5), Color::Reset);
    }

    #[test]
    fn paint_border_paints_the_strokes_and_leaves_the_title() {
        let screen = test_screen(20, 3, |frame| {
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .title(Span::raw(" ab ").fg(CYAN));
            let area = frame.area();
            frame.render_widget(block, area);
            paint_border(frame.buffer_mut(), area);
        });
        assert_eq!(screen[(0, 0)].fg, INDIGO);
        assert_eq!(screen[(19, 2)].fg, MAGENTA);
        assert_eq!(screen[(19, 1)].fg, MAGENTA, "the right side, all magenta");
        let middle = screen[(10, 2)].fg;
        assert!(
            middle != INDIGO && middle != MAGENTA,
            "the middle is a mix: {middle:?}"
        );
        let (x, y) = find_text(&screen, "ab");
        assert_eq!(screen[(x, y)].fg, CYAN, "the title kept its color");
    }

    #[test]
    fn paint_bar_paints_the_background_and_leaves_the_text() {
        let screen = test_screen(10, 1, |frame| {
            let area = frame.area();
            frame.render_widget(Span::raw("hi").fg(CYAN), area);
            paint_bar(frame.buffer_mut(), area);
        });
        assert_eq!((screen[(0, 0)].bg, screen[(9, 0)].bg), BAR);
        assert_eq!((screen[(0, 0)].symbol(), screen[(0, 0)].fg), ("h", CYAN));
    }

    #[test]
    fn an_empty_area_neither_paints_nor_breaks() {
        let screen = test_screen(4, 2, |frame| {
            paint_border(frame.buffer_mut(), Rect::new(0, 0, 0, 0));
            paint_border(frame.buffer_mut(), Rect::new(9, 9, 3, 3));
        });
        assert_eq!(screen[(0, 0)].fg, Color::Reset);
    }

    #[test]
    fn a_single_column_does_not_divide_by_zero() {
        let screen = test_screen(1, 1, |frame| {
            let area = frame.area();
            paint_bar(frame.buffer_mut(), area);
        });
        assert_eq!(screen[(0, 0)].bg, BAR.0);
    }
}
