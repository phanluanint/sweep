//! CPU load per core, core layout and load average.

use serde::Serialize;
use sysinfo::System;

use super::sysctl;

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CpuSnapshot {
    pub brand: String,
    /// Average load across cores, 0–100.
    pub total: f32,
    pub per_core: Vec<f32>,
    pub performance_cores: u32,
    pub efficiency_cores: u32,
    pub load_average: [f64; 3],
    pub uptime_secs: u64,
}

/// Apple silicon reports performance cores as perflevel0 and efficiency cores as perflevel1.
fn core_split() -> (u32, u32) {
    let p = sysctl::int("hw.perflevel0.logicalcpu").unwrap_or(0) as u32;
    let e = sysctl::int("hw.perflevel1.logicalcpu").unwrap_or(0) as u32;
    (p, e)
}

/// Needs `sys.refresh_cpu_usage()` called at least twice, some time apart.
pub fn sample(sys: &System) -> CpuSnapshot {
    let (performance_cores, efficiency_cores) = core_split();
    let load = System::load_average();
    CpuSnapshot {
        brand: sysctl::string("machdep.cpu.brand_string").unwrap_or_default(),
        total: sys.global_cpu_usage(),
        per_core: sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
        performance_cores,
        efficiency_cores,
        load_average: [load.one, load.five, load.fifteen],
        uptime_secs: System::uptime(),
    }
}
