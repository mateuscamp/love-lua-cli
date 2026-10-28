//! `jogar`: escolhe um dos jogos LÖVE de `~/lua` e abre pelo lançador do próprio projeto.
//!
//! Este programa não sabe abrir o LÖVE. Cada jogo é dono do seu lançador (o que confere o runtime
//! contra o `runtime.lock` e limpa o ambiente); aqui só se chama esse lançador de dentro da pasta
//! do jogo, como o AGENTS.md de cada um manda. Se um projeto mudar o lançador, muda a linha dele
//! em `JOGOS`.

use std::env;
use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{SocketAddr, UnixStream};
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

/// O número do display local em `valor` (`:0`, `:1.0`, `unix:0`). `Ok(None)` é display remoto
/// (`localhost:10.0`, o do `ssh -X`), que daqui não se confere; `Err` é valor que não é display.
fn display_local(valor: &str) -> Result<Option<u32>, ()> {
    let (maquina, resto) = valor.rsplit_once(':').ok_or(())?;
    let numero = resto
        .split_once('.')
        .map_or(resto, |(numero, _tela)| numero);
    let numero: u32 = numero.parse().map_err(|_| ())?;
    match maquina {
        "" | "unix" => Ok(Some(numero)),
        _ => Ok(None),
    }
}

/// Confere se há um X atendendo no `display`, procurando o socket dele em `sockets` (no sistema,
/// `/tmp/.X11-unix`). Os três lançadores forçam `SDL_VIDEO_DRIVER=x11`, e sem X o jogo morre no
/// boot com um stack trace ("x11 not available"); isto avisa antes, numa linha.
fn tela_x(display: Option<&str>, sockets: &Path) -> Result<(), String> {
    let valor = display.unwrap_or("");
    if valor.is_empty() {
        return Err("sem tela X: o DISPLAY está vazio".to_string());
    }
    let numero = match display_local(valor) {
        Ok(Some(numero)) => numero,
        Ok(None) => return Ok(()),
        Err(()) => return Err(format!("sem tela X: DISPLAY={valor:?} não é um display")),
    };
    // O X local escuta num socket de arquivo e, no Linux, também num abstrato de mesmo nome.
    let caminho = sockets.join(format!("X{numero}"));
    let atende = UnixStream::connect(&caminho).is_ok()
        || SocketAddr::from_abstract_name(caminho.as_os_str().as_bytes())
            .and_then(|endereco| UnixStream::connect_addr(&endereco))
            .is_ok();
    if atende {
        Ok(())
    } else {
        Err(format!("sem tela X: ninguém atende no DISPLAY={valor:?}"))
    }
}

/// A `tela_x` deste processo.
fn tela() -> Result<(), String> {
    tela_x(
        env::var("DISPLAY").ok().as_deref(),
        Path::new("/tmp/.X11-unix"),
    )
}

/// A saída do `git` rodado em `pasta`, ou `None` se ele não rodou ou falhou. Sem os locks
/// opcionais: o `status` não reescreve o índice de uma pasta onde alguém pode estar trabalhando.
fn git(pasta: &Path, argumentos: &[&str]) -> Option<String> {
    let saida = Command::new("git")
        .arg("-C")
        .arg(pasta)
        .arg("--no-optional-locks")
        .args(argumentos)
        .output()
        .ok()?;
    if !saida.status.success() {
        return None;
    }
    String::from_utf8(saida.stdout).ok()
}

/// O que a pasta do jogo tem fora do ramo principal (o do `origin/HEAD`, ou `main`): a branch, se
/// não for a principal, e as mudanças sem commit. O lançador abre o jogo como ele está na pasta;
/// quando alguém trabalha nela, é essa obra que abre, não o principal. `None` com a pasta limpa no
/// principal, ou quando o git não responde.
fn obra(pasta: &Path) -> Option<String> {
    let estado = git(pasta, &["status", "--porcelain=v2", "--branch"])?;
    let origem = git(
        pasta,
        &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"],
    );
    let principal = origem
        .as_deref()
        .and_then(|ramo| ramo.trim().strip_prefix("origin/"))
        .unwrap_or("main");
    let (mut ramo, mut commit, mut mudancas) = (None, "", 0);
    for linha in estado.lines() {
        if let Some(valor) = linha.strip_prefix("# branch.head ") {
            ramo = Some(valor);
        } else if let Some(valor) = linha.strip_prefix("# branch.oid ") {
            commit = valor;
        } else if !linha.starts_with('#') {
            mudancas += 1;
        }
    }
    let mut partes = Vec::new();
    match ramo {
        Some("(detached)") => partes.push(format!(
            "HEAD solto em {}",
            commit.get(..7).unwrap_or(commit)
        )),
        Some(ramo) if ramo != principal => partes.push(format!("branch {ramo}")),
        _ => {}
    }
    match mudancas {
        0 => {}
        1 => partes.push("1 mudança sem commit".to_string()),
        n => partes.push(format!("{n} mudanças sem commit")),
    }
    (!partes.is_empty()).then(|| partes.join(", "))
}

/// Os itens do menu: o nome de cada jogo e, se a pasta dele está em obra, o que há nela.
fn itens(raiz: &Path) -> Vec<String> {
    let largura = JOGOS.iter().map(|jogo| jogo.nome.len()).max().unwrap_or(0);
    JOGOS
        .iter()
        .map(|jogo| match obra(&raiz.join(jogo.pasta)) {
            Some(obra) => format!("{:largura$}  em obra: {obra}", jogo.nome),
            None => jogo.nome.to_string(),
        })
        .collect()
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
/// jogo. Sem tela X, a pergunta vira o aviso e o Enter não abre nada. O jogo com a pasta em obra
/// sai marcado, mas abre igual.
fn menu(raiz: &Path) -> ExitCode {
    // O Select esconde o cursor enquanto desenha, e o Ctrl+C mata o processo antes de ele
    // devolver: sem isto o terminal fica sem cursor.
    let _ = ctrlc::set_handler(|| {
        let _ = Term::stderr().show_cursor();
        process::exit(130);
    });
    let mut escolhido = 0;
    loop {
        let tela = tela();
        let itens = itens(raiz);
        let pergunta = match &tela {
            Ok(()) => "Qual jogo? (Enter abre, Esc sai)".to_string(),
            Err(aviso) => format!("{aviso}; nenhum jogo abre (Esc sai)"),
        };
        let escolha = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(pergunta)
            .items(&itens)
            .default(escolhido)
            .interact_opt();
        match escolha {
            Ok(Some(i)) => {
                escolhido = i;
                if tela.is_err() {
                    continue;
                }
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
    if let Err(aviso) = tela() {
        eprintln!("jogar: {} não abre: {aviso}", jogo.nome);
        return ExitCode::from(2);
    }
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
    use std::os::unix::net::UnixListener;

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

    #[test]
    fn display_local_le_o_numero_e_deixa_o_remoto_de_fora() {
        assert_eq!(display_local(":0"), Ok(Some(0)));
        assert_eq!(display_local(":1.0"), Ok(Some(1)));
        assert_eq!(display_local("unix:2"), Ok(Some(2)));
        assert_eq!(display_local("localhost:10.0"), Ok(None));
        for errado in ["", ":", "0", ":abc", ":0x"] {
            assert_eq!(display_local(errado), Err(()), "{errado:?}");
        }
    }

    #[test]
    fn tela_x_so_passa_com_alguem_atendendo_no_socket() {
        let sockets = env::temp_dir().join(format!("jogar-teste-{}", process::id()));
        std::fs::create_dir_all(&sockets).unwrap();
        let _x5 = UnixListener::bind(sockets.join("X5")).unwrap();

        assert_eq!(tela_x(Some(":5"), &sockets), Ok(()));
        assert_eq!(tela_x(Some("localhost:10.0"), &sockets), Ok(()));
        let sem_ninguem = tela_x(Some(":6"), &sockets).unwrap_err();
        assert!(sem_ninguem.contains(r#"DISPLAY=":6""#), "{sem_ninguem}");
        assert!(tela_x(None, &sockets).unwrap_err().contains("vazio"));
        assert!(tela_x(Some(""), &sockets).unwrap_err().contains("vazio"));
        assert!(
            tela_x(Some(":abc"), &sockets)
                .unwrap_err()
                .contains("não é um display")
        );

        std::fs::remove_dir_all(&sockets).unwrap();
    }

    /// Roda o git em `pasta` sem a configuração do usuário (assinatura, hooks), para o teste
    /// não depender dela.
    fn git_teste(pasta: &Path, argumentos: &[&str]) {
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

    /// Uma pasta vazia e só deste teste.
    fn pasta_de_teste(nome: &str) -> PathBuf {
        let pasta = env::temp_dir().join(format!("jogar-{nome}-{}", process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        pasta
    }

    #[test]
    fn obra_marca_branch_fora_do_principal_e_mudancas_sem_commit() {
        let raiz = pasta_de_teste("obra");
        let jogo = raiz.join("jogo");
        std::fs::create_dir(&jogo).unwrap();
        assert_eq!(obra(&jogo), None, "pasta sem git");

        git_teste(&jogo, &["init", "-q", "-b", "main"]);
        std::fs::write(jogo.join("main.lua"), "-- jogo\n").unwrap();
        git_teste(&jogo, &["add", "."]);
        git_teste(&jogo, &["commit", "-q", "-m", "um"]);
        assert_eq!(obra(&jogo), None, "limpa no main");

        std::fs::write(jogo.join("main.lua"), "-- jogo mudado\n").unwrap();
        assert_eq!(obra(&jogo).as_deref(), Some("1 mudança sem commit"));
        std::fs::write(jogo.join("novo.lua"), "-- novo\n").unwrap();
        assert_eq!(obra(&jogo).as_deref(), Some("2 mudanças sem commit"));

        git_teste(&jogo, &["checkout", "-q", "-b", "r0-pendencias"]);
        assert_eq!(
            obra(&jogo).as_deref(),
            Some("branch r0-pendencias, 2 mudanças sem commit")
        );
        git_teste(&jogo, &["add", "."]);
        git_teste(&jogo, &["commit", "-q", "-m", "dois"]);
        assert_eq!(obra(&jogo).as_deref(), Some("branch r0-pendencias"));

        git_teste(&jogo, &["checkout", "-q", "--detach"]);
        let solto = obra(&jogo).unwrap();
        assert!(solto.starts_with("HEAD solto em "), "{solto}");
        assert_eq!(solto.len(), "HEAD solto em ".len() + 7, "{solto}");

        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn obra_le_o_principal_do_origin_head() {
        let raiz = pasta_de_teste("principal");
        let origem = raiz.join("origem");
        std::fs::create_dir(&origem).unwrap();
        git_teste(&origem, &["init", "-q", "-b", "trunk"]);
        std::fs::write(origem.join("main.lua"), "-- jogo\n").unwrap();
        git_teste(&origem, &["add", "."]);
        git_teste(&origem, &["commit", "-q", "-m", "um"]);
        git_teste(&raiz, &["clone", "-q", "origem", "jogo"]);

        let jogo = raiz.join("jogo");
        assert_eq!(obra(&jogo), None, "no trunk, que é o principal da origem");
        git_teste(&jogo, &["checkout", "-q", "-b", "main"]);
        assert_eq!(obra(&jogo).as_deref(), Some("branch main"));

        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
