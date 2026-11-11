//! Moves cleanup items to the Trash, but only items the last scan listed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::rules::RULES;
use super::scan::GroupScan;
use crate::safety::{self, TrashReport};

/// What the last scan allowed: the exact item paths of cleanable groups, and the
/// folders they came from (used as roots for the path guard).
#[derive(Debug, Default, Clone)]
pub struct Allowed {
    pub items: HashSet<PathBuf>,
    pub roots: Vec<PathBuf>,
}

impl Allowed {
    pub fn from_scan(home: &Path, groups: &[GroupScan]) -> Self {
        let mut allowed = Allowed::default();
        for group in groups.iter().filter(|g| g.cleanable) {
            allowed
                .items
                .extend(group.items.iter().map(|i| PathBuf::from(&i.path)));
        }
        allowed.roots = RULES
            .iter()
            .filter(|r| r.cleanable())
            .flat_map(|r| r.targets.iter().map(|t| home.join(t.rel)))
            .collect();
        allowed
    }
}

pub fn clean(
    paths: &[PathBuf],
    home: &Path,
    allowed: &mut Allowed,
    trasher: impl Fn(&Path) -> Result<(), String>,
) -> TrashReport {
    let report = safety::trash_paths(
        paths,
        |p| {
            if !allowed.items.contains(p) {
                return Err("not in the last scan; rescan first".into());
            }
            safety::check(p, home, &allowed.roots).map_err(|e| e.to_string())
        },
        trasher,
    );
    for moved in &report.moved {
        allowed.items.remove(Path::new(moved));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::scan::scan_all;
    use std::fs;

    #[test]
    fn only_scanned_items_are_trashed() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let item = home.join("Library/Caches/com.example.a");
        fs::create_dir_all(&item).unwrap();
        fs::write(item.join("blob"), vec![0u8; 8192]).unwrap();
        fs::create_dir_all(home.join("Documents/keep")).unwrap();

        let groups = scan_all(&home, &HashSet::new());
        let mut allowed = Allowed::from_scan(&home, &groups);
        assert!(allowed.items.contains(&item));

        let report = clean(
            &[item.clone(), home.join("Documents/keep"), item.join("blob")],
            &home,
            &mut allowed,
            |_| Ok(()),
        );
        assert_eq!(report.moved, vec![item.display().to_string()]);
        assert_eq!(report.skipped.len(), 2);
        assert!(!allowed.items.contains(&item), "a second clean must rescan");
    }

    #[test]
    fn info_groups_are_never_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        fs::create_dir_all(home.join(".Trash/old")).unwrap();
        fs::write(home.join(".Trash/old/f"), vec![0u8; 4096]).unwrap();
        let groups = scan_all(&home, &HashSet::new());
        let allowed = Allowed::from_scan(&home, &groups);
        assert!(!allowed.items.contains(&home.join(".Trash/old")));
    }
}
