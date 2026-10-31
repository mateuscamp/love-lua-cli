//! The git of each game's folder: the branch, the uncommitted changes and the last commit.
//!
//! The launcher opens the game as it is in the folder; when someone is working there, that work in
//! progress is what opens, not the main branch, and the panel shows it (author, 28/10). The folders
//! in `~/lua` belong to other sessions, working there right now: from here they are only read,
//! without git's optional locks.

use std::path::Path;
use std::process::Command;

/// The git state of a game's folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Repo {
    /// The branch; `None` with a detached HEAD.
    pub branch: Option<String>,
    /// The main branch: what `origin/HEAD` points to, or `main`.
    pub main_branch: String,
    /// The HEAD commit, in 7 characters (empty in a repository with no commit).
    pub commit: String,
    /// Uncommitted changes, counting new files.
    pub changes: usize,
    pub last_commit: Option<Commit>,
}

/// The last commit of HEAD.
#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    /// Unix seconds of the commit (`%ct`).
    pub timestamp: i64,
    pub subject: String,
}

/// The output of `git` run in `folder`, or `None` if it did not run or failed. Without the optional
/// locks: `status` does not rewrite the index of a folder where someone may be working.
fn git_output(folder: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(folder)
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// The main branch of the repository in `folder`.
fn main_branch_of(folder: &Path) -> String {
    let origin = git_output(
        folder,
        &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"],
    );
    origin
        .as_deref()
        .and_then(|branch| branch.trim().strip_prefix("origin/"))
        .unwrap_or("main")
        .to_string()
}

/// Branch (`None` with a detached HEAD), commit and changes, from the output of
/// `git status --porcelain=v2 --branch`.
fn parse_status(status: &str) -> (Option<String>, String, usize) {
    let (mut branch, mut commit, mut changes) = (None, "", 0);
    for line in status.lines() {
        if let Some(value) = line.strip_prefix("# branch.head ") {
            branch = Some(value);
        } else if let Some(value) = line.strip_prefix("# branch.oid ") {
            commit = value;
        } else if !line.starts_with('#') {
            changes += 1;
        }
    }
    let branch = branch
        .filter(|branch| *branch != "(detached)")
        .map(str::to_string);
    let commit = commit.get(..7).unwrap_or(commit).to_string();
    (branch, commit, changes)
}

/// The last commit of HEAD; `None` in a repository with no commit.
fn read_last_commit(folder: &Path) -> Option<Commit> {
    let output = git_output(folder, &["log", "-1", "--format=%ct%x1f%s"])?;
    let (timestamp, subject) = output.trim_end().split_once('\x1f')?;
    Some(Commit {
        timestamp: timestamp.parse().ok()?,
        subject: subject.to_string(),
    })
}

/// The folder's git state, or `None` when it is not a repository or git does not answer.
pub fn read_repo(folder: &Path) -> Option<Repo> {
    let status = git_output(folder, &["status", "--porcelain=v2", "--branch"])?;
    let (branch, commit, changes) = parse_status(&status);
    Some(Repo {
        branch,
        main_branch: main_branch_of(folder),
        commit,
        changes,
        last_commit: read_last_commit(folder),
    })
}

impl Repo {
    /// The branch off the main one, or the detached HEAD, as the panel shows it.
    pub fn off_main(&self) -> Option<String> {
        match &self.branch {
            None => Some(format!("HEAD solto em {}", self.commit)),
            Some(branch) if *branch != self.main_branch => Some(format!("branch {branch}")),
            Some(_) => None,
        }
    }

    /// The uncommitted changes in words, or `None` with none.
    pub fn changes_in_words(&self) -> Option<String> {
        match self.changes {
            0 => None,
            1 => Some("1 mudança sem commit".to_string()),
            n => Some(format!("{n} mudanças sem commit")),
        }
    }

    /// What the folder has off the main branch: the branch, if it is not the main one, and the
    /// uncommitted changes. `None` with a clean folder on the main branch.
    pub fn work_in_progress(&self) -> Option<String> {
        let parts: Vec<String> = [self.off_main(), self.changes_in_words()]
            .into_iter()
            .flatten()
            .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{test_folder, test_git};

    fn wip(folder: &Path) -> Option<String> {
        read_repo(folder)?.work_in_progress()
    }

    #[test]
    fn work_in_progress_marks_a_branch_off_main_and_uncommitted_changes() {
        let root = test_folder("wip");
        let game = root.join("game");
        std::fs::create_dir(&game).unwrap();
        assert_eq!(wip(&game), None, "folder without git");

        test_git(&game, &["init", "-q", "-b", "main"]);
        std::fs::write(game.join("main.lua"), "-- game\n").unwrap();
        test_git(&game, &["add", "."]);
        test_git(&game, &["commit", "-q", "-m", "one"]);
        assert_eq!(wip(&game), None, "clean on main");

        std::fs::write(game.join("main.lua"), "-- game changed\n").unwrap();
        assert_eq!(wip(&game).as_deref(), Some("1 mudança sem commit"));
        std::fs::write(game.join("new.lua"), "-- new\n").unwrap();
        assert_eq!(wip(&game).as_deref(), Some("2 mudanças sem commit"));

        test_git(&game, &["checkout", "-q", "-b", "r0-pendencias"]);
        assert_eq!(
            wip(&game).as_deref(),
            Some("branch r0-pendencias, 2 mudanças sem commit")
        );
        test_git(&game, &["add", "."]);
        test_git(&game, &["commit", "-q", "-m", "two"]);
        assert_eq!(wip(&game).as_deref(), Some("branch r0-pendencias"));

        test_git(&game, &["checkout", "-q", "--detach"]);
        let detached = wip(&game).unwrap();
        assert!(detached.starts_with("HEAD solto em "), "{detached}");
        assert_eq!(detached.len(), "HEAD solto em ".len() + 7, "{detached}");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn work_in_progress_reads_the_main_branch_from_origin_head() {
        let root = test_folder("main-branch");
        let origin = root.join("origin");
        std::fs::create_dir(&origin).unwrap();
        test_git(&origin, &["init", "-q", "-b", "trunk"]);
        std::fs::write(origin.join("main.lua"), "-- game\n").unwrap();
        test_git(&origin, &["add", "."]);
        test_git(&origin, &["commit", "-q", "-m", "one"]);
        test_git(&root, &["clone", "-q", "origin", "game"]);

        let game = root.join("game");
        assert_eq!(
            wip(&game),
            None,
            "on trunk, which is the origin's main branch"
        );
        test_git(&game, &["checkout", "-q", "-b", "main"]);
        assert_eq!(wip(&game).as_deref(), Some("branch main"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn read_repo_reads_the_last_commit() {
        let game = test_folder("last-commit");
        test_git(&game, &["init", "-q", "-b", "main"]);
        assert_eq!(read_repo(&game).unwrap().last_commit, None, "no commit");

        std::fs::write(game.join("main.lua"), "-- game\n").unwrap();
        test_git(&game, &["add", "."]);
        test_git(
            &game,
            &["commit", "-q", "-m", "O mostrado acompanha as passagens"],
        );
        let last = read_repo(&game).unwrap().last_commit.unwrap();
        assert_eq!(last.subject, "O mostrado acompanha as passagens");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(
            (now - last.timestamp).abs() < 60,
            "{} against {now}",
            last.timestamp
        );

        std::fs::remove_dir_all(&game).unwrap();
    }
}
