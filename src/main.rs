//! `jogar`: escolhe um dos jogos LÖVE da raiz (`~/lua`) e abre pelo lançador comum, `tools/love`.
//!
//! Sem argumento, o painel em tela cheia (`painel`); com o nome de um jogo, abre direto, com o
//! terminal do jogo. Os módulos: `jogos` (a descoberta e o comando que abre), `lancador_comum` (a
//! cópia do lançador em cada jogo), `tela` (o X antes de abrir), `obra` (o git da pasta de cada
//! jogo), `fatos` (o que o jogo escreve enquanto roda) e `painel` (o único que fala com o terminal).

mod facts;
mod games;
mod git_state;
mod panel;
mod shared_launcher;
#[cfg(test)]
mod test_support;
mod x_display;

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn ajuda(raiz: &Path) -> String {
    let encontrados: Vec<String> = games::descobre(raiz)
        .into_iter()
        .map(|jogo| jogo.pasta)
        .collect();
    let lista = if encontrados.is_empty() {
        "nenhum (um jogo é uma pasta com tools/love executável)".to_string()
    } else {
        encontrados.join(", ")
    };
    format!(
        "jogar: abre um dos jogos LÖVE de {} pelo lançador comum, tools/love.\n\
         \n\
         \x20 jogar                    painel: setas escolhem, Enter abre, Esc ou q sai;\n\
         \x20                          com o jogo aberto, o que ele faz ao vivo, e x encerra\n\
         \x20 jogar <jogo> [args...]   abre direto; os args vão ao lançador depois da pasta,\n\
         \x20                          ex.: jogar campcrawler --config=campcrawler.lang=pt_BR\n\
         \n\
         A raiz é $JOGAR_RAIZ, ou ~/lua.\n\
         jogos: {lista}\n",
        raiz.display()
    )
}

/// `jogar <jogo> [args...]`: confere a tela, abre com o terminal do jogo e sai com o código dele.
fn direto(raiz: &Path, nome: &str, extras: &[String]) -> ExitCode {
    let Some(jogo) = games::procura(raiz, nome) else {
        eprint!("jogar: não conheço o jogo {nome:?}\n\n{}", ajuda(raiz));
        return ExitCode::from(2);
    };
    if let Err(aviso) = x_display::tela() {
        eprintln!("jogar: {} não abre: {aviso}", jogo.pasta);
        return ExitCode::from(2);
    }
    match games::abre(raiz, &jogo, extras) {
        Ok(codigo) => ExitCode::from(codigo),
        Err(erro) => {
            eprintln!("jogar: {erro}");
            ExitCode::from(2)
        }
    }
}

/// A raiz dos jogos: `escolhida` (o `$JOGAR_RAIZ`) quando não vazia, senão `casa/lua`. Sem as
/// duas, o erro de uma linha.
fn raiz_dos_jogos(casa: Option<OsString>, escolhida: Option<OsString>) -> Result<PathBuf, String> {
    if let Some(escolhida) = escolhida.filter(|escolhida| !escolhida.is_empty()) {
        return Ok(PathBuf::from(escolhida));
    }
    casa.map(|casa| PathBuf::from(casa).join("lua"))
        .ok_or_else(|| "$HOME não está definido, e nem $JOGAR_RAIZ".to_string())
}

fn main() -> ExitCode {
    let raiz = match raiz_dos_jogos(env::var_os("HOME"), env::var_os("JOGAR_RAIZ")) {
        Ok(raiz) => raiz,
        Err(erro) => {
            eprintln!("jogar: {erro}");
            return ExitCode::from(2);
        }
    };
    let argumentos: Vec<String> = env::args().skip(1).collect();
    match argumentos.split_first() {
        None => panel::menu(&raiz),
        Some((nome, _)) if nome == "-h" || nome == "--help" => {
            print!("{}", ajuda(&raiz));
            ExitCode::SUCCESS
        }
        Some((nome, extras)) => direto(&raiz, nome, extras),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O `$JOGAR_RAIZ` põe a raiz em outro lugar (para quem não guarda os jogos em `~/lua`); vazio,
    /// vale o `~/lua`.
    #[test]
    fn a_raiz_vem_do_jogar_raiz_ou_do_lua_da_casa() {
        let casa = Some(OsString::from("/c"));
        let escolhida = Some(OsString::from("/jogos"));
        assert_eq!(
            raiz_dos_jogos(casa.clone(), None),
            Ok(PathBuf::from("/c/lua"))
        );
        assert_eq!(
            raiz_dos_jogos(casa.clone(), escolhida.clone()),
            Ok(PathBuf::from("/jogos"))
        );
        assert_eq!(raiz_dos_jogos(None, escolhida), Ok(PathBuf::from("/jogos")));
        assert_eq!(
            raiz_dos_jogos(casa, Some(OsString::new())),
            Ok(PathBuf::from("/c/lua"))
        );
        assert!(raiz_dos_jogos(None, None).is_err());
    }

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
