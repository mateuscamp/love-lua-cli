//! A tecla que chegou, como ação do painel. O que cada ação faz depende da vista (lista ou
//! partida) e mora no laço; aqui só se traduz a tecla.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Acao {
    Sobe,
    Desce,
    Abre,
    Sai,
    /// `x`: encerra o jogo que está rodando.
    Encerra,
    /// Ctrl+C: em modo cru ele chega como tecla, não como sinal.
    Interrompe,
    Nada,
}

/// A ação da tecla. Só a tecla apertada conta (o terminal pode mandar a solta também).
pub fn acao(tecla: KeyEvent) -> Acao {
    if tecla.kind != KeyEventKind::Press {
        return Acao::Nada;
    }
    let ctrl = tecla.modifiers.contains(KeyModifiers::CONTROL);
    match tecla.code {
        KeyCode::Char('c') if ctrl => Acao::Interrompe,
        _ if ctrl => Acao::Nada,
        KeyCode::Up | KeyCode::Char('k') => Acao::Sobe,
        KeyCode::Down | KeyCode::Char('j') => Acao::Desce,
        KeyCode::Enter => Acao::Abre,
        KeyCode::Esc | KeyCode::Char('q') => Acao::Sai,
        KeyCode::Char('x') => Acao::Encerra,
        _ => Acao::Nada,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn tecla(codigo: KeyCode) -> KeyEvent {
        KeyEvent::new(codigo, KeyModifiers::NONE)
    }

    #[test]
    fn acao_traduz_setas_vim_enter_esc_q_x_e_ctrl_c() {
        let casos = [
            (tecla(KeyCode::Up), Acao::Sobe),
            (tecla(KeyCode::Char('k')), Acao::Sobe),
            (tecla(KeyCode::Down), Acao::Desce),
            (tecla(KeyCode::Char('j')), Acao::Desce),
            (tecla(KeyCode::Enter), Acao::Abre),
            (tecla(KeyCode::Esc), Acao::Sai),
            (tecla(KeyCode::Char('q')), Acao::Sai),
            (tecla(KeyCode::Char('x')), Acao::Encerra),
            (
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                Acao::Interrompe,
            ),
            (tecla(KeyCode::Char('c')), Acao::Nada),
            (
                KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release),
                Acao::Nada,
            ),
        ];
        for (evento, esperado) in casos {
            assert_eq!(acao(evento), esperado, "{evento:?}");
        }
    }
}
