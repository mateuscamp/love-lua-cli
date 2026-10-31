//! O desenho da lista: os jogos à esquerda, os detalhes do escolhido à direita, a tela X no título
//! e o recado embaixo. Só desenha; o que mostrar chega pronto em `VistaLista`, e as cores, de
//! `tema`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::formato;
use super::tema::{self, apagado, atalho};
use crate::jogos::{Jogo, LANCADOR};
use crate::obra::Repo;

/// A largura dos rótulos dos detalhes (`lançador  `).
const ROTULO: usize = 10;
/// O nome mais curto que a coluna dos jogos considera, para o título dela caber.
const LARGURA_MINIMA: usize = 4;

/// O que se sabe de um jogo descoberto.
pub struct Situacao {
    pub jogo: Jogo,
    /// `None` com a pasta sem git, ou com o git que não respondeu.
    pub repo: Option<Repo>,
    /// A cópia do lançador comum difere da dos outros jogos (`lancador_comum`).
    pub destoa: bool,
}

/// A frase da última tentativa: o jogo que fechou, o que não abriu.
pub struct Recado {
    pub texto: String,
    /// Sai em coral: o jogo não abriu ou saiu com erro.
    pub problema: bool,
}

pub struct VistaLista<'a> {
    /// A raiz como o autor a escreve (`~/lua`).
    pub raiz: &'a str,
    /// Uma por jogo descoberto, em ordem; vazia quando a raiz não tem jogo.
    pub situacoes: &'a [Situacao],
    pub escolhido: usize,
    pub tela: &'a Result<(), String>,
    pub display: &'a str,
    pub recado: Option<&'a Recado>,
    /// Segundos Unix de agora, para o "há" do último commit.
    pub agora: i64,
}

pub fn desenha(frame: &mut Frame, vista: &VistaLista) {
    let area = frame.area();
    let fora = tema::bloco_de_fora()
        .title(tema::etiqueta(" jogar "))
        .title_top(titulo_da_tela(vista).right_aligned());
    let dentro = fora.inner(area);
    frame.render_widget(fora, area);
    let rodape = rodape(vista);
    let [paineis, baixo] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(rodape.len() as u16)])
            .areas(dentro);
    let largura = vista
        .situacoes
        .iter()
        .map(|situacao| situacao.jogo.pasta.chars().count())
        .max()
        .unwrap_or(0)
        // Na lista vazia, ou de nomes curtos, o título ` Jogos ` ainda cabe na moldura.
        .max(LARGURA_MINIMA) as u16
        + 8;
    let [esquerda, direita] =
        Layout::horizontal([Constraint::Length(largura), Constraint::Min(10)]).areas(paineis);
    desenha_jogos(frame, esquerda, vista);
    desenha_detalhes(frame, direita, vista);
    frame.render_widget(Paragraph::new(rodape), baixo);
    tema::moldura(frame.buffer_mut(), area);
}

fn titulo_da_tela(vista: &VistaLista) -> Line<'static> {
    match vista.tela {
        Ok(()) => Line::from(format!(" tela X {} ✓ ", vista.display)).fg(tema::CIANO),
        Err(_) => Line::from(" sem tela X ").fg(tema::CORAL).bold(),
    }
}

/// O aviso da tela, o recado e a ajuda das teclas, um por linha.
fn rodape(vista: &VistaLista) -> Vec<Line<'static>> {
    let mut linhas = Vec::new();
    if let Err(aviso) = vista.tela {
        linhas.push(Line::from(format!(" {aviso}")).fg(tema::CORAL));
    }
    if let Some(recado) = vista.recado {
        let cor = if recado.problema {
            tema::CORAL
        } else {
            tema::CIANO
        };
        linhas.push(Line::from(format!(" {}", recado.texto)).fg(cor));
    }
    linhas.push(Line::from(vec![
        Span::raw(" "),
        atalho("↑↓"),
        apagado(" escolhe   "),
        atalho("Enter"),
        apagado(" abre   "),
        atalho("Esc"),
        apagado(" sai"),
    ]));
    linhas
}

fn em_obra(situacao: &Situacao) -> bool {
    situacao
        .repo
        .as_ref()
        .is_some_and(|repo| repo.obra().is_some())
}

/// A lista dos jogos, com a barra em degradê na linha do escolhido.
fn desenha_jogos(frame: &mut Frame, area: Rect, vista: &VistaLista) {
    let itens: Vec<ListItem> = vista
        .situacoes
        .iter()
        .map(|situacao| {
            let mut partes = vec![Span::raw(situacao.jogo.pasta.clone())];
            if em_obra(situacao) {
                partes.push(" ●".fg(tema::ORQUIDEA));
            }
            if situacao.destoa {
                partes.push(" ≠".fg(tema::CORAL));
            }
            ListItem::new(Line::from(partes))
        })
        .collect();
    let bloco = tema::bloco_de_dentro(" Jogos ".fg(tema::APAGADO).bold());
    let dentro = bloco.inner(area);
    let lista = List::new(itens)
        .block(bloco)
        .highlight_symbol(Line::from("▶ ").fg(tema::MAGENTA))
        .highlight_style(Modifier::BOLD);
    let mut estado = ListState::default().with_selected(Some(vista.escolhido));
    frame.render_stateful_widget(lista, area, &mut estado);
    // A lista rola quando não cabe: a barra vai na linha em que o escolhido ficou.
    let linha = vista.escolhido.saturating_sub(estado.offset()) as u16;
    if linha < dentro.height {
        let faixa = Rect::new(dentro.x, dentro.y + linha, dentro.width, 1);
        tema::barra(frame.buffer_mut(), faixa);
    }
}

fn desenha_detalhes(frame: &mut Frame, area: Rect, vista: &VistaLista) {
    let Some(situacao) = vista.situacoes.get(vista.escolhido) else {
        desenha_vazio(frame, area, vista.raiz);
        return;
    };
    let jogo = &situacao.jogo;
    let mut lancador = vec![Span::raw(LANCADOR.join(" "))];
    if situacao.destoa {
        lancador.push(" ≠ difere dos outros jogos".fg(tema::CORAL));
    }
    let mut linhas = vec![
        rotulado("pasta", pasta(vista.raiz, jogo)),
        rotulado("lançador", lancador),
    ];
    linhas.extend(git(situacao.repo.as_ref(), vista.agora));
    let titulo = Span::raw(format!(" {} ", jogo.pasta))
        .fg(tema::MAGENTA)
        .bold();
    let bloco = tema::bloco_de_dentro(titulo);
    frame.render_widget(Paragraph::new(linhas).block(bloco), area);
}

/// Sem jogo na raiz: onde se procurou e o que faz uma pasta virar jogo.
fn desenha_vazio(frame: &mut Frame, area: Rect, raiz: &str) {
    let linhas = vec![
        Line::from(format!(" nenhum jogo em {raiz}")),
        Line::from(apagado(format!(
            " um jogo é uma pasta com {} executável",
            LANCADOR[0]
        ))),
    ];
    let bloco = tema::bloco_de_dentro(" jogar ".fg(tema::MAGENTA).bold());
    frame.render_widget(Paragraph::new(linhas).block(bloco), area);
}

fn rotulado(rotulo: &str, valor: Vec<Span<'static>>) -> Line<'static> {
    let mut partes = vec![apagado(format!(" {rotulo:<ROTULO$}"))];
    partes.extend(valor);
    Line::from(partes)
}

fn pasta(raiz: &str, jogo: &Jogo) -> Vec<Span<'static>> {
    vec![Span::raw(format!("{raiz}/{}", jogo.pasta))]
}

/// Branch, mudanças e último commit; o ramo principal em ciano, a obra em orquídea.
fn git(repo: Option<&Repo>, agora: i64) -> Vec<Line<'static>> {
    let Some(repo) = repo else {
        return vec![rotulado("branch", vec![apagado("— sem git")])];
    };
    let nome = match &repo.ramo {
        Some(ramo) => ramo.clone(),
        None => format!("HEAD solto em {}", repo.commit),
    };
    let ramo = if repo.desvio().is_some() {
        vec![Span::raw(nome), " ● em obra".fg(tema::ORQUIDEA)]
    } else {
        vec![Span::raw(nome).fg(tema::CIANO)]
    };
    let mudancas = match repo.mudancas_por_extenso() {
        Some(texto) => Span::raw(texto).fg(tema::ORQUIDEA),
        None => apagado("nenhuma"),
    };
    let mut linhas = vec![
        rotulado("branch", ramo),
        rotulado("mudanças", vec![mudancas]),
    ];
    linhas.extend(ultimo(repo, agora));
    linhas
}

fn ultimo(repo: &Repo, agora: i64) -> Vec<Line<'static>> {
    let Some(commit) = &repo.ultimo else {
        return vec![rotulado("último", vec![apagado("—")])];
    };
    let ha = formato::duracao_curta((agora - commit.quando).max(0) as u64);
    vec![
        rotulado(
            "último",
            vec![Span::raw(format!("{} · há {ha}", repo.commit))],
        ),
        rotulado("", vec![apagado(commit.assunto.clone())]),
    ]
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::obra::Commit;
    use crate::painel::{acha, tela_de_teste, tema, texto_da_tela};

    const AGORA: i64 = 1_793_200_000;

    fn situacoes() -> Vec<Situacao> {
        let limpo = Repo {
            ramo: Some("main".to_string()),
            principal: "main".to_string(),
            commit: "a988f4e".to_string(),
            mudancas: 0,
            ultimo: None,
        };
        let em_obra = Repo {
            ramo: Some("r0-pendencias".to_string()),
            commit: "48f0ad1".to_string(),
            mudancas: 5,
            ultimo: Some(Commit {
                quando: AGORA - 12 * 60,
                assunto: "O mostrado acompanha as passagens".to_string(),
            }),
            ..limpo.clone()
        };
        let jogo = |pasta: &str| Jogo {
            pasta: pasta.to_string(),
        };
        vec![
            Situacao {
                jogo: jogo("bomberboom"),
                repo: Some(limpo),
                destoa: false,
            },
            Situacao {
                jogo: jogo("boomlitude"),
                repo: Some(em_obra),
                destoa: false,
            },
            Situacao {
                jogo: jogo("campcrawler"),
                repo: None,
                destoa: true,
            },
        ]
    }

    fn vista<'a>(situacoes: &'a [Situacao], tela: &'a Result<(), String>) -> VistaLista<'a> {
        VistaLista {
            raiz: "~/lua",
            situacoes,
            escolhido: 1,
            tela,
            display: ":0",
            recado: None,
            agora: AGORA,
        }
    }

    #[test]
    fn lista_marca_a_obra_e_detalha_o_escolhido() {
        let situacoes = situacoes();
        let tela = Ok(());
        let texto = texto_da_tela(80, 14, |frame| desenha(frame, &vista(&situacoes, &tela)));
        for esperado in [
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
            assert!(texto.contains(esperado), "falta {esperado:?} em\n{texto}");
        }
        assert!(!texto.contains("bomberboom ●"), "{texto}");
        assert!(
            texto.contains("campcrawler ≠"),
            "o que destoa, na lista: {texto}"
        );
        assert!(!texto.contains("difere"), "o escolhido não destoa: {texto}");
    }

    #[test]
    fn lista_pinta_em_anil_e_magenta() {
        let situacoes = situacoes();
        let tela = Ok(());
        let pintada = tela_de_teste(80, 14, |frame| desenha(frame, &vista(&situacoes, &tela)));
        assert_eq!(pintada[(60, 9)].bg, tema::NOITE, "o fundo é a noite anil");
        let cantos = (pintada[(0, 0)].fg, pintada[(79, 13)].fg);
        assert_eq!(cantos, (tema::ANIL, tema::MAGENTA), "a moldura em degradê");
        let cor = |texto: &str| {
            let (x, y) = acha(&pintada, texto);
            (pintada[(x, y)].fg, pintada[(x, y)].bg)
        };
        assert_eq!(
            cor("jogar"),
            (tema::NOITE, tema::MAGENTA),
            "o título é etiqueta"
        );
        assert_eq!(cor("✓"), (tema::CIANO, tema::NOITE));
        assert_eq!(cor("▶ boomlitude"), (tema::MAGENTA, tema::BARRA.0));
        assert_eq!(cor("●").0, tema::ORQUIDEA, "a marca da obra na lista");
        assert_eq!(cor("● em obra").0, tema::ORQUIDEA);
        assert_eq!(cor("≠").0, tema::CORAL, "o lançador que destoa");
        assert_eq!(cor("Enter").0, tema::MAGENTA, "a tecla da ajuda");
    }

    /// No terminal baixo a lista rola, e a barra vai com o escolhido.
    #[test]
    fn a_barra_acompanha_a_lista_que_rolou() {
        let situacoes = situacoes();
        let tela = Ok(());
        let mut ultima = vista(&situacoes, &tela);
        ultima.escolhido = 2;
        let pintada = tela_de_teste(80, 7, |frame| desenha(frame, &ultima));
        let (x, y) = acha(&pintada, "▶ campcrawler");
        assert_eq!(pintada[(x, y)].bg, tema::BARRA.0);
    }

    /// O escolhido cujo lançador destoa diz isso nos detalhes, em coral.
    #[test]
    fn lista_sem_tela_avisa_e_mostra_o_lancador_que_destoa() {
        let situacoes = situacoes();
        let tela = Err("sem tela X: ninguém atende no DISPLAY=\":77\"".to_string());
        let recado = Recado {
            texto: "Boomlitude saiu com 1 depois de 00:03".to_string(),
            problema: true,
        };
        let mut vista = vista(&situacoes, &tela);
        vista.escolhido = 2;
        vista.recado = Some(&recado);
        let texto = texto_da_tela(100, 14, |frame| desenha(frame, &vista));
        for esperado in [
            "sem tela X",
            "ninguém atende no DISPLAY=\":77\"",
            "lançador  tools/love . ≠ difere dos outros jogos",
            "— sem git",
            "Boomlitude saiu com 1",
        ] {
            assert!(texto.contains(esperado), "falta {esperado:?} em\n{texto}");
        }
        assert!(!texto.contains("✓"), "{texto}");
    }

    /// Raiz sem jogo: os detalhes dizem onde se procurou e o que é um jogo.
    #[test]
    fn lista_vazia_diz_onde_procurou_e_o_que_e_um_jogo() {
        let tela = Ok(());
        let mut vazia = vista(&[], &tela);
        vazia.escolhido = 0;
        let texto = texto_da_tela(100, 12, |frame| desenha(frame, &vazia));
        for esperado in [
            "╭ Jogos ",
            "nenhum jogo em ~/lua",
            "um jogo é uma pasta com tools/love executável",
        ] {
            assert!(texto.contains(esperado), "falta {esperado:?} em\n{texto}");
        }
    }
}
