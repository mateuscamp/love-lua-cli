//! O jogar inteiro, o binário, num pseudo-terminal (`script`, do util-linux), com um `~/lua` de
//! mentira: o Enter abre o jogo falso pelo painel, e o jogar que sai por sinal ou porque o terminal
//! fechou tem de levar o jogo junto. Auditoria de 28/10, achado A1: o jogo sobe num grupo de
//! processos próprio e, sem isto, ficava órfão com a janela aberta.

#[path = "../src/test_support.rs"]
mod test_support;

use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::Duration;

use test_support::{espera_pid, jogo_falso, morre_em, pasta_de_teste};

/// O painel aberto num terminal, com o jogo falso aberto pelo Enter.
struct Painel {
    terminal: Child,
    /// Segura o stdin do `script`: sem ele, o terminal fecharia antes da hora.
    _teclas: ChildStdin,
    jogar: i32,
    neto: i32,
}

/// Abre o painel com `HOME=casa` e aperta Enter. O lançador anota quem o abriu (o jogar) e um
/// neto, como um lançador que abrisse o LÖVE como filho.
fn abre_pelo_painel(casa: &Path) -> Painel {
    jogo_falso(
        &casa.join("lua"),
        "bomberboom",
        "echo $PPID > jogar\nsleep 30 &\necho $! > neto\nwait\n",
    );
    let mut terminal = Command::new("script")
        .args(["-qfec", env!("CARGO_BIN_EXE_jogar"), "/dev/null"])
        .env("HOME", casa)
        // O `$JOGAR_RAIZ` de quem roda a suíte levaria o painel para fora da casa de mentira.
        .env_remove("JOGAR_RAIZ")
        // Display remoto: o jogar não confere, e o jogo de mentira não precisa de X.
        .env("DISPLAY", "localhost:10.0")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("o teste precisa do script, do util-linux");
    let mut teclas = terminal.stdin.take().unwrap();
    thread::sleep(Duration::from_millis(800));
    teclas.write_all(b"\r").unwrap();
    let pasta = casa.join("lua/bomberboom");
    Painel {
        jogar: espera_pid(&pasta.join("jogar"), Duration::from_secs(5)),
        neto: espera_pid(&pasta.join("neto"), Duration::from_secs(5)),
        terminal,
        _teclas: teclas,
    }
}

/// Mata o que sobrou e apaga a casa; devolve se o neto e o jogar tinham morrido antes disso.
fn fecha(mut painel: Painel, casa: &Path) -> (bool, bool) {
    let neto_morreu = morre_em(painel.neto, Duration::from_secs(3));
    let jogar_saiu = morre_em(painel.jogar, Duration::from_secs(3));
    // SAFETY: os sinais vão só aos processos que este teste criou.
    unsafe {
        libc::kill(painel.neto, libc::SIGKILL);
        libc::kill(painel.jogar, libc::SIGKILL);
    }
    let _ = painel.terminal.kill();
    let _ = painel.terminal.wait();
    std::fs::remove_dir_all(casa).unwrap();
    (neto_morreu, jogar_saiu)
}

#[test]
fn sighup_no_jogar_fecha_o_jogo_aberto_pelo_painel() {
    let casa = pasta_de_teste("sighup");
    let painel = abre_pelo_painel(&casa);
    // SAFETY: o sinal vai só ao jogar que este teste abriu.
    unsafe { libc::kill(painel.jogar, libc::SIGHUP) };
    let (neto, _) = fecha(painel, &casa);
    assert!(neto, "o jogo sobreviveu ao SIGHUP no jogar");
}

/// O terminal que fecha de verdade (o dono do pseudo-terminal morre). Diferente do SIGHUP direto:
/// o tty fica morto, o `read` dele devolve 0, e o crossterm gira em falso lá dentro sem devolver o
/// laço; o tratador do sinal é quem tem de fechar o jogo e sair (re-auditoria de 28/10).
#[test]
fn terminal_que_fecha_leva_o_jogo_e_o_jogar() {
    let casa = pasta_de_teste("terminal-fecha");
    let mut painel = abre_pelo_painel(&casa);
    painel.terminal.kill().unwrap();
    let (neto, jogar) = fecha(painel, &casa);
    assert!(neto, "o jogo sobreviveu ao terminal fechado");
    assert!(jogar, "o jogar ficou rodando com o terminal fechado");
}

#[test]
fn sigterm_no_jogar_fecha_o_jogo_e_o_jogar_sai() {
    let casa = pasta_de_teste("sigterm");
    let painel = abre_pelo_painel(&casa);
    // SAFETY: o sinal vai só ao jogar que este teste abriu.
    unsafe { libc::kill(painel.jogar, libc::SIGTERM) };
    let (neto, jogar) = fecha(painel, &casa);
    assert!(neto, "o jogo sobreviveu ao SIGTERM no jogar");
    assert!(jogar, "o jogar não saiu com o SIGTERM");
}
