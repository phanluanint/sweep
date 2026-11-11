//! Guards every path before it is moved to the Trash.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::fsx;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GuardError {
    #[error("path must be absolute and plain")]
    NotPlain,
    #[error("path no longer exists")]
    Missing,
    #[error("path goes through a symlinked folder")]
    SymlinkEscape,
    #[error("this folder is protected")]
    Protected,
    #[error("path is outside what was scanned")]
    NotAllowed,
}

/// Top-level folders that are never trashed themselves (their contents can be).
const PROTECTED_IN_HOME: &[&str] = &[
    "Library",
    "Desktop",
    "Documents",
    "Downloads",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Applications",
    ".Trash",
];

/// Checks that `path` is safe to trash:
/// - absolute, with no `.`/`..` parts
/// - exists (a symlink itself is fine; trashing removes only the link)
/// - not reached through a symlinked parent folder
/// - strictly inside `home` and inside one of `roots`, and not a root itself
/// - not `home`, one of its standard folders, or anything above `home`
pub fn check(path: &Path, home: &Path, roots: &[PathBuf]) -> Result<(), GuardError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
    {
        return Err(GuardError::NotPlain);
    }
    if path.symlink_metadata().is_err() {
        return Err(GuardError::Missing);
    }
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(GuardError::Protected);
    };
    let real_parent = parent
        .canonicalize()
        .map_err(|_| GuardError::SymlinkEscape)?;
    if real_parent.join(name) != path {
        return Err(GuardError::SymlinkEscape);
    }
    if path == home
        || home.starts_with(path)
        || PROTECTED_IN_HOME.iter().any(|p| path == home.join(p))
    {
        return Err(GuardError::Protected);
    }
    if !path.starts_with(home) {
        return Err(GuardError::NotAllowed);
    }
    if roots.iter().any(|r| r == path) {
        return Err(GuardError::Protected);
    }
    if !roots.iter().any(|r| path.starts_with(r)) {
        return Err(GuardError::NotAllowed);
    }
    Ok(())
}

#[derive(Debug, Serialize, Clone)]
pub struct Skip {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashReport {
    pub moved: Vec<String>,
    pub bytes_moved: u64,
    pub skipped: Vec<Skip>,
    pub failed: Vec<Skip>,
}

/// Moves each path to the Trash after `allow` approves it. `trasher` does the move,
/// so tests can swap in a fake one.
pub fn trash_paths(
    paths: &[PathBuf],
    allow: impl Fn(&Path) -> Result<(), String>,
    trasher: impl Fn(&Path) -> Result<(), String>,
) -> TrashReport {
    let mut report = TrashReport::default();
    for path in paths {
        let shown = path.display().to_string();
        if let Err(reason) = allow(path) {
            report.skipped.push(Skip {
                path: shown,
                reason,
            });
            continue;
        }
        let bytes = fsx::usage(path).bytes;
        match trasher(path) {
            Ok(()) => {
                report.moved.push(shown);
                report.bytes_moved += bytes;
            }
            Err(reason) => report.failed.push(Skip {
                path: shown,
                reason,
            }),
        }
    }
    report
}

/// The real trasher: macOS Trash via Finder's API, so items can be put back.
pub fn system_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn home() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        fs::create_dir_all(home.join("Library/Caches/app")).unwrap();
        fs::write(home.join("Library/Caches/app/blob"), b"x").unwrap();
        (dir, home)
    }

    #[test]
    fn allows_items_inside_a_root() {
        let (_d, home) = home();
        let root = home.join("Library/Caches");
        assert_eq!(
            check(&root.join("app"), &home, std::slice::from_ref(&root)),
            Ok(())
        );
        assert_eq!(check(&root.join("app/blob"), &home, &[root]), Ok(()));
    }

    #[test]
    fn refuses_roots_home_and_standard_folders() {
        let (_d, home) = home();
        let roots = [home.clone()];
        assert_eq!(check(&home, &home, &roots), Err(GuardError::Protected));
        assert_eq!(
            check(&home.join("Library"), &home, &roots),
            Err(GuardError::Protected)
        );
        let caches = home.join("Library/Caches");
        assert_eq!(
            check(&caches, &home, std::slice::from_ref(&caches)),
            Err(GuardError::Protected)
        );
    }

    #[test]
    fn refuses_paths_outside_roots_or_home() {
        let (_d, home) = home();
        let other = tempfile::tempdir().unwrap();
        let outside = other.path().canonicalize().unwrap().join("f");
        fs::write(&outside, b"x").unwrap();
        let root = home.join("Library/Caches");
        assert_eq!(
            check(&outside, &home, std::slice::from_ref(&root)),
            Err(GuardError::NotAllowed)
        );
        fs::create_dir_all(home.join("Works/p")).unwrap();
        assert_eq!(
            check(&home.join("Works/p"), &home, &[root]),
            Err(GuardError::NotAllowed)
        );
    }

    #[test]
    fn refuses_symlink_escape_and_dot_dot() {
        let (_d, home) = home();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("secret"), b"x").unwrap();
        let root = home.join("Library/Caches");
        std::os::unix::fs::symlink(other.path(), root.join("escape")).unwrap();
        assert_eq!(
            check(
                &root.join("escape/secret"),
                &home,
                std::slice::from_ref(&root)
            ),
            Err(GuardError::SymlinkEscape)
        );
        // The link itself may be trashed: only the link goes.
        assert_eq!(
            check(&root.join("escape"), &home, std::slice::from_ref(&root)),
            Ok(())
        );
        assert_eq!(
            check(&root.join("app/../app"), &home, &[root]),
            Err(GuardError::NotPlain)
        );
    }

    #[test]
    fn trash_paths_reports_moved_skipped_failed() {
        let (_d, home) = home();
        let ok = home.join("Library/Caches/app/blob");
        let bad = home.join("Library/Caches/app");
        let report = trash_paths(
            &[ok.clone(), bad.clone(), PathBuf::from("relative")],
            |p| {
                if p.is_absolute() {
                    Ok(())
                } else {
                    Err("no".into())
                }
            },
            |p| if p == bad { Err("busy".into()) } else { Ok(()) },
        );
        assert_eq!(report.moved, vec![ok.display().to_string()]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.skipped.len(), 1);
    }
}
