//! O git da pasta de cada jogo: a branch, as mudanças sem commit e o último commit.
//!
//! O lançador abre o jogo como ele está na pasta; quando alguém trabalha nela, é essa obra que
//! abre, não o principal, e o painel mostra isso (autor, 28/10). As pastas de `~/lua` são de
//! outras sessões, trabalhando ali agora: daqui só se lê, sem os locks opcionais do git.

use std::path::Path;
use std::process::Command;

/// O estado git da pasta de um jogo.
#[derive(Debug, Clone, PartialEq)]
pub struct Repo {
    /// A branch; `None` com o HEAD solto.
    pub ramo: Option<String>,
    /// O ramo principal: o que o `origin/HEAD` aponta, ou `main`.
    pub principal: String,
    /// O commit do HEAD, em 7 caracteres (vazio num repositório sem commit).
    pub commit: String,
    /// Mudanças sem commit, contando os arquivos novos.
    pub mudancas: usize,
    pub ultimo: Option<Commit>,
}

/// O último commit do HEAD.
#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    /// Segundos Unix do commit (`%ct`).
    pub quando: i64,
    pub assunto: String,
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

/// O ramo principal do repositório em `pasta`.
fn principal(pasta: &Path) -> String {
    let origem = git(
        pasta,
        &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"],
    );
    origem
        .as_deref()
        .and_then(|ramo| ramo.trim().strip_prefix("origin/"))
        .unwrap_or("main")
        .to_string()
}

/// Branch (`None` com o HEAD solto), commit e mudanças, da saída de
/// `git status --porcelain=v2 --branch`.
fn le_status(estado: &str) -> (Option<String>, String, usize) {
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
    let ramo = ramo
        .filter(|ramo| *ramo != "(detached)")
        .map(str::to_string);
    let commit = commit.get(..7).unwrap_or(commit).to_string();
    (ramo, commit, mudancas)
}

/// O último commit do HEAD; `None` num repositório sem commit.
fn ultimo(pasta: &Path) -> Option<Commit> {
    let saida = git(pasta, &["log", "-1", "--format=%ct%x1f%s"])?;
    let (quando, assunto) = saida.trim_end().split_once('\x1f')?;
    Some(Commit {
        quando: quando.parse().ok()?,
        assunto: assunto.to_string(),
    })
}

/// O estado git da pasta, ou `None` quando ela não é repositório ou o git não responde.
pub fn repo(pasta: &Path) -> Option<Repo> {
    let estado = git(pasta, &["status", "--porcelain=v2", "--branch"])?;
    let (ramo, commit, mudancas) = le_status(&estado);
    Some(Repo {
        ramo,
        principal: principal(pasta),
        commit,
        mudancas,
        ultimo: ultimo(pasta),
    })
}

impl Repo {
    /// A branch fora do principal, ou o HEAD solto, como o painel mostra.
    pub fn desvio(&self) -> Option<String> {
        match &self.ramo {
            None => Some(format!("HEAD solto em {}", self.commit)),
            Some(ramo) if *ramo != self.principal => Some(format!("branch {ramo}")),
            Some(_) => None,
        }
    }

    /// As mudanças sem commit por extenso, ou `None` sem nenhuma.
    pub fn mudancas_por_extenso(&self) -> Option<String> {
        match self.mudancas {
            0 => None,
            1 => Some("1 mudança sem commit".to_string()),
            n => Some(format!("{n} mudanças sem commit")),
        }
    }

    /// O que a pasta tem fora do ramo principal: a branch, se não for a principal, e as mudanças
    /// sem commit. `None` com a pasta limpa no principal.
    pub fn obra(&self) -> Option<String> {
        let partes: Vec<String> = [self.desvio(), self.mudancas_por_extenso()]
            .into_iter()
            .flatten()
            .collect();
        (!partes.is_empty()).then(|| partes.join(", "))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::test_support::{git_teste, pasta_de_teste};

    fn obra(pasta: &Path) -> Option<String> {
        repo(pasta)?.obra()
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

    #[test]
    fn repo_le_o_ultimo_commit() {
        let jogo = pasta_de_teste("ultimo");
        git_teste(&jogo, &["init", "-q", "-b", "main"]);
        assert_eq!(repo(&jogo).unwrap().ultimo, None, "sem commit");

        std::fs::write(jogo.join("main.lua"), "-- jogo\n").unwrap();
        git_teste(&jogo, &["add", "."]);
        git_teste(
            &jogo,
            &["commit", "-q", "-m", "O mostrado acompanha as passagens"],
        );
        let ultimo = repo(&jogo).unwrap().ultimo.unwrap();
        assert_eq!(ultimo.assunto, "O mostrado acompanha as passagens");
        let agora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(
            (agora - ultimo.quando).abs() < 60,
            "{} contra {agora}",
            ultimo.quando
        );

        std::fs::remove_dir_all(&jogo).unwrap();
    }
}
