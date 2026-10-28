//! O desenho da partida, o "estado ao vivo" (autor, 28/10): uma linha por tipo de fato, atualizada
//! no lugar, com quantas vezes veio, há quanto tempo e o último valor; a linha que acabou de mudar
//! sai marcada. Não rola sozinho, então fica calmo mesmo com 50 fatos por segundo.

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table};

use super::formato;
use crate::fatos::{ERRO, Placar, TEXTO, Tipo};

/// Até quanto tempo depois de chegar a linha sai marcada.
const RECENTE: Duration = Duration::from_secs(1);
/// Quantos segundos a faísca do título cobre.
const JANELAS_DA_FAISCA: usize = 12;

pub struct VistaPartida<'a> {
    pub nome: &'a str,
    pub placar: &'a Placar,
    /// Desde que o jogo subiu.
    pub agora: Duration,
    /// Quantos tipos pular do alto, com as setas.
    pub rolagem: usize,
}

pub fn desenha(frame: &mut Frame, vista: &VistaPartida) {
    let titulo = format!(
        " {} · jogando há {} ",
        vista.nome,
        formato::relogio(vista.agora)
    );
    let ritmo = formato::faisca(&vista.placar.ritmo(vista.agora, JANELAS_DA_FAISCA));
    let contagem = format!(" {} fatos {ritmo} ", vista.placar.total());
    let fora = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(titulo).bold())
        .title_top(Line::from(contagem).right_aligned());
    let dentro = fora.inner(frame.area());
    frame.render_widget(fora, frame.area());
    let rodape = rodape(vista.placar);
    let [meio, baixo] =
        Layout::vertical([Constraint::Min(2), Constraint::Length(rodape.len() as u16)])
            .areas(dentro);
    frame.render_widget(tabela(vista), meio);
    frame.render_widget(Paragraph::new(rodape), baixo);
}

fn tabela(vista: &VistaPartida) -> Table<'static> {
    let cabecalho =
        Row::new([" TIPO", "VEZES", "HÁ", "O ÚLTIMO"]).style(Style::new().bold().dark_gray());
    let linhas = vista
        .placar
        .tipos()
        .iter()
        .skip(vista.rolagem)
        .map(|tipo| linha(tipo, vista))
        .collect::<Vec<_>>();
    let larguras = [
        Constraint::Length(13),
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Min(10),
    ];
    Table::new(linhas, larguras)
        .header(cabecalho)
        .column_spacing(1)
}

/// A linha de um tipo: marcada com `▸` se mudou há menos de `RECENTE`.
fn linha(tipo: &Tipo, vista: &VistaPartida) -> Row<'static> {
    let desde = vista.agora.saturating_sub(tipo.ultimo_em);
    let marca = if desde < RECENTE { "▸" } else { " " };
    let ultimo = if tipo.nome == "CONFIG" {
        config(vista.placar)
    } else {
        formato::resumo(&tipo.campos)
    };
    let celulas = [
        Cell::from(format!("{marca}{}", tipo.nome)),
        Cell::from(Line::from(tipo.vezes.to_string()).right_aligned()),
        Cell::from(formato::duracao_curta(desde.as_secs())),
        Cell::from(ultimo),
    ];
    Row::new(celulas).style(estilo(tipo, desde))
}

fn estilo(tipo: &Tipo, desde: Duration) -> Style {
    match tipo.nome.as_str() {
        ERRO => Style::new().red(),
        TEXTO => Style::new().dark_gray(),
        _ if desde < RECENTE => Style::new().bold(),
        _ => Style::new(),
    }
}

/// O `CONFIG` vem uma vez por chave no boot; o que interessa é o que saiu do padrão.
fn config(placar: &Placar) -> String {
    let fora = placar.fora_do_padrao();
    if fora.is_empty() {
        return "todas no padrão".to_string();
    }
    let pares: Vec<String> = fora
        .iter()
        .map(|(chave, valor)| format!("{chave} {valor}"))
        .collect();
    format!("{} fora do padrão: {}", fora.len(), pares.join(", "))
}

fn rodape(placar: &Placar) -> Vec<Line<'static>> {
    let mut linhas = Vec::new();
    if let Some(erro) = placar.primeiro_erro() {
        linhas.push(Line::from(format!(" erro: {erro}")).red());
    }
    let ajuda = " x encerra o jogo · ↑↓ rola · o painel volta quando a janela fechar";
    linhas.push(Line::from(ajuda).dark_gray());
    linhas
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::fatos::Origem;
    use crate::painel::texto_da_tela;

    fn placar() -> Placar {
        let mut placar = Placar::novo("CampCrawler");
        let linhas = [
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
        for (segundos, linha) in linhas {
            placar.anota(Origem::Saida, linha, Duration::from_secs_f64(segundos));
        }
        placar
    }

    fn vista(placar: &Placar) -> VistaPartida<'_> {
        VistaPartida {
            nome: "CampCrawler",
            placar,
            agora: Duration::from_secs_f64(134.2),
            rolagem: 0,
        }
    }

    #[test]
    fn partida_mostra_cada_tipo_e_marca_o_que_acabou_de_mudar() {
        let placar = placar();
        let texto = texto_da_tela(80, 12, |frame| desenha(frame, &vista(&placar)));
        for esperado in [
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
            assert!(texto.contains(esperado), "falta {esperado:?} em\n{texto}");
        }
        assert!(
            !texto.contains("▸MAPA"),
            "o MAPA não muda há 2 min\n{texto}"
        );
        let vez = texto.lines().find(|linha| linha.contains("▸VEZ")).unwrap();
        assert!(vez.contains(" 2 ") && vez.contains("0s"), "{vez}");
    }

    #[test]
    fn partida_rola_e_mostra_o_primeiro_erro() {
        let mut placar = placar();
        placar.anota(
            Origem::Erro,
            "Error: Could not initialize SDL video subsystem",
            Duration::from_secs(134),
        );
        let mut vista = vista(&placar);
        vista.rolagem = 1;
        let texto = texto_da_tela(80, 12, |frame| desenha(frame, &vista));
        assert!(!texto.contains("PARTIDA"), "rolou um\n{texto}");
        assert!(texto.contains("CONFIG"), "{texto}");
        assert!(
            texto.contains("erro: Error: Could not initialize SDL"),
            "{texto}"
        );
    }
}
