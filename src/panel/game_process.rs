//! O jogo rodando, do lado do processo: o lançador num grupo próprio, os dois canos lidos em threads,
//! o sinal ao grupo inteiro (o `x`, o Ctrl+C e o `Drop`) e o recado do fim. Fala com processo,
//! nunca com o terminal.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::list_screen::Recado;
use super::live_screen::VistaPartida;
use super::{formatting, signals};
use crate::facts::{Origem, Placar};
use crate::games::{self, Jogo};

/// Quanto o Ctrl+C espera o jogo fechar depois do SIGTERM, antes do SIGKILL.
const ESPERA_DO_FIM: Duration = Duration::from_secs(2);

/// Uma linha do jogo, de qual cano e quando chegou.
type Chegada = (Origem, String, Instant);

/// O jogo rodando, com o placar do que ele escreve.
pub(super) struct Partida {
    jogo: Jogo,
    filho: Child,
    chegadas: Receiver<Chegada>,
    inicio: Instant,
    pub(super) placar: Placar,
    pub(super) rolagem: usize,
    /// O autor pediu para fechar (`x`): o SIGTERM que vem daí não é erro do jogo.
    encerrado: bool,
}

impl Partida {
    pub(super) fn sobe(raiz: &Path, jogo: &Jogo) -> Result<Partida, String> {
        let mut filho = games::sobe(raiz, jogo)?;
        let (envia, chegadas) = mpsc::channel();
        let saida = filho
            .stdout
            .take()
            .expect("jogos::sobe põe o stdout num cano");
        let erro = filho
            .stderr
            .take()
            .expect("jogos::sobe põe o stderr num cano");
        escuta(saida, Origem::Saida, envia.clone());
        escuta(erro, Origem::Erro, envia);
        signals::anota_grupo_do_jogo(filho.id() as i32);
        Ok(Partida {
            jogo: jogo.clone(),
            filho,
            chegadas,
            inicio: Instant::now(),
            placar: Placar::novo(&jogo.pasta),
            rolagem: 0,
            encerrado: false,
        })
    }

    /// Passa ao placar as linhas que chegaram desde a última vez.
    pub(super) fn recolhe(&mut self) {
        for (origem, linha, instante) in self.chegadas.try_iter() {
            let quando = instante.saturating_duration_since(self.inicio);
            self.placar.anota(origem, &linha, quando);
        }
    }

    /// O recado do fim, se o jogo já fechou.
    pub(super) fn fim(&mut self) -> Option<Recado> {
        let estado = self.filho.try_wait().ok().flatten()?;
        // Colhido, o pid pode voltar a outro processo: o tratador de sinal não o mira mais.
        signals::anota_grupo_do_jogo(0);
        // As threads dos canos podem ainda ter as últimas linhas; o erro do boot costuma estar nelas.
        thread::sleep(Duration::from_millis(50));
        self.recolhe();
        let fim = Fim {
            codigo: games::codigo(estado),
            encerrado: self.encerrado,
            decorrido: self.inicio.elapsed(),
        };
        Some(recado_do_fim(&self.jogo.pasta, &fim, &self.placar))
    }

    pub(super) fn rola(&mut self, passo: isize) {
        let ultimo = self.placar.tipos().len().saturating_sub(1);
        self.rolagem = self.rolagem.saturating_add_signed(passo).min(ultimo);
    }

    /// Pede ao jogo inteiro (o grupo do lançador: ele e o LÖVE) que feche, com SIGTERM, que o LÖVE
    /// trata como fechar a janela.
    pub(super) fn encerra(&mut self) {
        self.encerrado = true;
        sinaliza_grupo(&self.filho, libc::SIGTERM);
    }

    /// Encerra e espera até `ESPERA_DO_FIM`; depois disso, SIGKILL. É para sair do jogar sem
    /// deixar jogo órfão.
    pub(super) fn encerra_e_espera(&mut self) {
        self.encerra();
        let prazo = Instant::now() + ESPERA_DO_FIM;
        while Instant::now() < prazo {
            if self.filho.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        sinaliza_grupo(&self.filho, libc::SIGKILL);
        let _ = self.filho.wait();
    }

    pub(super) fn vista(&self) -> VistaPartida<'_> {
        VistaPartida {
            nome: &self.jogo.pasta,
            placar: &self.placar,
            agora: self.inicio.elapsed(),
            rolagem: self.rolagem,
        }
    }
}

/// Como o jogo fechou.
struct Fim {
    /// O código do lançador, 128 + sinal se ele morreu por sinal.
    codigo: u8,
    /// O autor pediu para fechar (`x`).
    encerrado: bool,
    decorrido: Duration,
}

/// O recado da lista quando o jogo fecha: fechou (código 0), encerrado (pelo `x`), ou saiu com o
/// código e, se houver, a primeira linha do stderr, que num jogo que morre no boot diz por quê.
fn recado_do_fim(nome: &str, fim: &Fim, placar: &Placar) -> Recado {
    let duracao = formatting::relogio(fim.decorrido);
    let fatos = format!("{} fatos", placar.total());
    let codigo = fim.codigo;
    let (texto, problema) = match (codigo, fim.encerrado, placar.primeiro_erro()) {
        (0, _, _) => (
            format!("{nome} fechou depois de {duracao} · {fatos}"),
            false,
        ),
        (_, true, _) => (
            format!("{nome} encerrado depois de {duracao} · {fatos}"),
            false,
        ),
        (_, false, Some(erro)) => (format!("{nome} saiu com {codigo} · {erro}"), true),
        (_, false, None) => (
            format!("{nome} saiu com {codigo} depois de {duracao}"),
            true,
        ),
    };
    Recado { texto, problema }
}

/// A `Partida` que cai com o jogo ainda aberto (o jogar saindo por sinal, ou por pânico) fecha o
/// jogo inteiro, que senão ficaria órfão no grupo próprio. Jogo que já fechou e foi colhido não
/// leva sinal: o `try_wait` devolve o estado guardado, e o pid pode já ser de outro.
impl Drop for Partida {
    fn drop(&mut self) {
        if self.filho.try_wait().ok().flatten().is_none() {
            sinaliza_grupo(&self.filho, libc::SIGTERM);
        }
        signals::anota_grupo_do_jogo(0);
    }
}

/// Manda `sinal` ao grupo de processos do `filho`, que `jogos::sobe` cria com o pid dele.
fn sinaliza_grupo(filho: &Child, sinal: libc::c_int) {
    let grupo = filho.id() as libc::pid_t;
    // SAFETY: killpg só envia um sinal; não toca memória deste processo.
    let _ = unsafe { libc::killpg(grupo, sinal) };
}

/// Lê `cano` numa thread, linha a linha (bytes que não são UTF-8 viram �), e manda cada uma com a
/// hora em que chegou. Acaba quando o cano fecha ou o painel some.
fn escuta(cano: impl Read + Send + 'static, origem: Origem, envia: Sender<Chegada>) {
    thread::spawn(move || {
        let mut leitor = BufReader::new(cano);
        let mut bytes = Vec::new();
        while leitor.read_until(b'\n', &mut bytes).unwrap_or(0) > 0 {
            let linha = String::from_utf8_lossy(&bytes)
                .trim_end_matches(['\n', '\r'])
                .to_string();
            if envia.send((origem, linha, Instant::now())).is_err() {
                return;
            }
            bytes.clear();
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::games::Jogo;
    use crate::test_support::{
        LANCADOR_COM_NETO, espera_pid, jogo_falso, morre_em, pasta_de_teste,
    };

    /// Auditoria de 28/10, achado A1: o jogo sobe num grupo próprio, então, se o jogar sai sem
    /// fechá-lo (sinal, pânico), ninguém mais o fecha.
    #[test]
    fn partida_solta_com_o_jogo_aberto_fecha_o_jogo_inteiro() {
        let raiz = pasta_de_teste("partida-solta");
        jogo_falso(&raiz, "bomberboom", LANCADOR_COM_NETO);
        let jogo = Jogo {
            pasta: "bomberboom".to_string(),
        };
        let partida = Partida::sobe(&raiz, &jogo).unwrap();
        let neto = espera_pid(&raiz.join("bomberboom/neto"), Duration::from_secs(3));

        drop(partida);
        let morreu = morre_em(neto, Duration::from_secs(3));
        // SAFETY: só manda um sinal ao neto que o teste criou, caso ele tenha sobrado.
        let _ = unsafe { libc::kill(neto, libc::SIGKILL) };
        std::fs::remove_dir_all(&raiz).unwrap();
        assert!(morreu, "o neto {neto} sobreviveu à Partida solta");
    }

    #[test]
    fn recado_do_fim_diz_como_o_jogo_fechou() {
        let mut placar = Placar::novo("CampCrawler");
        placar.anota(Origem::Saida, "CAMPCRAWLER MAPA numero=1", Duration::ZERO);
        let fim = |codigo, encerrado| Fim {
            codigo,
            encerrado,
            decorrido: Duration::from_secs(134),
        };
        let recado = |fim: Fim, placar: &Placar| {
            let recado = recado_do_fim("CampCrawler", &fim, placar);
            (recado.texto, recado.problema)
        };
        let fechou = "CampCrawler fechou depois de 02:14 · 1 fatos";
        assert_eq!(recado(fim(0, false), &placar), (fechou.to_string(), false));
        let encerrado = "CampCrawler encerrado depois de 02:14 · 1 fatos";
        assert_eq!(
            recado(fim(143, true), &placar),
            (encerrado.to_string(), false)
        );
        let saiu = "CampCrawler saiu com 1 depois de 02:14";
        assert_eq!(recado(fim(1, false), &placar), (saiu.to_string(), true));
        placar.anota(Origem::Erro, "Error: x11 not available", Duration::ZERO);
        let erro = "CampCrawler saiu com 1 · Error: x11 not available";
        assert_eq!(recado(fim(1, false), &placar), (erro.to_string(), true));
    }
}
