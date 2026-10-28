//! O SIGHUP (o terminal que fecha) e o SIGTERM no painel. O tratador fecha o jogo aberto ele mesmo,
//! sem depender do laço: com o terminal fechado, o tty devolve 0 no `read`, o laço interno de
//! leitura do crossterm só sai com `WouldBlock` e gira em falso, e o laço do painel nunca mais
//! volta (re-auditoria de 28/10). No SIGHUP o jogar sai ali mesmo, porque não há terminal a
//! restaurar; no SIGTERM, com o terminal vivo, o laço vê a anotação e sai pelo caminho normal.

use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};

/// O sinal (SIGTERM) que pediu ao painel para sair; 0 enquanto nenhum veio.
static SINAL_DE_SAIDA: AtomicI32 = AtomicI32::new(0);

/// O grupo de processos do jogo aberto; 0 sem jogo. É o pid do lançador (`jogos::sobe`).
static GRUPO_DO_JOGO: AtomicI32 = AtomicI32::new(0);

/// O grupo do jogo que acabou de subir, ou 0 quando ele fecha e é colhido.
pub(super) fn anota_grupo_do_jogo(grupo: i32) {
    GRUPO_DO_JOGO.store(grupo, Ordering::SeqCst);
}

/// Só chamadas permitidas dentro de um tratador de sinal: atômicos, `killpg` e `_exit`.
extern "C" fn trata_sinal(sinal: libc::c_int) {
    let grupo = GRUPO_DO_JOGO.load(Ordering::SeqCst);
    // SAFETY: killpg e _exit são async-signal-safe; o grupo é o do jogo que o painel subiu.
    unsafe {
        if grupo > 0 {
            libc::killpg(grupo, libc::SIGTERM);
        }
        if sinal == libc::SIGHUP {
            libc::_exit(128 + sinal);
        }
    }
    SINAL_DE_SAIDA.store(sinal, Ordering::SeqCst);
}

/// Instala o tratador no SIGHUP e no SIGTERM. Sem ele o jogar morria na hora e o jogo, num grupo
/// de processos próprio, ficava órfão com a janela aberta (auditoria de 28/10, A1).
pub(super) fn anota_sinais_de_saida() {
    for sinal in [libc::SIGHUP, libc::SIGTERM] {
        // SAFETY: o tratador só usa chamadas async-signal-safe; a estrutura é zerada e preenchida
        // antes da chamada.
        unsafe {
            let mut acao: libc::sigaction = std::mem::zeroed();
            acao.sa_sigaction = trata_sinal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            acao.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut acao.sa_mask);
            libc::sigaction(sinal, &acao, std::ptr::null_mut());
        }
    }
}

/// A saída do jogar se um sinal pediu: 128 + sinal, como o shell daria.
pub(super) fn saida_por_sinal() -> Option<ExitCode> {
    let sinal = SINAL_DE_SAIDA.load(Ordering::SeqCst);
    (sinal != 0).then(|| ExitCode::from(128 + sinal as u8))
}
