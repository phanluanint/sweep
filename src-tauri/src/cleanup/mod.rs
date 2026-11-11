//! Cleanup: find caches and other leftovers, then move chosen items to the Trash.

pub mod clean;
pub mod rules;
pub mod scan;

use std::collections::HashSet;
use std::path::Path;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Bundle ids of running apps, read from each app's Info.plist. Their cache folders
/// (usually named after the bundle id) are left alone while the app runs.
pub fn running_bundle_ids() -> HashSet<String> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    let mut apps = HashSet::new();
    for process in sys.processes().values() {
        let Some(exe) = process.exe() else { continue };
        if let Some(app) = app_bundle(exe) {
            apps.insert(app.to_path_buf());
        }
    }
    apps.iter()
        .filter_map(|app| bundle_id(&app.join("Contents/Info.plist")))
        .collect()
}

/// `/Applications/Foo.app/Contents/MacOS/Foo` -> `/Applications/Foo.app` (outermost app).
fn app_bundle(exe: &Path) -> Option<&Path> {
    exe.ancestors()
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .last()
}

fn bundle_id(info_plist: &Path) -> Option<String> {
    let value = plist::Value::from_file(info_plist).ok()?;
    value
        .as_dictionary()?
        .get("CFBundleIdentifier")?
        .as_string()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_outermost_app_bundle() {
        let exe = Path::new(
            "/Applications/Google Chrome.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper",
        );
        assert_eq!(
            app_bundle(exe),
            Some(Path::new("/Applications/Google Chrome.app"))
        );
        assert_eq!(app_bundle(Path::new("/usr/bin/zsh")), None);
    }
}
