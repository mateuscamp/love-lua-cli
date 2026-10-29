//! O desenho da lista: os jogos à esquerda, os detalhes do escolhido à direita, a tela X no título
//! e o recado embaixo. Só desenha; o que mostrar chega pronto em `VistaLista`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, Paragraph};

use super::formato;
use crate::jogos::{JOGOS, Jogo};
use crate::obra::Repo;

/// A largura dos rótulos dos detalhes (`lançador  `).
const ROTULO: usize = 10;

/// O que se sabe da pasta de um jogo.
pub struct Situacao {
    pub existe: bool,
    /// `None` com a pasta sem git, ou com o git que não respondeu.
    pub repo: Option<Repo>,
}

/// A frase da última tentativa: o jogo que fechou, o que não abriu.
pub struct Recado {
    pub texto: String,
    /// Sai em vermelho: o jogo não abriu ou saiu com erro.
    pub problema: bool,
}

pub struct VistaLista<'a> {
    /// A raiz como o autor a escreve (`~/lua`).
    pub raiz: &'a str,
    /// Uma por jogo, na ordem de `JOGOS`.
    pub situacoes: &'a [Situacao],
    pub escolhido: usize,
    pub tela: &'a Result<(), String>,
    pub display: &'a str,
    pub recado: Option<&'a Recado>,
    /// Segundos Unix de agora, para o "há" do último commit.
    pub agora: i64,
}

pub fn desenha(frame: &mut Frame, vista: &VistaLista) {
    let fora = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(" jogar ").bold())
        .title_top(titulo_da_tela(vista).right_aligned());
    let dentro = fora.inner(frame.area());
    frame.render_widget(fora, frame.area());
    let rodape = rodape(vista);
    let [paineis, baixo] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(rodape.len() as u16)])
            .areas(dentro);
    let largura = JOGOS.iter().map(|jogo| jogo.nome.len()).max().unwrap_or(0) as u16 + 8;
    let [esquerda, direita] =
        Layout::horizontal([Constraint::Length(largura), Constraint::Min(10)]).areas(paineis);
    desenha_jogos(frame, esquerda, vista);
    desenha_detalhes(frame, direita, vista);
    frame.render_widget(Paragraph::new(rodape), baixo);
}

fn titulo_da_tela(vista: &VistaLista) -> Line<'static> {
    match vista.tela {
        Ok(()) => Line::from(format!(" tela X {} ✓ ", vista.display)).green(),
        Err(_) => Line::from(" sem tela X ").red().bold(),
    }
}

/// O aviso da tela, o recado e a ajuda das teclas, um por linha.
fn rodape(vista: &VistaLista) -> Vec<Line<'static>> {
    let mut linhas = Vec::new();
    if let Err(aviso) = vista.tela {
        linhas.push(Line::from(format!(" {aviso}")).red());
    }
    if let Some(recado) = vista.recado {
        let linha = Line::from(format!(" {}", recado.texto));
        linhas.push(if recado.problema {
            linha.red()
        } else {
            linha.green()
        });
    }
    linhas.push(Line::from(" ↑↓ escolhe   Enter abre   Esc sai").dark_gray());
    linhas
}

fn em_obra(situacao: &Situacao) -> bool {
    situacao
        .repo
        .as_ref()
        .is_some_and(|repo| repo.obra().is_some())
}

fn desenha_jogos(frame: &mut Frame, area: Rect, vista: &VistaLista) {
    let itens: Vec<ListItem> = JOGOS
        .iter()
        .zip(vista.situacoes)
        .map(|(jogo, situacao)| {
            let mut partes = vec![Span::raw(jogo.nome)];
            if em_obra(situacao) {
                partes.push(" ●".yellow());
            }
            if !situacao.existe {
                partes.push(" ✗".red());
            }
            ListItem::new(Line::from(partes))
        })
        .collect();
    let lista = List::new(itens)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(" Jogos "),
        )
        .highlight_symbol("▶ ")
        .highlight_style(Modifier::BOLD);
    let mut estado = ListState::default().with_selected(Some(vista.escolhido));
    frame.render_stateful_widget(lista, area, &mut estado);
}

fn desenha_detalhes(frame: &mut Frame, area: Rect, vista: &VistaLista) {
    let jogo = &JOGOS[vista.escolhido];
    let situacao = &vista.situacoes[vista.escolhido];
    let mut linhas = vec![
        rotulado("pasta", pasta(vista.raiz, jogo, situacao)),
        rotulado("lançador", vec![Span::raw(jogo.lancador.join(" "))]),
    ];
    if situacao.existe {
        linhas.extend(git(situacao.repo.as_ref(), vista.agora));
    }
    let bloco = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", jogo.nome));
    frame.render_widget(Paragraph::new(linhas).block(bloco), area);
}

fn rotulado(rotulo: &str, valor: Vec<Span<'static>>) -> Line<'static> {
    let mut partes = vec![Span::raw(format!(" {rotulo:<ROTULO$}")).dark_gray()];
    partes.extend(valor);
    Line::from(partes)
}

fn pasta(raiz: &str, jogo: &Jogo, situacao: &Situacao) -> Vec<Span<'static>> {
    let caminho = Span::raw(format!("{raiz}/{}", jogo.pasta));
    if situacao.existe {
        vec![caminho]
    } else {
        vec![caminho, " (não existe)".red()]
    }
}

/// Branch, mudanças e último commit; a obra em amarelo.
fn git(repo: Option<&Repo>, agora: i64) -> Vec<Line<'static>> {
    let Some(repo) = repo else {
        return vec![rotulado("branch", vec!["— sem git".dark_gray()])];
    };
    let nome = match &repo.ramo {
        Some(ramo) => ramo.clone(),
        None => format!("HEAD solto em {}", repo.commit),
    };
    let ramo = if repo.desvio().is_some() {
        vec![Span::raw(nome), " ● em obra".yellow()]
    } else {
        vec![Span::raw(nome).green()]
    };
    let mudancas = match repo.mudancas_por_extenso() {
        Some(texto) => Span::raw(texto).yellow(),
        None => "nenhuma".dark_gray(),
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
        return vec![rotulado("último", vec!["—".dark_gray()])];
    };
    let ha = formato::duracao_curta((agora - commit.quando).max(0) as u64);
    vec![
        rotulado(
            "último",
            vec![Span::raw(format!("{} · há {ha}", repo.commit))],
        ),
        rotulado("", vec![Span::raw(commit.assunto.clone()).dark_gray()]),
    ]
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::obra::Commit;
    use crate::painel::texto_da_tela;

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
        vec![
            Situacao {
                existe: true,
                repo: Some(limpo),
            },
            Situacao {
                existe: true,
                repo: Some(em_obra),
            },
            Situacao {
                existe: false,
                repo: None,
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
            "▶ Boomlitude ●",
            "BomberBoom ",
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
        assert!(!texto.contains("BomberBoom ●"), "{texto}");
    }

    #[test]
    fn lista_sem_tela_avisa_e_mostra_a_pasta_que_falta() {
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
            "~/lua/campcrawler (não existe)",
            "Boomlitude saiu com 1",
        ] {
            assert!(texto.contains(esperado), "falta {esperado:?} em\n{texto}");
        }
        assert!(!texto.contains("✓"), "{texto}");
    }
}
