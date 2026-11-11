//! Health: read-only CPU, memory, fan and process readings.

pub mod cpu;
pub mod memory;
pub mod processes;
pub mod smc;
pub mod sysctl;

use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HealthSnapshot {
    pub model: String,
    pub cpu: cpu::CpuSnapshot,
    pub cpu_temp_c: Option<f32>,
    pub memory: Option<memory::MemorySnapshot>,
    /// `None` = fans could not be read; empty = this Mac has no fans.
    pub fans: Option<Vec<smc::Fan>>,
    pub top_cpu: Vec<processes::ProcInfo>,
    pub top_memory: Vec<processes::ProcInfo>,
}

/// Keeps the state that per-interval readings need (previous CPU ticks, SMC handle).
pub struct Sampler {
    sys: System,
    smc: Option<smc::Smc>,
    model: String,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let model = sysctl::string("hw.model").unwrap_or_default();
        Self {
            sys,
            smc: smc::Smc::open(),
            model,
        }
    }

    pub fn sample(&mut self) -> HealthSnapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let (top_cpu, top_memory) = processes::top(&self.sys, 5);
        HealthSnapshot {
            model: self.model.clone(),
            cpu: cpu::sample(&self.sys),
            cpu_temp_c: self.smc.as_ref().and_then(smc::cpu_temperature),
            memory: memory::sample(self.sys.total_swap(), self.sys.used_swap()),
            fans: self.smc.as_ref().and_then(smc::fans),
            top_cpu,
            top_memory,
        }
    }
}
