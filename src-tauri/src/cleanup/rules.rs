//! The fixed list of cleanup groups and the folders each one looks in.
//!
//! Every target is a folder under the home directory. The *direct children* of that
//! folder are the items that can be trashed, so the folder itself always stays.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Rebuilt automatically; selected by default.
    Safe,
    /// Might be wanted; shown unselected for the user to review.
    Review,
    /// Shown for information only; never cleaned by Sweep.
    Info,
}

#[derive(Debug)]
pub struct Target {
    /// Folder relative to the home directory.
    pub rel: &'static str,
    /// Only items not used for this many days.
    pub min_age_days: Option<u64>,
    /// Child names to leave out (claimed by another group).
    pub exclude: &'static [&'static str],
}

#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub risk: Risk,
    pub default_selected: bool,
    /// False for groups planned but not built yet.
    pub available: bool,
    pub targets: &'static [Target],
}

impl Rule {
    pub fn cleanable(&self) -> bool {
        self.available && self.risk != Risk::Info
    }
}

const fn t(rel: &'static str) -> Target {
    Target {
        rel,
        min_age_days: None,
        exclude: &[],
    }
}

/// Browser cache folders inside ~/Library/Caches, owned by the Browser group.
const BROWSER_CACHES: &[&str] = &[
    "com.apple.Safari",
    "Google",
    "company.thebrowser.Browser",
    "Firefox",
];

pub const RULES: &[Rule] = &[
    Rule {
        id: "caches",
        label: "System & app caches",
        description: "~/Library/Caches · rebuilt by apps when needed",
        icon: "folder",
        risk: Risk::Safe,
        default_selected: true,
        available: true,
        targets: &[Target {
            rel: "Library/Caches",
            min_age_days: None,
            exclude: BROWSER_CACHES,
        }],
    },
    Rule {
        id: "logs",
        label: "Logs & crash reports",
        description: "~/Library/Logs · items older than 7 days",
        icon: "doc",
        risk: Risk::Safe,
        default_selected: true,
        available: true,
        targets: &[Target {
            rel: "Library/Logs",
            min_age_days: Some(7),
            exclude: &[],
        }],
    },
    Rule {
        id: "developer",
        label: "Developer caches",
        description: "Xcode DerivedData, simulator caches, Cargo, npm, pnpm store",
        icon: "code",
        risk: Risk::Safe,
        default_selected: true,
        available: true,
        targets: &[
            t("Library/Developer/Xcode/DerivedData"),
            t("Library/Developer/CoreSimulator/Caches"),
            t(".cargo/registry/cache"),
            t(".npm/_cacache"),
            t("Library/pnpm/store"),
        ],
    },
    Rule {
        id: "browser",
        label: "Browser caches",
        description: "Safari, Chrome, Arc, Firefox · cookies and history are kept",
        icon: "globe",
        risk: Risk::Safe,
        default_selected: true,
        available: true,
        targets: &[
            t("Library/Caches/com.apple.Safari"),
            t("Library/Caches/Google/Chrome"),
            t("Library/Caches/company.thebrowser.Browser"),
            t("Library/Caches/Firefox/Profiles"),
        ],
    },
    Rule {
        id: "trash",
        label: "Trash",
        description: "Shown for size only · empty it from Finder",
        icon: "trash",
        risk: Risk::Info,
        default_selected: false,
        available: true,
        targets: &[t(".Trash")],
    },
    Rule {
        id: "downloads",
        label: "Old downloads",
        description: "~/Downloads, not used for 90 days",
        icon: "download",
        risk: Risk::Review,
        default_selected: false,
        available: true,
        targets: &[Target {
            rel: "Downloads",
            min_age_days: Some(90),
            exclude: &[],
        }],
    },
    Rule {
        id: "docker",
        label: "Docker unused images & build cache",
        description: "Coming later · will run docker system prune",
        icon: "box",
        risk: Risk::Review,
        default_selected: false,
        available: false,
        targets: &[],
    },
    Rule {
        id: "homebrew",
        label: "Homebrew downloads",
        description: "Coming later · will run brew cleanup",
        icon: "box",
        risk: Risk::Safe,
        default_selected: false,
        available: false,
        targets: &[],
    },
];

pub fn find(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<_> = RULES.iter().map(|r| r.id).collect();
        assert_eq!(ids.len(), RULES.len());
    }

    #[test]
    fn targets_are_relative_and_plain() {
        for rule in RULES {
            for target in rule.targets {
                assert!(!target.rel.starts_with('/'), "{}", target.rel);
                assert!(!target.rel.contains(".."), "{}", target.rel);
            }
        }
    }

    #[test]
    fn review_and_info_groups_start_unselected() {
        for rule in RULES.iter().filter(|r| r.risk != Risk::Safe) {
            assert!(!rule.default_selected, "{}", rule.id);
        }
    }

    #[test]
    fn browser_caches_are_not_double_counted() {
        let caches = find("caches").unwrap();
        for target in find("browser").unwrap().targets {
            let first = target.rel.trim_start_matches("Library/Caches/");
            let first = first.split('/').next().unwrap();
            assert!(caches.targets[0].exclude.contains(&first), "{first}");
        }
    }
}
