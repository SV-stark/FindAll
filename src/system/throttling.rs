use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Real-time system load metrics used for adaptive background task throttling.
#[derive(Debug, Clone, Copy)]
pub struct SystemLoadMetrics {
    pub active_threads: u8,
    pub throttle_delay_ms: u64,
    pub cpu_usage_pct: f32,
    pub used_memory_pct: f32,
}

/// Computes adaptive thread allocations and delay intervals using real-time `sysinfo` data.
pub fn get_adaptive_system_load(base_threads: u8) -> SystemLoadMetrics {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    sys.refresh_cpu_all();
    sys.refresh_memory();

    let global_cpu = sys.global_cpu_usage();
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let used_mem_pct = if total_mem > 0 {
        (used_mem as f32 / total_mem as f32) * 100.0
    } else {
        0.0
    };

    let mut active_threads = base_threads;
    let mut throttle_delay_ms = 0;

    // High CPU (>85%): scale down threads and introduce a small throttle delay
    if global_cpu > 85.0 {
        active_threads = (base_threads / 2).max(1);
        throttle_delay_ms = 50;
    } else if global_cpu > 70.0 {
        active_threads = (base_threads * 3 / 4).max(1);
        throttle_delay_ms = 10;
    }

    // High RAM usage (>85%): scale down threads to relieve memory pressure
    if used_mem_pct > 85.0 {
        active_threads = (active_threads / 2).max(1);
        throttle_delay_ms += 30;
    }

    SystemLoadMetrics {
        active_threads,
        throttle_delay_ms,
        cpu_usage_pct: global_cpu,
        used_memory_pct: used_mem_pct,
    }
}
