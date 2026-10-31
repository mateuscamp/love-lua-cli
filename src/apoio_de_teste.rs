//! Ajudantes dos testes: só o descartável (pasta temporária, repositório git temporário, jogo de
//! mentira), nunca `~/lua`, `~/.cargo` nem a tela do autor. O teste de integração em `tests/`
//! inclui este mesmo arquivo pelo `#[path]`, porque o crate é só binário.

#![allow(dead_code)] // cada crate de teste usa uma parte

use std::env;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

/// Roda o git em `pasta` sem a configuração do usuário (assinatura, hooks), para o teste
/// não depender dela.
pub fn git_teste(pasta: &Path, argumentos: &[&str]) {
    let certo = Command::new("git")
        .arg("-C")
        .arg(pasta)
        .args(["-c", "user.name=teste", "-c", "user.email=teste@jogar"])
        .args(argumentos)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap()
        .success();
    assert!(certo, "git {argumentos:?}");
}

/// Uma pasta vazia e só deste teste (o `nome` distingue os testes que rodam em paralelo).
pub fn pasta_de_teste(nome: &str) -> PathBuf {
    let pasta = env::temp_dir().join(format!("jogar-{nome}-{}", process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    pasta
}

/// Um jogo de mentira em `raiz/pasta`, com o `tools/love` (o lançador dos quatro jogos) feito do
/// `corpo` em sh. Ele roda na pasta do jogo, como o de verdade.
///
/// Quem escreve o script é outro processo. Se fosse este, um fork de outro teste em paralelo
/// herdaria o descritor de escrita, e o exec do lançador falharia com "Text file busy" (ETXTBSY:
/// 5 em 40 rodadas da suíte com 32 threads, 28/10).
pub fn jogo_falso(raiz: &Path, pasta: &str, corpo: &str) {
    let ferramentas = raiz.join(pasta).join("tools");
    std::fs::create_dir_all(&ferramentas).unwrap();
    let lancador = ferramentas.join("love");
    let certo = Command::new("sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
            "sh",
        ])
        .arg(&lancador)
        .arg(format!("#!/bin/sh\n{corpo}"))
        .status()
        .unwrap()
        .success();
    assert!(certo, "não escreveu {}", lancador.display());
}

/// O pid que o jogo de mentira escreveu em `arquivo`, esperando até `prazo` ele aparecer.
pub fn espera_pid(arquivo: &Path, prazo: Duration) -> i32 {
    let fim = Instant::now() + prazo;
    loop {
        let lido = std::fs::read_to_string(arquivo).unwrap_or_default();
        if let Ok(pid) = lido.trim().parse() {
            return pid;
        }
        assert!(Instant::now() < fim, "{} não apareceu", arquivo.display());
        thread::sleep(Duration::from_millis(20));
    }
}

/// Se o processo `pid` roda (zumbi conta como morto: já saiu, só falta quem o colha).
pub fn pid_vivo(pid: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(estado) => !estado
            .rsplit_once(") ")
            .is_some_and(|(_, resto)| resto.starts_with('Z')),
        Err(_) => false,
    }
}

/// Se `pid` morre até `prazo`.
pub fn morre_em(pid: i32, prazo: Duration) -> bool {
    let fim = Instant::now() + prazo;
    while Instant::now() < fim {
        if !pid_vivo(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

/// O lançador de mentira deixa um neto (como um lançador que abrisse o LÖVE como filho, e não no
/// lugar dele), anota o pid dele e espera.
pub const LANCADOR_COM_NETO: &str = "sleep 30 &\necho $! > neto\nwait\n";
