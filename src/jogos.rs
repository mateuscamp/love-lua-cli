//! Os jogos de `~/lua` e o comando que abre cada um pelo lançador do próprio projeto.
//!
//! Este programa não sabe abrir o LÖVE. Cada jogo é dono do seu lançador (o que confere o runtime
//! contra o `runtime.lock` e limpa o ambiente); aqui só se chama esse lançador de dentro da pasta
//! do jogo, como o AGENTS.md de cada um manda. Se um projeto mudar o lançador, muda a linha dele
//! em `JOGOS`.

use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

pub struct Jogo {
    pub nome: &'static str,
    /// A pasta dentro de `~/lua`; é também o nome que se digita em `jogar <jogo>`.
    pub pasta: &'static str,
    /// O lançador do projeto e os argumentos dele até a pasta do jogo. Programa com barra é
    /// caminho dentro da pasta do jogo; sem barra, vem do PATH.
    pub lancador: &'static [&'static str],
}

pub const JOGOS: &[Jogo] = &[
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

pub fn procura(nome: &str) -> Option<&'static Jogo> {
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

/// A pasta do jogo, ou o erro de uma linha quando ela não existe.
fn pasta_do(raiz: &Path, jogo: &Jogo) -> Result<PathBuf, String> {
    let pasta = raiz.join(jogo.pasta);
    if pasta.is_dir() {
        Ok(pasta)
    } else {
        Err(format!(
            "{}: a pasta {} não existe",
            jogo.nome,
            pasta.display()
        ))
    }
}

fn nao_abriu(jogo: &Jogo, erro: std::io::Error) -> String {
    format!(
        "{}: o lançador {:?} não abriu: {erro}",
        jogo.nome, jogo.lancador
    )
}

/// O código de saída do lançador como o shell daria: 128 + sinal se ele morreu por sinal.
pub fn codigo(estado: ExitStatus) -> u8 {
    let codigo = estado
        .code()
        .or(estado.signal().map(|sinal| 128 + sinal))
        .unwrap_or(1);
    u8::try_from(codigo).unwrap_or(1)
}

/// Abre o jogo com o terminal dele (o modo direto, `jogar <jogo>`) e espera ele fechar. Devolve o
/// código de saída do lançador, ou o erro de antes de ele abrir.
pub fn abre(raiz: &Path, jogo: &Jogo, extras: &[String]) -> Result<u8, String> {
    pasta_do(raiz, jogo)?;
    let estado = comando(raiz, jogo, extras)
        .status()
        .map_err(|erro| nao_abriu(jogo, erro))?;
    Ok(codigo(estado))
}

/// Sobe o jogo para o painel e não espera. Sem terminal: stdin nulo, para não disputar as teclas
/// com o painel, e stdout e stderr num cano, que o painel lê. Num grupo de processos próprio, com
/// o pid do lançador, para o `x` do painel encerrar o lançador e o LÖVE juntos.
pub fn sobe(raiz: &Path, jogo: &Jogo) -> Result<Child, String> {
    pasta_do(raiz, jogo)?;
    comando(raiz, jogo, &[])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|erro| nao_abriu(jogo, erro))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_de_teste::{jogo_falso, pasta_de_teste};
    use std::io::{BufRead, BufReader};

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

    /// Um BomberBoom de mentira: o `tools/love` dele escreve um fato no stdout (com a pasta, o
    /// argumento e o grupo de processos em que rodou) e um erro no stderr.
    fn bomberboom_falso(raiz: &Path) {
        jogo_falso(
            raiz,
            "bomberboom",
            "echo \"BOMBERBOOM INICIO pasta=$(basename \"$PWD\") arg=$1 \
             grupo=$(cut -d' ' -f5 /proc/$$/stat)\"\n\
             echo 'Error: de mentira' >&2\n",
        );
    }

    #[test]
    fn sobe_roda_o_lancador_na_pasta_num_grupo_proprio_com_a_saida_num_cano() {
        let raiz = pasta_de_teste("sobe");
        bomberboom_falso(&raiz);
        let mut filho = sobe(&raiz, procura("bomberboom").unwrap()).unwrap();
        let saida = BufReader::new(filho.stdout.take().unwrap());
        let erro = BufReader::new(filho.stderr.take().unwrap());
        let linhas: Vec<String> = saida.lines().map(Result::unwrap).collect();
        let erros: Vec<String> = erro.lines().map(Result::unwrap).collect();
        // O grupo é o do próprio lançador, para o painel encerrar o lançador e o LÖVE juntos.
        let esperado = format!(
            "BOMBERBOOM INICIO pasta=bomberboom arg=. grupo={}",
            filho.id()
        );
        assert_eq!(linhas, [esperado]);
        assert_eq!(erros, ["Error: de mentira"]);
        assert_eq!(codigo(filho.wait().unwrap()), 0);
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn sobe_sem_a_pasta_do_jogo_diz_qual() {
        let raiz = pasta_de_teste("sobe-sem-pasta");
        let erro = sobe(&raiz, procura("campcrawler").unwrap()).unwrap_err();
        assert!(erro.contains("campcrawler não existe"), "{erro}");
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
