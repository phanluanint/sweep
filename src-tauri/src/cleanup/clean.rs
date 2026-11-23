//! Moves cleanup items to the Trash, but only items the last scan listed.

use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::scan::{FileStamp, GroupScan, Item};
use crate::safety::{self, Skip, TrashReport};

// The Finder backend passes the paths as one AppleScript command argument.
const TRASH_BATCH_SIZE: usize = 32;

#[derive(Debug, Default)]
pub struct Allowed {
    items: HashMap<PathBuf, Item>,
}

impl Allowed {
    pub fn from_scan(groups: &[GroupScan]) -> Self {
        let items = groups
            .iter()
            .filter(|group| group.cleanable)
            .flat_map(|group| group.items.iter())
            .map(|item| (PathBuf::from(&item.path), item.clone()))
            .collect();
        Self { items }
    }

    fn eligible(
        &self,
        path: &Path,
        home: &Path,
        running: &HashSet<String>,
    ) -> Result<&Item, String> {
        let item = self
            .items
            .get(path)
            .ok_or("Item is no longer in the latest scan. Rescan first.")?;
        let path = Path::new(&item.path);
        let root = path
            .parent()
            .expect("scanned item has a parent")
            .to_path_buf();
        safety::check(path, home, &[root]).map_err(|error| error.to_string())?;
        if let Some(owner) = item.owner.as_ref().filter(|owner| running.contains(*owner)) {
            return Err(format!(
                "{owner} is open. Quit the app and rescan to move this cache."
            ));
        }
        let meta = path.symlink_metadata().map_err(|error| error.to_string())?;
        if !(meta.is_dir() || meta.is_file()) || FileStamp::from_meta(&meta) != item.stamp {
            return Err("Item changed since the scan. Rescan before moving it.".into());
        }
        Ok(item)
    }
}

pub fn clean(
    paths: &[PathBuf],
    home: &Path,
    allowed: &mut Allowed,
    running: &HashSet<String>,
    mut trasher: impl FnMut(&[PathBuf]) -> Result<(), String>,
) -> TrashReport {
    let mut report = TrashReport::default();
    let mut seen = HashSet::new();
    for chunk in paths.chunks(TRASH_BATCH_SIZE) {
        let mut batch = Vec::new();
        for path in chunk.iter().filter(|path| seen.insert(path.as_path())) {
            match allowed.eligible(path, home, running) {
                Ok(item) => batch.push(PathBuf::from(&item.path)),
                Err(reason) => report.skipped.push(Skip {
                    path: path.display().to_string(),
                    reason,
                }),
            }
        }
        if batch.is_empty() {
            continue;
        }
        match trasher(&batch) {
            Ok(()) => {
                for path in batch {
                    let item = allowed.items.remove(&path).expect("validated batch item");
                    report.bytes_moved += item.bytes;
                    report.moved.push(item.path);
                }
            }
            Err(error) => {
                // Finder may move part of a batch before returning an error.
                for path in batch {
                    let missing = path
                        .symlink_metadata()
                        .is_err_and(|e| e.kind() == ErrorKind::NotFound);
                    let reason = if missing {
                        allowed.items.remove(&path);
                        format!(
                            "Finder reported an error and the item is no longer at this path. Check Trash. {error}"
                        )
                    } else {
                        format!("Finder could not confirm this move. {error}")
                    };
                    report.failed.push(Skip {
                        path: path.display().to_string(),
                        reason,
                    });
                }
            }
        }
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
        let mut allowed = Allowed::from_scan(&groups);
        assert!(allowed.items.contains_key(&item));

        let report = clean(
            &[item.clone(), home.join("Documents/keep"), item.join("blob")],
            &home,
            &mut allowed,
            &HashSet::new(),
            |_| Ok(()),
        );
        assert_eq!(report.moved, vec![item.display().to_string()]);
        assert_eq!(report.skipped.len(), 2);
        assert!(
            !allowed.items.contains_key(&item),
            "a second clean must rescan"
        );
    }

    #[test]
    fn rechecks_open_apps_and_replaced_items() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let browser_cache = home.join("Library/Caches/Google/Chrome/Default/Cache");
        let app_cache = home.join("Library/Caches/com.example.app");
        fs::create_dir_all(&browser_cache).unwrap();
        fs::create_dir_all(&app_cache).unwrap();
        fs::write(browser_cache.join("data"), b"cache").unwrap();
        fs::write(app_cache.join("data"), b"cache").unwrap();
        let groups = scan_all(&home, &HashSet::new());
        let mut allowed = Allowed::from_scan(&groups);

        fs::rename(&app_cache, home.join("original")).unwrap();
        fs::create_dir(&app_cache).unwrap();
        let report = clean(
            &[browser_cache, app_cache],
            &home,
            &mut allowed,
            &HashSet::from(["com.google.Chrome".into()]),
            |_| panic!("ineligible items reached Finder"),
        );
        assert_eq!(report.skipped.len(), 2);
        assert!(report.skipped[0].reason.contains("Chrome is open"));
        assert!(report.skipped[1].reason.contains("changed since the scan"));
    }

    #[test]
    fn batches_unique_items_and_reports_partial_failures_without_retrying() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let root = home.join("Library/Caches");
        fs::create_dir_all(&root).unwrap();
        let mut paths: Vec<_> = (0..=TRASH_BATCH_SIZE)
            .map(|number| root.join(number.to_string()))
            .collect();
        for path in &paths {
            fs::write(path, b"cache").unwrap();
        }
        let groups = scan_all(&home, &HashSet::new());
        let mut allowed = Allowed::from_scan(&groups);
        let first = paths[0].clone();
        let last = paths[TRASH_BATCH_SIZE].clone();
        let last_bytes = allowed.items[&last].bytes;
        paths.insert(1, first.clone());
        let mut batch_sizes = Vec::new();
        let report = clean(&paths, &home, &mut allowed, &HashSet::new(), |batch| {
            batch_sizes.push(batch.len());
            if batch[0] == first {
                fs::rename(&first, home.join("mock-trash")).unwrap();
                Err("partial move".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(batch_sizes, [TRASH_BATCH_SIZE - 1, 2]);
        assert_eq!(report.moved.len(), 2);
        assert_eq!(report.bytes_moved, last_bytes * 2);
        assert_eq!(report.failed.len(), TRASH_BATCH_SIZE - 1);
        assert!(report.failed[0].reason.contains("Check Trash"));
        assert!(report.failed[1].reason.contains("could not confirm"));
        assert!(!allowed.items.contains_key(&first));
        assert!(allowed.items.contains_key(&paths[2]));
        assert!(!allowed.items.contains_key(&last));
    }

    #[test]
    fn info_groups_are_never_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        fs::create_dir_all(home.join(".Trash/old")).unwrap();
        fs::write(home.join(".Trash/old/f"), vec![0u8; 4096]).unwrap();
        let groups = scan_all(&home, &HashSet::new());
        let allowed = Allowed::from_scan(&groups);
        assert!(!allowed.items.contains_key(&home.join(".Trash/old")));
    }
}
