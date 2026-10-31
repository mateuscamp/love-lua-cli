//! Ajudantes dos testes de desenho do painel: a tela do `TestBackend` e a leitura dela, por texto e
//! por cor. Saíram do `mod.rs` quando ele passou do teto de 500 linhas (31/10).

/// A tela que `desenho` pinta, de `largura` x `altura`, para os testes lerem texto e cor.
pub(crate) fn tela_de_teste(
    largura: u16,
    altura: u16,
    desenho: impl FnOnce(&mut ratatui::Frame),
) -> ratatui::buffer::Buffer {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(largura, altura)).unwrap();
    terminal.draw(desenho).unwrap();
    terminal.backend().buffer().clone()
}

/// A coluna e a linha onde `texto` começa na `tela`, para o teste olhar a cor dali.
pub(crate) fn acha(tela: &ratatui::buffer::Buffer, texto: &str) -> (u16, u16) {
    let area = tela.area;
    let letras: Vec<String> = texto.chars().map(String::from).collect();
    let cabe = |x: u16, y: u16| {
        letras.iter().enumerate().all(|(i, letra)| {
            let coluna = x + i as u16;
            coluna < area.right() && tela[(coluna, y)].symbol() == letra
        })
    };
    (area.top()..area.bottom())
        .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
        .find(|&(x, y)| cabe(x, y))
        .unwrap_or_else(|| panic!("{texto:?} não está na tela"))
}

/// O que `desenho` põe numa tela de `largura` x `altura`, linha a linha, para os testes lerem.
pub(crate) fn texto_da_tela(
    largura: u16,
    altura: u16,
    desenho: impl FnOnce(&mut ratatui::Frame),
) -> String {
    let tela = tela_de_teste(largura, altura, desenho);
    (0..altura)
        .map(|y| {
            (0..largura)
                .map(|x| tela[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
