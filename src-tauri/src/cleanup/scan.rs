//! Measures each cleanup group and lists the items it would trash.

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

use super::rules::{RULES, Risk, Rule};
use crate::fsx;

#[derive(Debug, Serialize, Clone)]
pub struct Item {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroupScan {
    pub id: String,
    pub label: String,
    pub description: String,
    pub icon: String,
    pub risk: Risk,
    pub default_selected: bool,
    pub available: bool,
    pub cleanable: bool,
    pub bytes: u64,
    pub files: u64,
    /// Largest first.
    pub items: Vec<Item>,
    /// Cache folders left alone because their app is running.
    pub skipped_in_use: Vec<String>,
    /// Entries that could not be read (often missing Full Disk Access).
    pub errors: u64,
}

pub fn scan_rule(rule: &Rule, home: &Path, running: &HashSet<String>, now: i64) -> GroupScan {
    let mut group = GroupScan {
        id: rule.id.into(),
        label: rule.label.into(),
        description: rule.description.into(),
        icon: rule.icon.into(),
        risk: rule.risk,
        default_selected: rule.default_selected,
        available: rule.available,
        cleanable: rule.cleanable(),
        bytes: 0,
        files: 0,
        items: Vec::new(),
        skipped_in_use: Vec::new(),
        errors: 0,
    };
    for target in rule.targets {
        let dir = home.join(target.rel);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            // Missing folders are normal (tool not installed); unreadable ones count as errors.
            if dir.symlink_metadata().is_ok() {
                group.errors += 1;
            }
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                group.errors += 1;
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".DS_Store" || target.exclude.contains(&name.as_str()) {
                continue;
            }
            if running.contains(&name) {
                group.skipped_in_use.push(name);
                continue;
            }
            let path = entry.path();
            let Ok(meta) = path.symlink_metadata() else {
                group.errors += 1;
                continue;
            };
            if let Some(days) = target.min_age_days {
                let age_days = (now - fsx::last_used_secs(&meta)).max(0) / 86_400;
                if (age_days as u64) < days {
                    continue;
                }
            }
            let usage = fsx::usage(&path);
            group.errors += usage.errors;
            if usage.bytes == 0 && usage.files == 0 {
                continue;
            }
            group.bytes += usage.bytes;
            group.files += usage.files;
            group.items.push(Item {
                path: path.display().to_string(),
                name,
                bytes: usage.bytes,
                files: usage.files,
            });
        }
    }
    group.items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    group
}

/// Scans every group in parallel, keeping the order of [`RULES`].
pub fn scan_all(home: &Path, running: &HashSet<String>) -> Vec<GroupScan> {
    let now = fsx::now_secs();
    std::thread::scope(|s| {
        let handles: Vec<_> = RULES
            .iter()
            .map(|rule| s.spawn(move || scan_rule(rule, home, running, now)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("cleanup scan thread panicked"))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::rules::{Target, find};
    use std::fs;

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; len]).unwrap();
    }

    #[test]
    fn lists_children_and_skips_excluded_and_running() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        write(&home.join("Library/Caches/com.example.a/blob"), 64 * 1024);
        write(&home.join("Library/Caches/com.example.running/blob"), 4096);
        write(&home.join("Library/Caches/Google/Chrome/x"), 4096);
        let running: HashSet<String> = ["com.example.running".to_string()].into();
        let g = scan_rule(find("caches").unwrap(), home, &running, fsx::now_secs());
        let names: Vec<_> = g.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.a"]);
        assert_eq!(g.skipped_in_use, vec!["com.example.running"]);
        assert!(g.bytes >= 64 * 1024);
        assert!(g.cleanable);
    }

    #[test]
    fn age_filter_keeps_only_old_items() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        write(&home.join("Downloads/new.zip"), 4096);
        static TARGETS: [Target; 1] = [Target {
            rel: "Downloads",
            min_age_days: Some(90),
            exclude: &[],
        }];
        let rule = Rule {
            targets: &TARGETS,
            ..*find("downloads").unwrap()
        };
        let none = scan_rule(&rule, home, &HashSet::new(), fsx::now_secs());
        assert!(none.items.is_empty());
        let later = fsx::now_secs() + 91 * 86_400;
        let some = scan_rule(&rule, home, &HashSet::new(), later);
        assert_eq!(some.items.len(), 1);
    }

    #[test]
    fn missing_folders_are_not_errors() {
        let dir = tempfile::tempdir().unwrap();
        let g = scan_rule(
            find("developer").unwrap(),
            dir.path(),
            &HashSet::new(),
            fsx::now_secs(),
        );
        assert_eq!(g.errors, 0);
        assert!(g.items.is_empty());
    }
}
