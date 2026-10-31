//! Os jogos da raiz (`~/lua`) e o comando que abre cada um pelo lançador comum.
//!
//! Este programa não sabe abrir o LÖVE. Todo jogo traz o mesmo lançador, `tools/love`, que confere
//! o runtime contra o `runtime.lock` do projeto e limpa o ambiente; aqui só se chama esse lançador
//! de dentro da pasta do jogo. Jogo é toda pasta da raiz com um `tools/love` executável (autor,
//! 31/10): pôr um jogo é pôr a pasta, sem mexer aqui. A regra do lançador comum mora no README.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

/// O lançador comum, relativo à pasta do jogo, e o argumento dele: a própria pasta. Desde 28/10 os
/// jogos de `~/lua` abrem todos assim (o Boomlitude foi o último a trocar, Compositando/boomlitude#5).
pub const LANCADOR: [&str; 2] = ["tools/love", "."];

#[derive(Clone, Debug, PartialEq)]
pub struct Jogo {
    /// A pasta dentro da raiz: é o nome mostrado e o que se digita em `jogar <jogo>`. O prefixo
    /// dos fatos que o jogo escreve é ela em maiúsculas.
    pub pasta: String,
}

/// Os jogos de `raiz`, em ordem alfabética sem olhar a caixa: as pastas com `tools/love`
/// executável. Raiz que não se lê dá a lista vazia, que o painel explica; pasta cujo nome não é
/// UTF-8 fica de fora.
pub fn descobre(raiz: &Path) -> Vec<Jogo> {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return Vec::new();
    };
    let mut jogos: Vec<Jogo> = entradas
        .filter_map(|entrada| entrada.ok()?.file_name().into_string().ok())
        .filter(|pasta| tem_lancador(&raiz.join(pasta)))
        .map(|pasta| Jogo { pasta })
        .collect();
    jogos.sort_by_key(|jogo| jogo.pasta.to_lowercase());
    jogos
}

/// Se `pasta` tem o `tools/love` como arquivo executável por alguém (dono, grupo ou outros). O
/// link simbólico vale pelo arquivo a que aponta.
fn tem_lancador(pasta: &Path) -> bool {
    std::fs::metadata(pasta.join(LANCADOR[0]))
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// O jogo de `raiz` cuja pasta é `nome`, sem olhar a caixa.
pub fn procura(raiz: &Path, nome: &str) -> Option<Jogo> {
    descobre(raiz)
        .into_iter()
        .find(|jogo| jogo.pasta.eq_ignore_ascii_case(nome))
}

/// O comando que abre `jogo` a partir de `raiz`, com `extras` depois da pasta (é onde o lançador
/// comum aceita o `--config=`). O programa vai absoluto: o `Command` não promete resolver o
/// relativo a partir do `current_dir`.
fn comando(raiz: &Path, jogo: &Jogo, extras: &[String]) -> Command {
    let pasta = raiz.join(&jogo.pasta);
    let [programa, argumento] = LANCADOR;
    let mut comando = Command::new(pasta.join(programa));
    comando.arg(argumento).args(extras).current_dir(pasta);
    comando
}

/// A pasta do jogo, ou o erro de uma linha quando ela sumiu depois da descoberta.
fn pasta_do(raiz: &Path, jogo: &Jogo) -> Result<PathBuf, String> {
    let pasta = raiz.join(&jogo.pasta);
    if pasta.is_dir() {
        Ok(pasta)
    } else {
        Err(format!(
            "{}: a pasta {} não existe",
            jogo.pasta,
            pasta.display()
        ))
    }
}

fn nao_abriu(jogo: &Jogo, erro: std::io::Error) -> String {
    format!(
        "{}: o lançador {} não abriu: {erro}",
        jogo.pasta,
        LANCADOR.join(" ")
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

    /// As pastas dos `jogos`, na ordem, para comparar de uma vez.
    fn pastas(jogos: &[Jogo]) -> Vec<&str> {
        jogos.iter().map(|jogo| jogo.pasta.as_str()).collect()
    }

    /// Jogo é a pasta com `tools/love` executável (autor, 31/10). Fica de fora a pasta sem ele, a
    /// com ele sem o bit de execução, a com `tools/love` que é pasta, e o arquivo solto na raiz. A
    /// ordem não olha a caixa: pela tabela ASCII, o `Zumbi` viria antes.
    #[test]
    fn descobre_as_pastas_com_tools_love_executavel_em_ordem() {
        let raiz = pasta_de_teste("descobre");
        for pasta in ["gods", "Zumbi", "campcrawler"] {
            jogo_falso(&raiz, pasta, "exit 0\n");
        }
        std::fs::create_dir_all(raiz.join("anticolonia/assets")).unwrap();
        std::fs::create_dir_all(raiz.join("pasta-love/tools/love")).unwrap();
        std::fs::create_dir_all(raiz.join("sem-bit/tools")).unwrap();
        std::fs::write(raiz.join("sem-bit/tools/love"), "#!/bin/sh\n").unwrap();
        std::fs::write(raiz.join("solto.txt"), "não é jogo").unwrap();
        let jogos = descobre(&raiz);
        assert_eq!(pastas(&jogos), ["campcrawler", "gods", "Zumbi"]);
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn raiz_que_nao_existe_nao_tem_jogos() {
        let casa = pasta_de_teste("sem-raiz");
        assert_eq!(descobre(&casa.join("lua")), []);
        std::fs::remove_dir_all(&casa).unwrap();
    }

    #[test]
    fn procura_pela_pasta_sem_olhar_a_caixa() {
        let raiz = pasta_de_teste("procura");
        jogo_falso(&raiz, "campcrawler", "exit 0\n");
        let achado = procura(&raiz, "CampCrawler").map(|jogo| jogo.pasta);
        assert_eq!(achado.as_deref(), Some("campcrawler"));
        assert_eq!(procura(&raiz, "camcrawler"), None);
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    /// Todo jogo abre pelo lançador comum, absoluto, de dentro da pasta, com os extras depois dela.
    #[test]
    fn o_lancador_comum_roda_na_pasta_do_jogo_com_os_extras_depois() {
        let extras = ["--config=bomberboom.seed=3".to_string()];
        let jogo = Jogo {
            pasta: "bomberboom".to_string(),
        };
        let c = comando(Path::new("/r"), &jogo, &extras);
        assert_eq!(c.get_program(), "/r/bomberboom/tools/love");
        assert_eq!(
            c.get_args().collect::<Vec<_>>(),
            [".", "--config=bomberboom.seed=3"]
        );
        assert_eq!(c.get_current_dir(), Some(Path::new("/r/bomberboom")));
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
        let jogo = Jogo {
            pasta: "bomberboom".to_string(),
        };
        let mut filho = sobe(&raiz, &jogo).unwrap();
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

    /// A pasta que some entre a descoberta e o Enter.
    #[test]
    fn sobe_sem_a_pasta_do_jogo_diz_qual() {
        let raiz = pasta_de_teste("sobe-sem-pasta");
        let jogo = Jogo {
            pasta: "campcrawler".to_string(),
        };
        let erro = sobe(&raiz, &jogo).unwrap_err();
        assert!(erro.contains("campcrawler não existe"), "{erro}");
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
