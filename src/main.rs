//! `jogar`: escolhe um dos jogos LÖVE de `~/lua` e abre pelo lançador do próprio projeto.
//!
//! Este programa não sabe abrir o LÖVE. Cada jogo é dono do seu lançador (o que confere o runtime
//! contra o `runtime.lock` e limpa o ambiente); aqui só se chama esse lançador de dentro da pasta
//! do jogo, como o AGENTS.md de cada um manda. Se um projeto mudar o lançador, muda a linha dele
//! em `JOGOS`.

use std::env;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitCode};

use dialoguer::Select;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;

struct Jogo {
    nome: &'static str,
    /// A pasta dentro de `~/lua`; é também o nome que se digita em `jogar <jogo>`.
    pasta: &'static str,
    /// O lançador do projeto e os argumentos dele até a pasta do jogo. Programa com barra é
    /// caminho dentro da pasta do jogo; sem barra, vem do PATH.
    lancador: &'static [&'static str],
}

const JOGOS: &[Jogo] = &[
    Jogo {
        nome: "BomberBoom",
        pasta: "bomberboom",
        lancador: &["tools/love", "."],
    },
    Jogo {
        nome: "Boomlitude",
        pasta: "boomlitude",
        lancador: &["luajit", "ferramentas/love/lanca.lua", "."],
    },
    Jogo {
        nome: "CampCrawler",
        pasta: "campcrawler",
        lancador: &["tools/love", "."],
    },
];

fn procura(nome: &str) -> Option<&'static Jogo> {
    JOGOS
        .iter()
        .find(|jogo| jogo.pasta.eq_ignore_ascii_case(nome))
}

/// O comando que abre `jogo` a partir de `raiz`, com `extras` depois da pasta (é onde os três
/// lançadores aceitam o `--config=`). O programa relativo vira absoluto: o `Command` não promete
/// resolvê-lo a partir do `current_dir`.
fn comando(raiz: &Path, jogo: &Jogo, extras: &[String]) -> Command {
    let pasta = raiz.join(jogo.pasta);
    let (programa, argumentos) = jogo.lancador.split_first().expect("lançador sem programa");
    let programa = if programa.contains('/') {
        pasta.join(programa)
    } else {
        PathBuf::from(programa)
    };
    let mut comando = Command::new(programa);
    comando.args(argumentos).args(extras).current_dir(pasta);
    comando
}

/// Abre o jogo e espera ele fechar. Devolve o código de saída do lançador (128 + sinal, se ele
/// morreu por sinal), ou o erro de antes de ele abrir.
fn abre(raiz: &Path, jogo: &Jogo, extras: &[String]) -> Result<u8, String> {
    let pasta = raiz.join(jogo.pasta);
    if !pasta.is_dir() {
        return Err(format!(
            "{}: a pasta {} não existe",
            jogo.nome,
            pasta.display()
        ));
    }
    let estado = comando(raiz, jogo, extras).status().map_err(|erro| {
        format!(
            "{}: o lançador {:?} não abriu: {erro}",
            jogo.nome, jogo.lancador
        )
    })?;
    let codigo = estado
        .code()
        .or(estado.signal().map(|sinal| 128 + sinal))
        .unwrap_or(1);
    Ok(u8::try_from(codigo).unwrap_or(1))
}

/// O menu: setas escolhem, Enter abre, Esc ou q sai. Quando o jogo fecha, o menu volta no mesmo
/// jogo.
fn menu(raiz: &Path) -> ExitCode {
    // O Select esconde o cursor enquanto desenha, e o Ctrl+C mata o processo antes de ele
    // devolver: sem isto o terminal fica sem cursor.
    let _ = ctrlc::set_handler(|| {
        let _ = Term::stderr().show_cursor();
        process::exit(130);
    });
    let nomes: Vec<&str> = JOGOS.iter().map(|jogo| jogo.nome).collect();
    let mut escolhido = 0;
    loop {
        let escolha = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Qual jogo? (Enter abre, Esc sai)")
            .items(&nomes)
            .default(escolhido)
            .interact_opt();
        match escolha {
            Ok(Some(i)) => {
                escolhido = i;
                match abre(raiz, &JOGOS[i], &[]) {
                    Ok(0) => {}
                    Ok(codigo) => eprintln!("jogar: {} saiu com {codigo}", JOGOS[i].nome),
                    Err(erro) => eprintln!("jogar: {erro}"),
                }
            }
            Ok(None) => return ExitCode::SUCCESS,
            // O Ctrl+C, que com o handler interrompe a leitura da tecla em vez de matar.
            Err(dialoguer::Error::IO(erro)) if erro.kind() == io::ErrorKind::Interrupted => {
                let _ = Term::stderr().show_cursor();
                return ExitCode::from(130);
            }
            Err(erro) => {
                eprintln!("jogar: o menu precisa de um terminal ({erro})");
                return ExitCode::from(2);
            }
        }
    }
}

fn ajuda() -> String {
    let pastas: Vec<&str> = JOGOS.iter().map(|jogo| jogo.pasta).collect();
    format!(
        "jogar: abre um dos jogos LÖVE de ~/lua pelo lançador do próprio projeto.\n\
         \n\
         \x20 jogar                    menu: setas escolhem, Enter abre, Esc ou q sai\n\
         \x20 jogar <jogo> [args...]   abre direto; os args vão ao lançador depois da pasta,\n\
         \x20                          ex.: jogar campcrawler --config=campcrawler.lang=pt_BR\n\
         \n\
         jogos: {}\n",
        pastas.join(", ")
    )
}

fn main() -> ExitCode {
    let Some(casa) = env::var_os("HOME") else {
        eprintln!("jogar: $HOME não está definido");
        return ExitCode::from(2);
    };
    let raiz = PathBuf::from(casa).join("lua");
    let mut argumentos = env::args().skip(1);
    let Some(nome) = argumentos.next() else {
        return menu(&raiz);
    };
    if nome == "-h" || nome == "--help" {
        print!("{}", ajuda());
        return ExitCode::SUCCESS;
    }
    let Some(jogo) = procura(&nome) else {
        eprint!("jogar: não conheço o jogo {nome:?}\n\n{}", ajuda());
        return ExitCode::from(2);
    };
    let extras: Vec<String> = argumentos.collect();
    match abre(&raiz, jogo, &extras) {
        Ok(codigo) => ExitCode::from(codigo),
        Err(erro) => {
            eprintln!("jogar: {erro}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn procura_pela_pasta_sem_olhar_a_caixa() {
        assert_eq!(procura("CampCrawler").map(|j| j.nome), Some("CampCrawler"));
        assert_eq!(procura("boomlitude").map(|j| j.nome), Some("Boomlitude"));
        assert!(procura("camcrawler").is_none());
    }

    #[test]
    fn lancador_com_barra_vira_caminho_absoluto_na_pasta_do_jogo() {
        let extras = ["--config=bomberboom.seed=3".to_string()];
        let c = comando(Path::new("/r"), procura("bomberboom").unwrap(), &extras);
        assert_eq!(c.get_program(), "/r/bomberboom/tools/love");
        assert_eq!(
            c.get_args().collect::<Vec<_>>(),
            [".", "--config=bomberboom.seed=3"]
        );
        assert_eq!(c.get_current_dir(), Some(Path::new("/r/bomberboom")));
    }

    #[test]
    fn lancador_sem_barra_vem_do_path_e_roda_na_pasta_do_jogo() {
        let c = comando(Path::new("/r"), procura("boomlitude").unwrap(), &[]);
        assert_eq!(c.get_program(), "luajit");
        assert_eq!(
            c.get_args().collect::<Vec<_>>(),
            ["ferramentas/love/lanca.lua", "."]
        );
        assert_eq!(c.get_current_dir(), Some(Path::new("/r/boomlitude")));
    }
}
