//! O painel em tela cheia (autor, 28/10): a lista dos jogos com os detalhes do escolhido e,
//! enquanto um jogo roda na janela dele, o estado ao vivo do que ele escreve. Este arquivo é o laço
//! e a lista, o único que fala com o terminal; o jogo rodando mora em `processo`, os sinais em
//! `sinais`, o desenho em `lista` e `partida`, as teclas em `teclas`.
//!
//! O jogo sobe sem terminal (a saída num cano), então o painel fica na tela o tempo todo: não há
//! suspender e retomar o modo cru. Quando a janela do jogo fecha, a lista volta com um recado.

mod formato;
mod lista;
mod partida;
mod processo;
mod sinais;
mod teclas;

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEvent};

use crate::jogos::{JOGOS, Jogo};
use crate::{obra, tela};
use lista::{Recado, Situacao, VistaLista};
use processo::Partida;
use sinais::{anota_sinais_de_saida, saida_por_sinal};
use teclas::Acao;

/// De quanto em quanto o painel acorda sem tecla, para o placar andar.
const PASSO: Duration = Duration::from_millis(100);
/// De quanto em quanto a lista relê o git das pastas e a tela X.
const RELEITURA: Duration = Duration::from_secs(2);

/// Abre o painel e só volta quando o autor sai: 0 com Esc ou q, 130 com Ctrl+C, 128 + sinal com
/// SIGHUP ou SIGTERM, 2 sem terminal.
pub fn menu(raiz: &Path) -> ExitCode {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("jogar: o menu precisa de um terminal; sem ele, jogar <jogo> abre direto");
        return ExitCode::from(2);
    }
    anota_sinais_de_saida();
    let mut terminal = ratatui::init();
    let saida = laco(&mut terminal, raiz);
    ratatui::restore();
    saida.unwrap_or_else(|erro| {
        eprintln!("jogar: o terminal falhou: {erro}");
        ExitCode::from(2)
    })
}

/// O laço sai pelo `return`, e a `partida` cai com ele: com um jogo aberto, o `Drop` dela o fecha.
fn laco(terminal: &mut DefaultTerminal, raiz: &Path) -> io::Result<ExitCode> {
    let mut lista = Lista::nova(raiz);
    let mut partida: Option<Partida> = None;
    loop {
        if let Some(saida) = saida_por_sinal() {
            return Ok(saida);
        }
        acompanha(&mut lista, &mut partida);
        terminal.draw(|frame| match &partida {
            Some(jogando) => partida::desenha(frame, &jogando.vista()),
            None => lista::desenha(frame, &lista.vista()),
        })?;
        let Some(tecla) = proxima_tecla()? else {
            continue;
        };
        if let Some(saida) = reage(teclas::acao(tecla), &mut lista, &mut partida) {
            return Ok(saida);
        }
    }
}

/// A tecla que chegar em até `PASSO`. Um sinal que interrompe a espera (o SIGHUP do terminal que
/// fecha) não é erro: o laço volta e vê o sinal anotado.
fn proxima_tecla() -> io::Result<Option<KeyEvent>> {
    let interrompido = |erro: &io::Error| erro.kind() == io::ErrorKind::Interrupted;
    match event::poll(PASSO) {
        Ok(true) => {}
        Ok(false) => return Ok(None),
        Err(erro) if interrompido(&erro) => return Ok(None),
        Err(erro) => return Err(erro),
    }
    match event::read() {
        Ok(Event::Key(tecla)) => Ok(Some(tecla)),
        Ok(_) => Ok(None),
        Err(erro) if interrompido(&erro) => Ok(None),
        Err(erro) => Err(erro),
    }
}

/// O que anda sem tecla: o placar recolhe o que o jogo escreveu e, quando ele fecha, a lista volta
/// com o recado; sem jogo, a lista relê o git e a tela de tempos em tempos.
fn acompanha(lista: &mut Lista, partida: &mut Option<Partida>) {
    let Some(jogando) = partida else {
        lista.le_se_velha();
        return;
    };
    jogando.recolhe();
    if let Some(recado) = jogando.fim() {
        *partida = None;
        lista.le();
        lista.recado = Some(recado);
    }
}

/// A ação no contexto. Na lista, setas escolhem, Enter abre e Esc ou q saem; na partida, setas
/// rolam e `x` encerra o jogo. Ctrl+C sai de qualquer lugar, fechando o jogo antes. `Some` é sair.
fn reage(acao: Acao, lista: &mut Lista, partida: &mut Option<Partida>) -> Option<ExitCode> {
    match (acao, partida.as_mut()) {
        (Acao::Interrompe, jogando) => {
            jogando.map(Partida::encerra_e_espera);
            return Some(ExitCode::from(130));
        }
        (Acao::Sobe, Some(jogando)) => jogando.rola(-1),
        (Acao::Desce, Some(jogando)) => jogando.rola(1),
        (Acao::Encerra, Some(jogando)) => jogando.encerra(),
        (_, Some(_)) => {}
        (Acao::Sobe, None) => lista.move_escolha(-1),
        (Acao::Desce, None) => lista.move_escolha(1),
        (Acao::Abre, None) => *partida = lista.abre(),
        (Acao::Sai, None) => return Some(ExitCode::SUCCESS),
        (_, None) => {}
    }
    None
}

/// A lista: o que se sabe de cada jogo e o que o autor escolheu.
struct Lista {
    raiz: PathBuf,
    raiz_mostrada: String,
    escolhido: usize,
    situacoes: Vec<Situacao>,
    tela: Result<(), String>,
    display: String,
    recado: Option<Recado>,
    lida: Instant,
}

impl Lista {
    fn nova(raiz: &Path) -> Lista {
        let mut lista = Lista::vazia(raiz);
        lista.le();
        lista
    }

    /// A lista antes de ler o git e a tela X: os testes partem dela, sem tocar o X do autor.
    fn vazia(raiz: &Path) -> Lista {
        Lista {
            raiz: raiz.to_path_buf(),
            raiz_mostrada: mostra_raiz(raiz),
            escolhido: 0,
            situacoes: Vec::new(),
            tela: Ok(()),
            display: String::new(),
            recado: None,
            lida: Instant::now(),
        }
    }

    /// Relê o git de cada pasta e a tela X (uns 15 ms para os três jogos).
    fn le(&mut self) {
        self.situacoes = JOGOS
            .iter()
            .map(|jogo| situacao(&self.raiz, jogo))
            .collect();
        self.tela = tela::tela();
        self.display = std::env::var("DISPLAY").unwrap_or_default();
        self.lida = Instant::now();
    }

    fn le_se_velha(&mut self) {
        if self.lida.elapsed() >= RELEITURA {
            self.le();
        }
    }

    /// Anda na lista, dando a volta nas pontas.
    fn move_escolha(&mut self, passo: isize) {
        let total = JOGOS.len() as isize;
        self.escolhido = (self.escolhido as isize + passo).rem_euclid(total) as usize;
    }

    /// Sobe o jogo escolhido; sem tela X ou se ele não sobe, fica o recado e nada abre. O aviso da
    /// tela já está no rodapé, então o recado não o repete.
    fn abre(&mut self) -> Option<Partida> {
        if self.tela.is_err() {
            self.recado = Some(Recado {
                texto: format!("{} não abre sem tela X", JOGOS[self.escolhido].nome),
                problema: true,
            });
            return None;
        }
        match Partida::sobe(&self.raiz, &JOGOS[self.escolhido]) {
            Ok(jogando) => {
                self.recado = None;
                Some(jogando)
            }
            Err(erro) => {
                self.recado = Some(Recado {
                    texto: erro,
                    problema: true,
                });
                None
            }
        }
    }

    fn vista(&self) -> VistaLista<'_> {
        VistaLista {
            raiz: &self.raiz_mostrada,
            situacoes: &self.situacoes,
            escolhido: self.escolhido,
            tela: &self.tela,
            display: &self.display,
            recado: self.recado.as_ref(),
            agora: segundos_unix(),
        }
    }
}

fn situacao(raiz: &Path, jogo: &Jogo) -> Situacao {
    let pasta = raiz.join(jogo.pasta);
    Situacao {
        existe: pasta.is_dir(),
        repo: obra::repo(&pasta),
    }
}

/// A raiz com o `HOME` escrito como `~`, como o autor a escreve.
fn mostra_raiz(raiz: &Path) -> String {
    let casa = std::env::var_os("HOME").map(PathBuf::from);
    match casa.and_then(|casa| raiz.strip_prefix(casa).ok().map(Path::to_path_buf)) {
        Some(resto) => format!("~/{}", resto.display()),
        None => raiz.display().to_string(),
    }
}

fn segundos_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |desde| desde.as_secs() as i64)
}

/// O que `desenho` põe numa tela de `largura` x `altura`, linha a linha, para os testes lerem.
#[cfg(test)]
pub(crate) fn texto_da_tela(
    largura: u16,
    altura: u16,
    desenho: impl FnOnce(&mut ratatui::Frame),
) -> String {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(largura, altura)).unwrap();
    terminal.draw(desenho).unwrap();
    let tela = terminal.backend().buffer();
    (0..altura)
        .map(|y| {
            (0..largura)
                .map(|x| tela[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_de_teste::{
        LANCADOR_COM_NETO, espera_pid, jogo_falso, morre_em, pasta_de_teste,
    };
    use std::thread;

    // Auditoria de 28/10, achado A2: o laço do painel sem terminal. As teclas entram pelo `reage`,
    // o tempo pelo `acompanha`, e os jogos são de mentira.

    /// Roda o `acompanha` até o jogo fechar e a lista voltar.
    fn espera_o_fim(lista: &mut Lista, partida: &mut Option<Partida>) {
        let prazo = Instant::now() + Duration::from_secs(5);
        while partida.is_some() {
            assert!(Instant::now() < prazo, "o jogo de mentira não fechou");
            acompanha(lista, partida);
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn recado(lista: &Lista) -> (&str, bool) {
        let recado = lista.recado.as_ref().expect("a lista ficou sem recado");
        (recado.texto.as_str(), recado.problema)
    }

    #[test]
    fn na_lista_as_setas_dao_a_volta_e_esc_ou_ctrl_c_saem() {
        let raiz = pasta_de_teste("lista-teclas");
        let mut lista = Lista::vazia(&raiz);
        let mut partida = None;
        assert_eq!(reage(Acao::Sobe, &mut lista, &mut partida), None);
        assert_eq!(
            lista.escolhido,
            JOGOS.len() - 1,
            "do primeiro, sobe ao último"
        );
        assert_eq!(reage(Acao::Desce, &mut lista, &mut partida), None);
        assert_eq!(lista.escolhido, 0, "do último, desce ao primeiro");
        assert_eq!(reage(Acao::Encerra, &mut lista, &mut partida), None);
        assert_eq!(
            reage(Acao::Sai, &mut lista, &mut partida),
            Some(ExitCode::SUCCESS)
        );
        assert_eq!(
            reage(Acao::Interrompe, &mut lista, &mut partida),
            Some(ExitCode::from(130))
        );
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn enter_sem_tela_ou_sem_a_pasta_nao_abre_e_deixa_o_recado() {
        let raiz = pasta_de_teste("lista-abre");
        let mut lista = Lista::vazia(&raiz);
        let mut partida = None;
        lista.tela = Err("sem tela X: o DISPLAY está vazio".to_string());
        reage(Acao::Abre, &mut lista, &mut partida);
        assert!(partida.is_none());
        assert_eq!(recado(&lista), ("BomberBoom não abre sem tela X", true));

        lista.tela = Ok(());
        reage(Acao::Abre, &mut lista, &mut partida);
        assert!(partida.is_none());
        let (texto, problema) = recado(&lista);
        assert!(
            texto.contains("bomberboom não existe") && problema,
            "{texto}"
        );
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn x_encerra_o_jogo_e_a_lista_volta_com_o_recado() {
        let raiz = pasta_de_teste("partida-x");
        let corpo = format!("echo 'BOMBERBOOM INICIO motor=love'\n{LANCADOR_COM_NETO}");
        jogo_falso(&raiz, "bomberboom", &corpo);
        let mut lista = Lista::vazia(&raiz);
        let mut partida = None;
        reage(Acao::Abre, &mut lista, &mut partida);
        assert!(
            partida.is_some(),
            "não abriu: {:?}",
            lista.recado.map(|r| r.texto)
        );
        let neto = espera_pid(&raiz.join("bomberboom/neto"), Duration::from_secs(3));
        let prazo = Instant::now() + Duration::from_secs(3);
        while partida.as_ref().unwrap().placar.total() == 0 {
            assert!(
                Instant::now() < prazo,
                "o fato do jogo de mentira não chegou"
            );
            acompanha(&mut lista, &mut partida);
            thread::sleep(Duration::from_millis(20));
        }
        reage(Acao::Desce, &mut lista, &mut partida);
        reage(Acao::Desce, &mut lista, &mut partida);
        assert_eq!(partida.as_ref().unwrap().rolagem, 0, "um tipo só: não rola");

        reage(Acao::Encerra, &mut lista, &mut partida);
        espera_o_fim(&mut lista, &mut partida);
        let (texto, problema) = recado(&lista);
        assert!(
            texto.starts_with("BomberBoom encerrado depois de 00:0"),
            "{texto}"
        );
        assert!(texto.ends_with("· 1 fatos") && !problema, "{texto}");
        assert!(
            morre_em(neto, Duration::from_secs(3)),
            "o neto {neto} sobrou"
        );
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn jogo_que_morre_deixa_o_codigo_e_o_erro_no_recado() {
        let raiz = pasta_de_teste("partida-morre");
        jogo_falso(
            &raiz,
            "bomberboom",
            "echo 'Error: sem runtime' >&2\nexit 2\n",
        );
        let mut lista = Lista::vazia(&raiz);
        let mut partida = None;
        reage(Acao::Abre, &mut lista, &mut partida);
        espera_o_fim(&mut lista, &mut partida);
        assert_eq!(
            recado(&lista),
            ("BomberBoom saiu com 2 · Error: sem runtime", true)
        );
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn ctrl_c_com_o_jogo_aberto_fecha_o_jogo_e_sai_com_130() {
        let raiz = pasta_de_teste("partida-ctrl-c");
        jogo_falso(&raiz, "bomberboom", LANCADOR_COM_NETO);
        let mut lista = Lista::vazia(&raiz);
        let mut partida = None;
        reage(Acao::Abre, &mut lista, &mut partida);
        assert!(
            partida.is_some(),
            "não abriu: {:?}",
            lista.recado.map(|r| r.texto)
        );
        let neto = espera_pid(&raiz.join("bomberboom/neto"), Duration::from_secs(3));
        let saida = reage(Acao::Interrompe, &mut lista, &mut partida);
        assert_eq!(saida, Some(ExitCode::from(130)));
        assert!(
            morre_em(neto, Duration::from_secs(1)),
            "o neto {neto} sobrou"
        );
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
