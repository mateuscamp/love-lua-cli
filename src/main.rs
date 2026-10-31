//! `jogar`: picks one of the LÖVE games in the root (`~/lua`) and opens it through the shared
//! launcher, `tools/love`.
//!
//! With no argument, the full-screen panel (`panel`); with a game's name, it opens it directly,
//! with the game's own terminal. The modules: `games` (discovery and the command that opens a
//! game), `shared_launcher` (each game's copy of the launcher), `x_display` (the X server before
//! opening), `git_state` (the git of each game's folder), `facts` (what the game writes while it
//! runs) and `panel` (the only one that talks to the terminal).

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

fn help_text(root: &Path) -> String {
    let found: Vec<String> = games::discover(root)
        .into_iter()
        .map(|game| game.folder)
        .collect();
    let list = if found.is_empty() {
        "nenhum (um jogo é uma pasta com tools/love executável)".to_string()
    } else {
        found.join(", ")
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
         jogos: {list}\n",
        root.display()
    )
}

/// `jogar <game> [args...]`: checks the X display, opens the game with its own terminal and exits
/// with its code.
fn open_directly(root: &Path, name: &str, extra_args: &[String]) -> ExitCode {
    let Some(game) = games::find_game(root, name) else {
        eprint!("jogar: não conheço o jogo {name:?}\n\n{}", help_text(root));
        return ExitCode::from(2);
    };
    if let Err(warning) = x_display::check_this_display() {
        eprintln!("jogar: {} não abre: {warning}", game.folder);
        return ExitCode::from(2);
    }
    match games::run_attached(root, &game, extra_args) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("jogar: {error}");
            ExitCode::from(2)
        }
    }
}

/// The games' root: `chosen` (the `$JOGAR_RAIZ`) when not empty, else `home/lua`. Without either,
/// the one-line error.
fn games_root(home: Option<OsString>, chosen: Option<OsString>) -> Result<PathBuf, String> {
    if let Some(chosen) = chosen.filter(|chosen| !chosen.is_empty()) {
        return Ok(PathBuf::from(chosen));
    }
    home.map(|home| PathBuf::from(home).join("lua"))
        .ok_or_else(|| "$HOME não está definido, e nem $JOGAR_RAIZ".to_string())
}

fn main() -> ExitCode {
    let root = match games_root(env::var_os("HOME"), env::var_os("JOGAR_RAIZ")) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("jogar: {error}");
            return ExitCode::from(2);
        }
    };
    let args: Vec<String> = env::args().skip(1).collect();
    match args.split_first() {
        None => panel::menu(&root),
        Some((name, _)) if name == "-h" || name == "--help" => {
            print!("{}", help_text(&root));
            ExitCode::SUCCESS
        }
        Some((name, extra_args)) => open_directly(&root, name, extra_args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `$JOGAR_RAIZ` puts the root somewhere else (for whoever does not keep the games in
    /// `~/lua`); when empty, `~/lua` holds.
    #[test]
    fn the_root_comes_from_jogar_raiz_or_from_lua_in_home() {
        let home = Some(OsString::from("/c"));
        let chosen = Some(OsString::from("/games"));
        assert_eq!(games_root(home.clone(), None), Ok(PathBuf::from("/c/lua")));
        assert_eq!(
            games_root(home.clone(), chosen.clone()),
            Ok(PathBuf::from("/games"))
        );
        assert_eq!(games_root(None, chosen), Ok(PathBuf::from("/games")));
        assert_eq!(
            games_root(home, Some(OsString::new())),
            Ok(PathBuf::from("/c/lua"))
        );
        assert!(games_root(None, None).is_err());
    }

    /// Audit of 28/10, finding A3: AGENTS.md said Rust 1.85+, and ratatui 0.30 needs 1.88. The
    /// minimum version lives in Cargo.toml's `rust-version`, which cargo enforces, and AGENTS.md
    /// states the same one.
    #[test]
    fn agents_md_states_the_rust_version_from_cargo_toml() {
        let version = include_str!("../Cargo.toml")
            .lines()
            .find_map(|line| line.strip_prefix("rust-version = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("Cargo.toml does not state the rust-version");
        let expected = format!("Rust {version}+");
        assert!(
            include_str!("../AGENTS.md").contains(&expected),
            "AGENTS.md does not state {expected}"
        );
    }
}
