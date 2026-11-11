//! The Tauri command layer. Kept thin: each command validates input, runs the core
//! module on a blocking thread, and stores what later commands need.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::cleanup::{self, clean::Allowed, scan::GroupScan};
use crate::disk::{
    self,
    largest::FileInfo,
    walk::{Child, ScanStats, Tree},
};
use crate::safety::{self, TrashReport};

pub struct AppState {
    pub home: PathBuf,
    pub cleanup: Mutex<Allowed>,
    pub disk: Mutex<Option<Tree>>,
    pub disk_cancel: Arc<AtomicBool>,
    pub health_interval_ms: Arc<AtomicU64>,
}

impl AppState {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            cleanup: Mutex::default(),
            disk: Mutex::default(),
            disk_cancel: Arc::default(),
            health_interval_ms: Arc::new(AtomicU64::new(2000)),
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn to_paths(paths: Vec<String>) -> Vec<PathBuf> {
    paths.into_iter().map(PathBuf::from).collect()
}

#[tauri::command]
pub fn home_dir(state: State<'_, AppState>) -> String {
    state.home.display().to_string()
}

/// Capacity of the volume holding the home folder.
#[tauri::command]
pub fn volume_info(state: State<'_, AppState>) -> Result<crate::fsx::VolumeInfo, String> {
    crate::fsx::volume(&state.home).ok_or_else(|| "cannot read volume".into())
}

// ---- Cleanup -------------------------------------------------------------

#[tauri::command]
pub async fn cleanup_scan(app: AppHandle) -> Result<Vec<GroupScan>, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let running = cleanup::running_bundle_ids();
        let groups = cleanup::scan::scan_all(&state.home, &running);
        *state.cleanup.lock().map_err(|e| e.to_string())? =
            Allowed::from_scan(&state.home, &groups);
        Ok(groups)
    })
    .await
}

#[tauri::command]
pub async fn cleanup_clean(app: AppHandle, paths: Vec<String>) -> Result<TrashReport, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let mut allowed = state.cleanup.lock().map_err(|e| e.to_string())?;
        Ok(cleanup::clean::clean(
            &to_paths(paths),
            &state.home,
            &mut allowed,
            safety::system_trash,
        ))
    })
    .await
}

// ---- Disk Map ------------------------------------------------------------

#[tauri::command]
pub async fn disk_scan(app: AppHandle, root: Option<String>) -> Result<ScanStats, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let root = root
            .map(PathBuf::from)
            .unwrap_or_else(|| state.home.clone());
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err(format!("{} is not a folder", root.display()));
        }
        state.disk_cancel.store(false, Ordering::Relaxed);
        let emitter = app.clone();
        let tree = disk::walk::scan(&root, disk::TOP_FILES, &state.disk_cancel, |p| {
            let _ = emitter.emit("disk://progress", p);
        })
        .map_err(|e| e.to_string())?;
        let stats = tree.stats.clone();
        *state.disk.lock().map_err(|e| e.to_string())? = Some(tree);
        Ok(stats)
    })
    .await
}

#[tauri::command]
pub fn disk_cancel(state: State<'_, AppState>) {
    state.disk_cancel.store(true, Ordering::Relaxed);
}

/// The folder itself, then its children. Errors if no scan covers it.
#[tauri::command]
pub fn disk_children(
    state: State<'_, AppState>,
    path: String,
) -> Result<(Child, Vec<Child>), String> {
    let guard = state.disk.lock().map_err(|e| e.to_string())?;
    let tree = guard.as_ref().ok_or("no scan yet")?;
    let path = Path::new(&path);
    let node = tree.node(path).ok_or("folder is not in the scan")?;
    let children = tree.children(path).unwrap_or_default();
    Ok((node, children))
}

#[tauri::command]
pub fn disk_largest(state: State<'_, AppState>) -> Result<Vec<FileInfo>, String> {
    let guard = state.disk.lock().map_err(|e| e.to_string())?;
    Ok(guard
        .as_ref()
        .map(|t| t.largest.clone())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn disk_trash(app: AppHandle, paths: Vec<String>) -> Result<TrashReport, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.disk.lock().map_err(|e| e.to_string())?;
        let tree = guard.as_mut().ok_or("no scan yet")?;
        let roots = [tree.root.clone()];
        let report = safety::trash_paths(
            &to_paths(paths),
            |p| safety::check(p, &state.home, &roots).map_err(|e| e.to_string()),
            safety::system_trash,
        );
        for moved in &report.moved {
            tree.forget(Path::new(moved));
        }
        Ok(report)
    })
    .await
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() || path.symlink_metadata().is_err() {
        return Err("file not found".into());
    }
    std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ---- Health --------------------------------------------------------------

/// Sets how often `health://tick` fires; 0 pauses it.
#[tauri::command]
pub fn set_health_interval(state: State<'_, AppState>, ms: u64) {
    let ms = if ms == 0 { 0 } else { ms.clamp(1000, 60_000) };
    state.health_interval_ms.store(ms, Ordering::Relaxed);
}
