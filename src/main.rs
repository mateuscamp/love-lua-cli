//! `jogar`: escolhe um dos jogos LÖVE de `~/lua` e abre pelo lançador do próprio projeto.
//!
//! Sem argumento, o painel em tela cheia (`painel`); com o nome de um jogo, abre direto, com o
//! terminal do jogo. Os módulos: `jogos` (a tabela e o comando que abre), `tela` (o X antes de
//! abrir), `obra` (o git da pasta de cada jogo), `fatos` (o que o jogo escreve enquanto roda) e
//! `painel` (o único que fala com o terminal).

#[cfg(test)]
mod apoio_de_teste;
mod fatos;
mod jogos;
mod obra;
mod painel;
mod tela;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use jogos::JOGOS;

fn ajuda() -> String {
    let pastas: Vec<&str> = JOGOS.iter().map(|jogo| jogo.pasta).collect();
    format!(
        "jogar: abre um dos jogos LÖVE de ~/lua pelo lançador do próprio projeto.\n\
         \n\
         \x20 jogar                    painel: setas escolhem, Enter abre, Esc ou q sai;\n\
         \x20                          com o jogo aberto, o que ele faz ao vivo, e x encerra\n\
         \x20 jogar <jogo> [args...]   abre direto; os args vão ao lançador depois da pasta,\n\
         \x20                          ex.: jogar campcrawler --config=campcrawler.lang=pt_BR\n\
         \n\
         jogos: {}\n",
        pastas.join(", ")
    )
}

/// `jogar <jogo> [args...]`: confere a tela, abre com o terminal do jogo e sai com o código dele.
fn direto(raiz: &std::path::Path, nome: &str, extras: &[String]) -> ExitCode {
    let Some(jogo) = jogos::procura(nome) else {
        eprint!("jogar: não conheço o jogo {nome:?}\n\n{}", ajuda());
        return ExitCode::from(2);
    };
    if let Err(aviso) = tela::tela() {
        eprintln!("jogar: {} não abre: {aviso}", jogo.nome);
        return ExitCode::from(2);
    }
    match jogos::abre(raiz, jogo, extras) {
        Ok(codigo) => ExitCode::from(codigo),
        Err(erro) => {
            eprintln!("jogar: {erro}");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let Some(casa) = env::var_os("HOME") else {
        eprintln!("jogar: $HOME não está definido");
        return ExitCode::from(2);
    };
    let raiz = PathBuf::from(casa).join("lua");
    let argumentos: Vec<String> = env::args().skip(1).collect();
    match argumentos.split_first() {
        None => painel::menu(&raiz),
        Some((nome, _)) if nome == "-h" || nome == "--help" => {
            print!("{}", ajuda());
            ExitCode::SUCCESS
        }
        Some((nome, extras)) => direto(&raiz, nome, extras),
    }
}

#[cfg(test)]
mod testes {
    /// Auditoria de 28/10, achado A3: o AGENTS.md dizia Rust 1.85+, e o ratatui 0.30 pede 1.88. A
    /// versão mínima mora no `rust-version` do Cargo.toml, que o cargo cobra, e o AGENTS.md diz a
    /// mesma.
    #[test]
    fn agents_md_diz_a_versao_do_rust_do_cargo_toml() {
        let versao = include_str!("../Cargo.toml")
            .lines()
            .find_map(|linha| linha.strip_prefix("rust-version = \""))
            .and_then(|resto| resto.strip_suffix('"'))
            .expect("o Cargo.toml não diz o rust-version");
        let esperado = format!("Rust {versao}+");
        assert!(
            include_str!("../AGENTS.md").contains(&esperado),
            "o AGENTS.md não diz {esperado}"
        );
    }
}
