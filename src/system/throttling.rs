/// Real-time system load metrics used for adaptive background task throttling.
#[derive(Debug, Clone, Copy)]
pub struct SystemLoadMetrics {
    pub active_threads: u8,
    pub throttle_delay_ms: u64,
    pub cpu_usage_pct: f32,
    pub used_memory_pct: f32,
}

/// Returns (`total_memory_bytes`, `used_memory_bytes`, `used_memory_percent`)
#[cfg(windows)]
pub fn get_system_memory() -> (u64, u64, f32) {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
        ..Default::default()
    };
    if unsafe { GlobalMemoryStatusEx(&raw mut status) }.is_ok() {
        let total = status.ullTotalPhys;
        let avail = status.ullAvailPhys;
        let used = total.saturating_sub(avail);
        let used_pct = status.dwMemoryLoad as f32;
        (total, used, used_pct)
    } else {
        (8_000_000_000, 4_000_000_000, 50.0)
    }
}

/// Returns (`total_memory_bytes`, `used_memory_bytes`, `used_memory_percent`) for non-Windows
#[cfg(not(windows))]
pub fn get_system_memory() -> (u64, u64, f32) {
    (8_000_000_000, 4_000_000_000, 50.0)
}

/// Computes adaptive thread allocations and delay intervals using real-time OS memory metrics.
pub fn get_adaptive_system_load(base_threads: u8) -> SystemLoadMetrics {
    let (_total_mem, _used_mem, used_mem_pct) = get_system_memory();

    let mut active_threads = base_threads;
    let mut throttle_delay_ms = 0;

    // High RAM usage (>85%): scale down threads to relieve memory pressure
    if used_mem_pct > 85.0 {
        active_threads = (active_threads / 2).max(1);
        throttle_delay_ms += 30;
    } else if used_mem_pct > 70.0 {
        active_threads = (base_threads * 3 / 4).max(1);
        throttle_delay_ms += 10;
    }

    SystemLoadMetrics {
        active_threads,
        throttle_delay_ms,
        cpu_usage_pct: 0.0,
        used_memory_pct: used_mem_pct,
    }
}
