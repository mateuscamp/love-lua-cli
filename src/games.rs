//! The games in the root (`~/lua`) and the command that opens each one through the shared launcher.
//!
//! This program does not know how to open LÖVE. Every game carries the same launcher, `tools/love`,
//! which checks the runtime against the project's `runtime.lock` and cleans the environment; here
//! that launcher is only called from inside the game's folder. A game is any folder in the root
//! with an executable `tools/love` (author, 31/10): adding a game is adding the folder, without
//! touching this code. The shared launcher rule lives in the README.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

/// The shared launcher, relative to the game's folder, and its argument: the folder itself. Since
/// 28/10 the games in `~/lua` all open this way (Boomlitude was the last to switch,
/// Compositando/boomlitude#5).
pub const LAUNCHER: [&str; 2] = ["tools/love", "."];

#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    /// The folder inside the root: it is the name shown and what one types in `jogar <game>`. The
    /// prefix of the facts the game writes is this folder in uppercase.
    pub folder: String,
}

/// The games in `root`, in alphabetical order ignoring case: the folders with an executable
/// `tools/love`. A root that cannot be read gives the empty list, which the panel explains; a
/// folder whose name is not UTF-8 is left out.
pub fn discover(root: &Path) -> Vec<Game> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut games: Vec<Game> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|folder| has_launcher(&root.join(folder)))
        .map(|folder| Game { folder })
        .collect();
    games.sort_by_key(|game| game.folder.to_lowercase());
    games
}

/// Whether `folder` has `tools/love` as a file executable by someone (owner, group or others). A
/// symbolic link counts as the file it points to.
fn has_launcher(folder: &Path) -> bool {
    std::fs::metadata(folder.join(LAUNCHER[0]))
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The file in a game's folder that declares it cannot be played yet.
pub const UNPLAYABLE_MARKER: &str = "NAO_JOGAVEL";

/// Why `game` cannot be played yet: `Some` when its folder has the `NAO_JOGAVEL` file, with the
/// file's first non-empty line as the reason (a default sentence when it has none). The game
/// declares it, `jogar` only shows it (author, 01/11): the game still opens.
pub fn unplayable_reason(root: &Path, game: &Game) -> Option<String> {
    let text = std::fs::read_to_string(root.join(&game.folder).join(UNPLAYABLE_MARKER)).ok()?;
    let reason = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("sem motivo declarado");
    Some(reason.to_string())
}

/// The game in `root` whose folder is `name`, ignoring case.
pub fn find_game(root: &Path, name: &str) -> Option<Game> {
    discover(root)
        .into_iter()
        .find(|game| game.folder.eq_ignore_ascii_case(name))
}

/// The command that opens `game` from `root`, with `extra_args` after the folder (that is where the
/// shared launcher takes `--config=`). The program goes as an absolute path: `Command` does not
/// promise to resolve a relative one from `current_dir`.
fn launch_command(root: &Path, game: &Game, extra_args: &[String]) -> Command {
    let folder = root.join(&game.folder);
    let [program, argument] = LAUNCHER;
    let mut command = Command::new(folder.join(program));
    command.arg(argument).args(extra_args).current_dir(folder);
    command
}

/// The game's folder, or the one-line error when it vanished after discovery.
fn existing_folder(root: &Path, game: &Game) -> Result<PathBuf, String> {
    let folder = root.join(&game.folder);
    if folder.is_dir() {
        Ok(folder)
    } else {
        Err(format!(
            "{}: a pasta {} não existe",
            game.folder,
            folder.display()
        ))
    }
}

fn launch_failed(game: &Game, error: std::io::Error) -> String {
    format!(
        "{}: o lançador {} não abriu: {error}",
        game.folder,
        LAUNCHER.join(" ")
    )
}

/// The launcher's exit code as the shell would give it: 128 + signal if it died by a signal.
pub fn exit_code(status: ExitStatus) -> u8 {
    let code = status
        .code()
        .or(status.signal().map(|signal| 128 + signal))
        .unwrap_or(1);
    u8::try_from(code).unwrap_or(1)
}

/// Opens the game with its own terminal (the direct mode, `jogar <game>`) and waits for it to
/// close. Returns the launcher's exit code, or the error from before it opened.
pub fn run_attached(root: &Path, game: &Game, extra_args: &[String]) -> Result<u8, String> {
    existing_folder(root, game)?;
    let status = launch_command(root, game, extra_args)
        .status()
        .map_err(|error| launch_failed(game, error))?;
    Ok(exit_code(status))
}

/// Starts the game for the panel and does not wait. No terminal: null stdin, so as not to fight the
/// panel for the keys, and stdout and stderr into a pipe, which the panel reads. In its own process
/// group, with the launcher's pid, so the panel's `x` ends the launcher and LÖVE together.
pub fn spawn_piped(root: &Path, game: &Game) -> Result<Child, String> {
    existing_folder(root, game)?;
    launch_command(root, game, &[])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|error| launch_failed(game, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fake_game, test_folder};
    use std::io::{BufRead, BufReader};

    /// The folders of `games`, in order, to compare at once.
    fn folders(games: &[Game]) -> Vec<&str> {
        games.iter().map(|game| game.folder.as_str()).collect()
    }

    /// A game declares it is not playable yet with a `NAO_JOGAVEL` file (author, 01/11): the first
    /// non-empty line is the reason, an empty file gets a default one, and no file means playable.
    #[test]
    fn unplayable_reason_reads_the_marker_file_in_the_game_folder() {
        let root = test_folder("unplayable");
        for folder in ["gods", "vazio", "pronto"] {
            fake_game(&root, folder, "exit 0\n");
        }
        std::fs::write(
            root.join("gods/NAO_JOGAVEL"),
            "\n  só o domínio, sem telas \nF3\n",
        )
        .unwrap();
        std::fs::write(root.join("vazio/NAO_JOGAVEL"), "").unwrap();
        let game = |folder: &str| Game {
            folder: folder.to_string(),
        };
        assert_eq!(
            unplayable_reason(&root, &game("gods")),
            Some("só o domínio, sem telas".to_string())
        );
        assert_eq!(
            unplayable_reason(&root, &game("vazio")),
            Some("sem motivo declarado".to_string())
        );
        assert_eq!(unplayable_reason(&root, &game("pronto")), None);
    }

    /// A game is the folder with an executable `tools/love` (author, 31/10). Left out: the folder
    /// without it, the one with it but without the execute bit, the one whose `tools/love` is a
    /// folder, and the loose file in the root. The order ignores case: by the ASCII table, `Zumbi`
    /// would come first.
    #[test]
    fn discover_finds_the_folders_with_executable_tools_love_in_order() {
        let root = test_folder("discover");
        for folder in ["gods", "Zumbi", "campcrawler"] {
            fake_game(&root, folder, "exit 0\n");
        }
        std::fs::create_dir_all(root.join("anticolonia/assets")).unwrap();
        std::fs::create_dir_all(root.join("love-folder/tools/love")).unwrap();
        std::fs::create_dir_all(root.join("no-bit/tools")).unwrap();
        std::fs::write(root.join("no-bit/tools/love"), "#!/bin/sh\n").unwrap();
        std::fs::write(root.join("loose.txt"), "not a game").unwrap();
        let games = discover(&root);
        assert_eq!(folders(&games), ["campcrawler", "gods", "Zumbi"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_root_that_does_not_exist_has_no_games() {
        let home = test_folder("no-root");
        assert_eq!(discover(&home.join("lua")), []);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn find_game_matches_the_folder_ignoring_case() {
        let root = test_folder("find-game");
        fake_game(&root, "campcrawler", "exit 0\n");
        let found = find_game(&root, "CampCrawler").map(|game| game.folder);
        assert_eq!(found.as_deref(), Some("campcrawler"));
        assert_eq!(find_game(&root, "camcrawler"), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Every game opens through the shared launcher, as an absolute path, from inside its folder,
    /// with the extra arguments after the folder.
    #[test]
    fn the_shared_launcher_runs_in_the_game_folder_with_the_extras_after() {
        let extra_args = ["--config=bomberboom.seed=3".to_string()];
        let game = Game {
            folder: "bomberboom".to_string(),
        };
        let c = launch_command(Path::new("/r"), &game, &extra_args);
        assert_eq!(c.get_program(), "/r/bomberboom/tools/love");
        assert_eq!(
            c.get_args().collect::<Vec<_>>(),
            [".", "--config=bomberboom.seed=3"]
        );
        assert_eq!(c.get_current_dir(), Some(Path::new("/r/bomberboom")));
    }

    /// A fake BomberBoom: its `tools/love` writes a fact to stdout (with the folder, the argument
    /// and the process group it ran in) and an error to stderr.
    fn fake_bomberboom(root: &Path) {
        fake_game(
            root,
            "bomberboom",
            "echo \"BOMBERBOOM INICIO pasta=$(basename \"$PWD\") arg=$1 \
             grupo=$(cut -d' ' -f5 /proc/$$/stat)\"\n\
             echo 'Error: fake' >&2\n",
        );
    }

    #[test]
    fn spawn_piped_runs_the_launcher_in_the_folder_in_its_own_group_with_a_pipe() {
        let root = test_folder("spawn-piped");
        fake_bomberboom(&root);
        let game = Game {
            folder: "bomberboom".to_string(),
        };
        let mut child = spawn_piped(&root, &game).unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stderr = BufReader::new(child.stderr.take().unwrap());
        let lines: Vec<String> = stdout.lines().map(Result::unwrap).collect();
        let errors: Vec<String> = stderr.lines().map(Result::unwrap).collect();
        // The group is the launcher's own, so the panel ends the launcher and LÖVE together.
        let expected = format!(
            "BOMBERBOOM INICIO pasta=bomberboom arg=. grupo={}",
            child.id()
        );
        assert_eq!(lines, [expected]);
        assert_eq!(errors, ["Error: fake"]);
        assert_eq!(exit_code(child.wait().unwrap()), 0);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The folder that vanishes between discovery and Enter.
    #[test]
    fn spawn_piped_without_the_game_folder_says_which() {
        let root = test_folder("spawn-no-folder");
        let game = Game {
            folder: "campcrawler".to_string(),
        };
        let error = spawn_piped(&root, &game).unwrap_err();
        assert!(error.contains("campcrawler não existe"), "{error}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
