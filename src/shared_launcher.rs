//! The shared launcher is the same in every game (author, 31/10; the rule lives in the README). The
//! panel reads each game's copy and marks the one that differs from the others: it only warns, it
//! does not block, like the work-in-progress mark.
//!
//! On 31/10 `tools/love` was the same in all four games, but BomberBoom's
//! `tools/lancador/sistema.lua` already had a fix (`mktemp` instead of `os.tmpname`) that the
//! others did not.

use std::path::Path;

/// The shared launcher's files in a game folder, by relative path, in order: `tools/love` and the
/// pieces in `tools/lancador/` (one level only, as in the games of 31/10). `tools/love_projeto.lua`
/// belongs to each project and is left out. A file that cannot be read is also left out, and the
/// copy without it differs from the ones that have it.
pub type LauncherCopy = Vec<(String, Vec<u8>)>;

/// Reads the copy of the shared launcher that lives in `folder`.
pub fn read_copy(folder: &Path) -> LauncherCopy {
    let mut copy: LauncherCopy = Vec::new();
    read_into_copy(folder, "tools/love", &mut copy);
    let Ok(pieces) = std::fs::read_dir(folder.join("tools/lancador")) else {
        return copy;
    };
    let mut names: Vec<String> = pieces
        .filter_map(|piece| piece.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names {
        read_into_copy(folder, &format!("tools/lancador/{name}"), &mut copy);
    }
    copy
}

/// Puts the content of `folder/relative` into `copy`, if it can be read (a subfolder cannot).
fn read_into_copy(folder: &Path, relative: &str, copy: &mut LauncherCopy) {
    if let Ok(content) = std::fs::read(folder.join(relative)) {
        copy.push((relative.to_string(), content));
    }
}

/// Which copies differ: the ones unlike the majority's copy. With no majority (a tie at the top),
/// every copy differs, because each one differs from some other; with all equal, or a single one,
/// none.
pub fn which_differ(copies: &[LauncherCopy]) -> Vec<bool> {
    let equals: Vec<usize> = copies
        .iter()
        .map(|copy| copies.iter().filter(|other| *other == copy).count())
        .collect();
    let largest = equals.iter().copied().max().unwrap_or(0);
    // A single group at the top: the `largest` copies with the maximum count are all the same.
    let has_majority = equals.iter().filter(|&&count| count == largest).count() == largest;
    equals
        .iter()
        .map(|&count| !has_majority || count != largest)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fake_game, test_folder};

    /// A copy with only `tools/love`, holding `content`.
    fn copy_of(content: &str) -> LauncherCopy {
        vec![("tools/love".to_string(), content.as_bytes().to_vec())]
    }

    #[test]
    fn the_copy_outside_the_majority_differs() {
        let (a, b) = (copy_of("a"), copy_of("b"));
        assert_eq!(which_differ(&[]), Vec::<bool>::new());
        assert_eq!(
            which_differ(std::slice::from_ref(&a)),
            [false],
            "a single one does not differ"
        );
        assert_eq!(which_differ(&[a.clone(), a.clone(), a.clone()]), [false; 3]);
        assert_eq!(
            which_differ(&[a.clone(), b.clone(), a.clone()]),
            [false, true, false]
        );
        assert_eq!(
            which_differ(&[a.clone(), b.clone(), b.clone(), a.clone()]),
            [true; 4],
            "two against two: no majority, all of them"
        );
        assert_eq!(which_differ(&[a, b]), [true; 2]);
    }

    /// The copy is `tools/love` and the pieces in `tools/lancador/`; `tools/love_projeto.lua`,
    /// which belongs to each project, does not count.
    #[test]
    fn read_copy_takes_the_launcher_and_its_pieces_without_the_project_file() {
        let root = test_folder("read-copy");
        for (folder, project, system) in [
            ("one", "name = 'one'", "local M = {}"),
            ("two", "name = 'two'", "local M = {}"),
            ("three", "name = 'three'", "local M = { tmp = 'mktemp' }"),
        ] {
            fake_game(&root, folder, "exec love .\n");
            let tools = root.join(folder).join("tools");
            std::fs::write(tools.join("love_projeto.lua"), project).unwrap();
            std::fs::create_dir_all(tools.join("lancador")).unwrap();
            std::fs::write(tools.join("lancador/sistema.lua"), system).unwrap();
            std::fs::write(tools.join("lancador/troca.lua"), "-- troca").unwrap();
        }
        let copies: Vec<LauncherCopy> = ["one", "two", "three"]
            .iter()
            .map(|folder| read_copy(&root.join(folder)))
            .collect();
        let paths: Vec<&str> = copies[0].iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            paths,
            [
                "tools/love",
                "tools/lancador/sistema.lua",
                "tools/lancador/troca.lua"
            ]
        );
        assert_eq!(copies[0], copies[1], "love_projeto.lua does not count");
        assert_ne!(copies[0], copies[2], "the different piece counts");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
