//! Measures each cleanup group and lists the items it would trash.

use std::collections::HashSet;
use std::fs::{self, Metadata, ReadDir};
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::SystemTime;

use serde::Serialize;

use super::rules::{CacheLayout, RULES, Risk, Rule};
use crate::fsx;

#[derive(Debug, Serialize, Clone)]
pub struct Item {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub files: u64,
    #[serde(skip)]
    pub(super) owner: Option<String>,
    #[serde(skip)]
    pub(super) stamp: FileStamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileStamp {
    device: u64,
    inode: u64,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
}

impl FileStamp {
    pub(super) fn from_meta(meta: &Metadata) -> Self {
        Self {
            device: meta.dev(),
            inode: meta.ino(),
            created: meta.created().ok(),
            modified: meta.modified().ok(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroupScan {
    pub id: String,
    pub label: String,
    pub description: String,
    pub icon: String,
    pub risk: Risk,
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
        available: rule.available,
        cleanable: rule.cleanable(),
        bytes: 0,
        files: 0,
        items: Vec::new(),
        skipped_in_use: Vec::new(),
        errors: 0,
    };
    for target in rule.targets {
        let cutoff = target.min_age_days.map(|days| now - days as i64 * 86_400);
        let root = home.join(target.rel);
        let Some(entries) = read_dir(&root, &mut group.errors) else {
            continue;
        };
        if let Some(browser) = target.browser {
            if running.contains(browser.owner) {
                if !group
                    .skipped_in_use
                    .iter()
                    .any(|owner| owner == browser.owner)
                {
                    group.skipped_in_use.push(browser.owner.into());
                }
                continue;
            }
            if matches!(browser.layout, CacheLayout::Direct) {
                for name in browser.names {
                    group.add_item(&root.join(name), Some(browser.owner), None);
                }
                continue;
            }
            for entry in entries {
                let Ok(entry) = entry else {
                    group.errors += 1;
                    continue;
                };
                if matches!(browser.layout, CacheLayout::ChromiumProfiles)
                    && !entry.file_name().to_str().is_some_and(is_chromium_profile)
                {
                    continue;
                }
                let Ok(kind) = entry.file_type() else {
                    group.errors += 1;
                    continue;
                };
                if !kind.is_dir() {
                    continue;
                }
                for name in browser.names {
                    group.add_item(&entry.path().join(name), Some(browser.owner), None);
                }
            }
            continue;
        }
        for entry in entries {
            let Ok(entry) = entry else {
                group.errors += 1;
                continue;
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                group.errors += 1;
                continue;
            };
            if name == ".DS_Store" || target.exclude.contains(&name) {
                continue;
            }
            let owner = (rule.id == "caches").then_some(name);
            if let Some(owner) = owner.filter(|owner| running.contains(*owner)) {
                group.skipped_in_use.push(owner.into());
                continue;
            }
            group.add_item(&entry.path(), owner, cutoff);
        }
    }
    group.items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    group
}

fn read_dir(path: &Path, errors: &mut u64) -> Option<ReadDir> {
    let entries = match path.canonicalize() {
        Ok(canonical) if canonical == path => fs::read_dir(path),
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        _ => {
            *errors += 1;
            return None;
        }
    };
    match entries {
        Ok(entries) => Some(entries),
        Err(_) => {
            *errors += 1;
            None
        }
    }
}

fn is_chromium_profile(name: &str) -> bool {
    matches!(name, "Default" | "Guest Profile" | "System Profile")
        || name.strip_prefix("Profile ").is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|digit| digit.is_ascii_digit())
        })
}

impl GroupScan {
    fn add_item(&mut self, path: &Path, owner: Option<&str>, cutoff: Option<i64>) {
        let Some(path_str) = path.to_str() else {
            self.errors += 1;
            return;
        };
        let meta = match path.symlink_metadata() {
            Ok(meta) => meta,
            Err(error) if error.kind() == ErrorKind::NotFound => return,
            Err(_) => {
                self.errors += 1;
                return;
            }
        };
        if !(meta.is_dir() || self.id != "browser" && meta.is_file())
            || cutoff.is_some_and(|cutoff| fsx::last_used_secs(&meta) > cutoff)
        {
            return;
        }
        let usage = fsx::usage(path);
        self.errors += usage.errors;
        if usage.bytes == 0 && usage.files == 0 {
            return;
        }
        self.bytes += usage.bytes;
        self.files += usage.files;
        self.items.push(Item {
            path: path_str.into(),
            name: path
                .file_name()
                .expect("cleanup item has a name")
                .to_string_lossy()
                .into_owned(),
            bytes: usage.bytes,
            files: usage.files,
            owner: owner.map(str::to_owned),
            stamp: FileStamp::from_meta(&meta),
        });
    }
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

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; len]).unwrap();
    }

    #[test]
    fn lists_children_and_skips_excluded_and_running() {
        let dir = tempfile::tempdir().unwrap();
        let home = &dir.path().canonicalize().unwrap();
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
    fn browser_scan_only_lists_known_caches_and_skips_open_browsers() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let chrome = home.join("Library/Caches/Google/Chrome");
        let firefox = home.join("Library/Caches/Firefox/Profiles/abc.default-release");
        let chrome_cache = chrome.join("Profile 1/Cache");
        let firefox_cache = firefox.join("cache2");
        write(&chrome_cache.join("data"), 4096);
        write(&chrome.join("Default/Network/Cookies"), 4096);
        write(&chrome.join("Other/Cache/data"), 4096);
        write(&firefox_cache.join("entries/data"), 4096);
        write(&firefox.join("places.sqlite"), 4096);
        std::os::unix::fs::symlink(&firefox, chrome.join("Profile 2")).unwrap();

        let rule = find("browser").unwrap();
        let group = scan_rule(rule, &home, &HashSet::new(), fsx::now_secs());
        let paths: HashSet<_> = group
            .items
            .iter()
            .map(|item| Path::new(&item.path))
            .collect();
        assert_eq!(
            paths,
            HashSet::from([chrome_cache.as_path(), firefox_cache.as_path()])
        );

        let running = HashSet::from(["com.google.Chrome".into()]);
        let group = scan_rule(rule, &home, &running, fsx::now_secs());
        assert_eq!(group.skipped_in_use, ["com.google.Chrome"]);
        assert_eq!(group.items.len(), 1);
        assert_eq!(Path::new(&group.items[0].path), firefox_cache);
    }

    #[test]
    fn scan_skips_roots_reached_through_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(&outside.path().join("Caches/com.example.a/blob"), 4096);
        std::os::unix::fs::symlink(outside.path(), home.join("Library")).unwrap();

        let group = scan_rule(
            find("caches").unwrap(),
            &home,
            &HashSet::new(),
            fsx::now_secs(),
        );
        assert!(group.items.is_empty());
        assert_eq!(group.errors, 1);
    }

    #[test]
    fn age_filter_keeps_only_old_items() {
        let dir = tempfile::tempdir().unwrap();
        let home = &dir.path().canonicalize().unwrap();
        write(&home.join("Downloads/new.zip"), 4096);
        static TARGETS: [Target; 1] = [Target {
            rel: "Downloads",
            min_age_days: Some(90),
            exclude: &[],
            browser: None,
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
            &dir.path().canonicalize().unwrap(),
            &HashSet::new(),
            fsx::now_secs(),
        );
        assert_eq!(g.errors, 0);
        assert!(g.items.is_empty());
    }
}
