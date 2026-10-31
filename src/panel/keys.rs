//! The key that arrived, as a panel action. What each action does depends on the view (the list or
//! the running game) and lives in the loop; here the key is only translated.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    MoveUp,
    MoveDown,
    Open,
    Quit,
    /// `x`: stops the game that is running.
    StopGame,
    /// Ctrl+C: in raw mode it arrives as a key, not as a signal.
    Interrupt,
    Nothing,
}

/// The key's action. Only the pressed key counts (the terminal may send the released one too).
pub fn action_for(key: KeyEvent) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::Nothing;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if ctrl => Action::Interrupt,
        _ if ctrl => Action::Nothing,
        KeyCode::Up | KeyCode::Char('k') => Action::MoveUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MoveDown,
        KeyCode::Enter => Action::Open,
        KeyCode::Esc | KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('x') => Action::StopGame,
        _ => Action::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn action_for_translates_arrows_vim_enter_esc_q_x_and_ctrl_c() {
        let cases = [
            (key(KeyCode::Up), Action::MoveUp),
            (key(KeyCode::Char('k')), Action::MoveUp),
            (key(KeyCode::Down), Action::MoveDown),
            (key(KeyCode::Char('j')), Action::MoveDown),
            (key(KeyCode::Enter), Action::Open),
            (key(KeyCode::Esc), Action::Quit),
            (key(KeyCode::Char('q')), Action::Quit),
            (key(KeyCode::Char('x')), Action::StopGame),
            (
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Action::Interrupt,
            ),
            (key(KeyCode::Char('c')), Action::Nothing),
            (
                KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release),
                Action::Nothing,
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(action_for(event), expected, "{event:?}");
        }
    }
}
