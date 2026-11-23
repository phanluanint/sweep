//! The fixed list of cleanup groups and the folders each one looks in.
//!
//! Every target is a folder under the home directory. The *direct children* of that
//! folder are the items that can be trashed, so the folder itself always stays.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Rebuilt automatically.
    Safe,
    /// Might be wanted; review before moving it to Trash.
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
    pub browser: Option<BrowserCache>,
}

#[derive(Debug, Clone, Copy)]
pub struct BrowserCache {
    pub owner: &'static str,
    pub layout: CacheLayout,
    pub names: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub enum CacheLayout {
    Direct,
    ChromiumProfiles,
    FirefoxProfiles,
}

#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub risk: Risk,
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
        browser: None,
    }
}

const fn browser(
    rel: &'static str,
    owner: &'static str,
    layout: CacheLayout,
    names: &'static [&'static str],
) -> Target {
    Target {
        browser: Some(BrowserCache {
            owner,
            layout,
            names,
        }),
        ..t(rel)
    }
}

const CHROMIUM_CACHE_NAMES: &[&str] = &["Cache", "Code Cache", "GPUCache"];

/// Browser cache folders inside ~/Library/Caches, owned by the Browser group.
const BROWSER_CACHES: &[&str] = &[
    "com.apple.Safari",
    "Google",
    "company.thebrowser.Browser",
    "Arc",
    "Firefox",
];

pub const RULES: &[Rule] = &[
    Rule {
        id: "caches",
        label: "System & app caches",
        description: "~/Library/Caches · rebuilt by apps when needed",
        icon: "folder",
        risk: Risk::Safe,
        available: true,
        targets: &[Target {
            rel: "Library/Caches",
            min_age_days: None,
            exclude: BROWSER_CACHES,
            browser: None,
        }],
    },
    Rule {
        id: "logs",
        label: "Logs & crash reports",
        description: "~/Library/Logs · items older than 7 days",
        icon: "doc",
        risk: Risk::Safe,
        available: true,
        targets: &[Target {
            rel: "Library/Logs",
            min_age_days: Some(7),
            exclude: &[],
            browser: None,
        }],
    },
    Rule {
        id: "developer",
        label: "Developer caches",
        description: "Xcode DerivedData, simulator caches, Cargo, npm, pnpm store",
        icon: "code",
        risk: Risk::Safe,
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
        available: true,
        targets: &[
            browser(
                "Library/Caches/com.apple.Safari",
                "com.apple.Safari",
                CacheLayout::Direct,
                &["WebKitCache"],
            ),
            browser(
                "Library/Containers/com.apple.Safari/Data/Library/Caches/com.apple.Safari",
                "com.apple.Safari",
                CacheLayout::Direct,
                &["WebKitCache"],
            ),
            browser(
                "Library/Caches/Google/Chrome",
                "com.google.Chrome",
                CacheLayout::ChromiumProfiles,
                CHROMIUM_CACHE_NAMES,
            ),
            browser(
                "Library/Caches/company.thebrowser.Browser",
                "company.thebrowser.Browser",
                CacheLayout::Direct,
                CHROMIUM_CACHE_NAMES,
            ),
            browser(
                "Library/Caches/Arc/User Data",
                "company.thebrowser.Browser",
                CacheLayout::ChromiumProfiles,
                CHROMIUM_CACHE_NAMES,
            ),
            browser(
                "Library/Caches/Firefox/Profiles",
                "org.mozilla.firefox",
                CacheLayout::FirefoxProfiles,
                &["cache2"],
            ),
        ],
    },
    Rule {
        id: "trash",
        label: "Trash",
        description: "Shown for size only · empty it from Finder",
        icon: "trash",
        risk: Risk::Info,
        available: true,
        targets: &[t(".Trash")],
    },
    Rule {
        id: "downloads",
        label: "Old downloads",
        description: "~/Downloads, not used for 90 days",
        icon: "download",
        risk: Risk::Review,
        available: true,
        targets: &[Target {
            rel: "Downloads",
            min_age_days: Some(90),
            exclude: &[],
            browser: None,
        }],
    },
    Rule {
        id: "docker",
        label: "Docker unused images & build cache",
        description: "Coming later · will run docker system prune",
        icon: "box",
        risk: Risk::Review,
        available: false,
        targets: &[],
    },
    Rule {
        id: "homebrew",
        label: "Homebrew downloads",
        description: "Coming later · will run brew cleanup",
        icon: "box",
        risk: Risk::Safe,
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
    fn browser_caches_are_not_double_counted() {
        let caches = find("caches").unwrap();
        for target in find("browser").unwrap().targets {
            let Some(first) = target.rel.strip_prefix("Library/Caches/") else {
                continue;
            };
            let first = first.split('/').next().unwrap();
            assert!(caches.targets[0].exclude.contains(&first), "{first}");
        }
    }
}
