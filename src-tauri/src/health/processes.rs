//! Top processes by CPU and by memory.

use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Serialize, Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    /// Percent of one core, as Activity Monitor shows it.
    pub cpu: f32,
    pub memory: u64,
}

pub fn top(sys: &System, n: usize) -> (Vec<ProcInfo>, Vec<ProcInfo>) {
    let all: Vec<ProcInfo> = sys
        .processes()
        .values()
        .map(|p| ProcInfo {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            cpu: p.cpu_usage(),
            memory: p.memory(),
        })
        .collect();
    let mut by_cpu = all.clone();
    by_cpu.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    by_cpu.truncate(n);
    let mut by_memory = all;
    by_memory.sort_by(|a, b| b.memory.cmp(&a.memory));
    by_memory.truncate(n);
    (by_cpu, by_memory)
}
