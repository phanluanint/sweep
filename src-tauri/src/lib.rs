//! Sweep: Mac cleanup, disk map and read-only health monitor.

pub mod cleanup;
mod commands;
pub mod disk;
pub mod fsx;
pub mod health;
pub mod safety;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use commands::AppState;

/// Samples health on its own thread and emits `health://tick` at the chosen interval.
fn start_health_loop(app: AppHandle, interval_ms: Arc<AtomicU64>) {
    std::thread::Builder::new()
        .name("health".into())
        .spawn(move || {
            let mut sampler = health::Sampler::new();
            loop {
                let ms = interval_ms.load(Ordering::Relaxed);
                if ms == 0 {
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }
                let snapshot = sampler.sample();
                let _ = app.emit("health://tick", &snapshot);
                std::thread::sleep(Duration::from_millis(ms));
            }
        })
        .expect("spawn health thread");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let home = dirs::home_dir().expect("home directory");
    tauri::Builder::default()
        .manage(AppState::new(home))
        .setup(|app| {
            let interval = Arc::clone(&app.state::<AppState>().health_interval_ms);
            start_health_loop(app.handle().clone(), interval);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::home_dir,
            commands::volume_info,
            commands::cleanup_scan,
            commands::cleanup_clean,
            commands::disk_scan,
            commands::disk_cancel,
            commands::disk_children,
            commands::disk_largest,
            commands::disk_trash,
            commands::reveal_in_finder,
            commands::set_health_interval,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sweep");
}
