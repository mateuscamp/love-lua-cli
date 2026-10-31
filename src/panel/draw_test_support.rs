//! Helpers for the panel's drawing tests: the `TestBackend` screen and reading it, by text and by
//! color. They left `mod.rs` when it went past the 500-line ceiling (31/10).

/// The screen that `drawing` paints, `width` x `height`, for the tests to read text and color.
pub(crate) fn test_screen(
    width: u16,
    height: u16,
    drawing: impl FnOnce(&mut ratatui::Frame),
) -> ratatui::buffer::Buffer {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    terminal.draw(drawing).unwrap();
    terminal.backend().buffer().clone()
}

/// The column and line where `text` starts on `screen`, for the test to look at the color there.
pub(crate) fn find_text(screen: &ratatui::buffer::Buffer, text: &str) -> (u16, u16) {
    let area = screen.area;
    let letters: Vec<String> = text.chars().map(String::from).collect();
    let fits = |x: u16, y: u16| {
        letters.iter().enumerate().all(|(i, letter)| {
            let column = x + i as u16;
            column < area.right() && screen[(column, y)].symbol() == letter
        })
    };
    (area.top()..area.bottom())
        .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
        .find(|&(x, y)| fits(x, y))
        .unwrap_or_else(|| panic!("{text:?} is not on the screen"))
}

/// What `drawing` puts on a `width` x `height` screen, line by line, for the tests to read.
pub(crate) fn screen_text(
    width: u16,
    height: u16,
    drawing: impl FnOnce(&mut ratatui::Frame),
) -> String {
    let screen = test_screen(width, height, drawing);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| screen[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
