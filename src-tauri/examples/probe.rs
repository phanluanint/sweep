//! Prints one health sample and the cleanup scan as JSON, without the UI.
//! `cargo run --example probe`

use std::time::Duration;

fn main() {
    let mut sampler = sweep_lib::health::Sampler::new();
    std::thread::sleep(Duration::from_millis(1000));
    let health = sampler.sample();
    println!("{}", serde_json::to_string_pretty(&health).unwrap());

    let home = dirs::home_dir().expect("home");
    let running = sweep_lib::cleanup::running_bundle_ids();
    for g in sweep_lib::cleanup::scan::scan_all(&home, &running) {
        println!(
            "{:<12} {:>12} bytes {:>8} files {:>4} items  errors={} in_use={}",
            g.id,
            g.bytes,
            g.files,
            g.items.len(),
            g.errors,
            g.skipped_in_use.len()
        );
    }
}
