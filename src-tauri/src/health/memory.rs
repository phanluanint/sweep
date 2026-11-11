//! Memory breakdown in the same terms Activity Monitor uses.

use serde::Serialize;

use super::sysctl;

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    pub total: u64,
    pub used: u64,
    pub app: u64,
    pub wired: u64,
    pub compressed: u64,
    pub cached: u64,
    pub free: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    /// 0–100, higher means more pressure.
    pub pressure_pct: u8,
    /// normal | warning | critical
    pub pressure_level: &'static str,
}

/// `vm_statistics64` from <mach/vm_statistics.h>.
#[repr(C)]
#[derive(Default)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

const _: () = assert!(std::mem::size_of::<VmStatistics64>() == 152);
const HOST_VM_INFO64: i32 = 4;
const HOST_VM_INFO64_COUNT: u32 = (std::mem::size_of::<VmStatistics64>() / 4) as u32;

unsafe extern "C" {
    fn mach_host_self() -> u32;
    fn host_statistics64(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
}

fn vm_stats() -> Option<VmStatistics64> {
    let mut stats = VmStatistics64::default();
    let mut count = HOST_VM_INFO64_COUNT;
    // SAFETY: stats is a correctly sized vm_statistics64 buffer and count holds its length
    // in integer_t units, as host_statistics64 expects.
    let rc = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            (&raw mut stats).cast(),
            &mut count,
        )
    };
    (rc == 0).then_some(stats)
}

/// Swap totals come from sysinfo, which reads `vm.swapusage`.
pub fn sample(swap_total: u64, swap_used: u64) -> Option<MemorySnapshot> {
    let stats = vm_stats()?;
    let page = sysctl::int("hw.pagesize").unwrap_or(16_384) as u64;
    let total = sysctl::int("hw.memsize")? as u64;
    let pages = |n: u32| n as u64 * page;

    let app = pages(
        stats
            .internal_page_count
            .saturating_sub(stats.purgeable_count),
    );
    let wired = pages(stats.wire_count);
    let compressed = pages(stats.compressor_page_count);
    let cached = pages(stats.external_page_count + stats.purgeable_count);
    let used = app + wired + compressed;
    let free = total.saturating_sub(used + cached);

    // kern.memorystatus_level is the percentage of memory still available.
    let available = sysctl::int("kern.memorystatus_level")
        .unwrap_or(100)
        .clamp(0, 100);
    let pressure_pct = (100 - available) as u8;
    let pressure_level = match sysctl::int("kern.memorystatus_vm_pressure_level") {
        Some(4) => "critical",
        Some(2) => "warning",
        _ => "normal",
    };

    Some(MemorySnapshot {
        total,
        used,
        app,
        wired,
        compressed,
        cached,
        free,
        swap_total,
        swap_used,
        pressure_pct,
        pressure_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakdown_fits_in_total() {
        let m = sample(0, 0).expect("vm stats");
        assert!(m.total > 0);
        assert!(m.used > 0 && m.used <= m.total);
        assert!(m.app + m.wired + m.compressed + m.cached <= m.total + m.total / 10);
    }
}
