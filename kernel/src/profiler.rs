// CPU Utilization and Context Switch Latency Profiler for WovenHat-OS
extern crate alloc;
use core::sync::atomic::{AtomicU64, Ordering};
use core::arch::x86_64::_rdtsc;

pub struct KernelProfiler {
    pub total_context_switches: AtomicU64,
    pub cumulative_switch_latency_cycles: AtomicU64,
    pub idle_cycles: AtomicU64,
    pub active_cycles: AtomicU64,
}

impl KernelProfiler {
    pub const fn new() -> Self {
        Self {
            total_context_switches: AtomicU64::new(0),
            cumulative_switch_latency_cycles: AtomicU64::new(0),
            idle_cycles: AtomicU64::new(0),
            active_cycles: AtomicU64::new(0),
        }
    }

    pub fn record_context_switch(&self, start_tsc: u64) {
        let end_tsc = unsafe { _rdtsc() };
        let latency = end_tsc.saturating_sub(start_tsc);
        self.total_context_switches.fetch_add(1, Ordering::Relaxed);
        self.cumulative_switch_latency_cycles.fetch_add(latency, Ordering::Relaxed);
    }

    pub fn get_average_switch_latency_cycles(&self) -> u64 {
        let switches = self.total_context_switches.load(Ordering::Relaxed);
        if switches == 0 {
            return 0;
        }
        self.cumulative_switch_latency_cycles.load(Ordering::Relaxed) / switches
    }

    pub fn calculate_cpu_utilization_permille(&self) -> u32 {
        let active = self.active_cycles.load(Ordering::Relaxed);
        let idle = self.idle_cycles.load(Ordering::Relaxed);
        let total = active + idle;
        if total == 0 {
            return 0;
        }
        ((active * 1000) / total) as u32
    }
}
