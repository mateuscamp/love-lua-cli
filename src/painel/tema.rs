//! As cores do painel, anil e magenta (autor, 30/10): a noite anil no fundo, o magenta no que pede
//! o olho, e o degradê de um ao outro na moldura de fora, na barra do escolhido e na faísca. Só cor
//! e pintura; o que desenhar vem pronto de `lista.rs` e `partida.rs`.

use std::borrow::Cow;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType};

/// O fundo de tudo: anil quase preto.
pub const NOITE: Color = Color::Rgb(17, 13, 43);
/// As molduras de dentro e o começo dos degradês.
pub const ANIL: Color = Color::Rgb(75, 63, 196);
/// Rótulos, ajuda, o que é secundário.
pub const APAGADO: Color = Color::Rgb(122, 113, 204);
/// O texto.
pub const LAVANDA: Color = Color::Rgb(226, 222, 255);
/// O destaque: o título, a tecla, a marca do que acabou de mudar, o fim dos degradês.
pub const MAGENTA: Color = Color::Rgb(255, 46, 160);
/// A obra: pasta fora do ramo principal ou com mudança.
pub const ORQUIDEA: Color = Color::Rgb(214, 120, 255);
/// Tudo certo: a tela X, o ramo principal, o jogo que fechou bem.
pub const CIANO: Color = Color::Rgb(98, 214, 255);
/// Problema: sem tela, pasta que falta, jogo que morreu, erro.
pub const CORAL: Color = Color::Rgb(255, 92, 122);
/// O fundo da barra do escolhido, do magenta escuro, na esquerda, ao anil escuro.
pub const BARRA: (Color, Color) = (Color::Rgb(112, 24, 122), Color::Rgb(38, 30, 98));

/// Os traços da moldura arredondada; o resto da borda é título.
const TRACOS: [&str; 6] = ["─", "│", "╭", "╮", "╰", "╯"];

/// A cor a `fracao` (de 0 a 1) do caminho de `de` a `ate`. Cor que não é RGB fica `de`.
pub fn degrade(de: Color, ate: Color, fracao: f32) -> Color {
    let (Color::Rgb(r0, g0, b0), Color::Rgb(r1, g1, b1)) = (de, ate) else {
        return de;
    };
    let fracao = fracao.clamp(0.0, 1.0);
    let mistura =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * fracao).round() as u8;
    Color::Rgb(mistura(r0, r1), mistura(g0, g1), mistura(b0, b1))
}

/// Onde `x` cai na largura de `area`: 0 na primeira coluna, 1 na última.
fn posicao_na_largura(area: Rect, x: u16) -> f32 {
    let ultima = area.width.saturating_sub(1).max(1);
    f32::from(x.saturating_sub(area.x)) / f32::from(ultima)
}

/// Pinta os traços da moldura de `area` do `ANIL`, na esquerda, ao `MAGENTA`, na direita. O título
/// que está na moldura fica com a cor dele.
pub fn moldura(tela: &mut Buffer, area: Rect) {
    let area = area.intersection(tela.area);
    if area.is_empty() {
        return;
    }
    let topo_e_base =
        (area.left()..area.right()).flat_map(|x| [(x, area.top()), (x, area.bottom() - 1)]);
    let lados = (area.top()..area.bottom()).flat_map(|y| [(area.left(), y), (area.right() - 1, y)]);
    for (x, y) in topo_e_base.chain(lados) {
        let celula = &mut tela[(x, y)];
        if TRACOS.contains(&celula.symbol()) {
            celula.set_fg(degrade(ANIL, MAGENTA, posicao_na_largura(area, x)));
        }
    }
}

/// Pinta o fundo de `linha` com o degradê da `BARRA`, sem mexer no texto nem na cor dele.
pub fn barra(tela: &mut Buffer, linha: Rect) {
    let linha = linha.intersection(tela.area);
    for x in linha.left()..linha.right() {
        tela[(x, linha.y)].set_bg(degrade(BARRA.0, BARRA.1, posicao_na_largura(linha, x)));
    }
}

/// A moldura de fora, que pinta o fundo de noite e o texto de lavanda. Os traços ganham o degradê
/// com `moldura`, depois que o resto já está desenhado.
pub fn bloco_de_fora<'a>() -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .style(Style::new().fg(LAVANDA).bg(NOITE))
}

/// Uma moldura de dentro, em anil, com o `titulo` como ele vier pintado.
pub fn bloco_de_dentro(titulo: Span) -> Block {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ANIL))
        .title(titulo)
}

/// O nome em etiqueta: a noite sobre o magenta, no título da moldura de fora.
pub fn etiqueta<'a>(texto: impl Into<Cow<'a, str>>) -> Span<'a> {
    Span::raw(texto).fg(NOITE).bg(MAGENTA).bold()
}

/// A tecla na ajuda do rodapé.
pub fn atalho(texto: &'static str) -> Span<'static> {
    Span::raw(texto).fg(MAGENTA).bold()
}

/// O texto secundário: rótulo, ajuda, assunto do commit.
pub fn apagado<'a>(texto: impl Into<Cow<'a, str>>) -> Span<'a> {
    Span::raw(texto).fg(APAGADO)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::painel::{acha, tela_de_teste};

    #[test]
    fn degrade_vai_de_uma_cor_a_outra() {
        let (preto, branco) = (Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255));
        assert_eq!(degrade(preto, branco, 0.0), preto);
        assert_eq!(degrade(preto, branco, 1.0), branco);
        assert_eq!(degrade(preto, branco, 0.5), Color::Rgb(128, 128, 128));
        assert_eq!(degrade(preto, branco, 7.0), branco, "fração além de 1");
        assert_eq!(degrade(Color::Reset, branco, 0.5), Color::Reset);
    }

    #[test]
    fn moldura_pinta_os_tracos_e_deixa_o_titulo() {
        let tela = tela_de_teste(20, 3, |frame| {
            let bloco = Block::bordered()
                .border_type(BorderType::Rounded)
                .title(Span::raw(" ab ").fg(CIANO));
            let area = frame.area();
            frame.render_widget(bloco, area);
            moldura(frame.buffer_mut(), area);
        });
        assert_eq!(tela[(0, 0)].fg, ANIL);
        assert_eq!(tela[(19, 2)].fg, MAGENTA);
        assert_eq!(tela[(19, 1)].fg, MAGENTA, "o lado direito, todo magenta");
        let meio = tela[(10, 2)].fg;
        assert!(
            meio != ANIL && meio != MAGENTA,
            "o meio é mistura: {meio:?}"
        );
        let (x, y) = acha(&tela, "ab");
        assert_eq!(tela[(x, y)].fg, CIANO, "o título ficou com a cor dele");
    }

    #[test]
    fn barra_pinta_o_fundo_e_deixa_o_texto() {
        let tela = tela_de_teste(10, 1, |frame| {
            let area = frame.area();
            frame.render_widget(Span::raw("oi").fg(CIANO), area);
            barra(frame.buffer_mut(), area);
        });
        assert_eq!((tela[(0, 0)].bg, tela[(9, 0)].bg), BARRA);
        assert_eq!((tela[(0, 0)].symbol(), tela[(0, 0)].fg), ("o", CIANO));
    }

    #[test]
    fn area_vazia_nao_pinta_nem_quebra() {
        let tela = tela_de_teste(4, 2, |frame| {
            moldura(frame.buffer_mut(), Rect::new(0, 0, 0, 0));
            moldura(frame.buffer_mut(), Rect::new(9, 9, 3, 3));
        });
        assert_eq!(tela[(0, 0)].fg, Color::Reset);
    }

    #[test]
    fn uma_coluna_so_nao_divide_por_zero() {
        let tela = tela_de_teste(1, 1, |frame| {
            let area = frame.area();
            barra(frame.buffer_mut(), area);
        });
        assert_eq!(tela[(0, 0)].bg, BARRA.0);
    }
}
