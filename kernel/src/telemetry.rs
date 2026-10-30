// COM1 Serial Telemetry and Profiler Streaming Subsystem for WovenHat-OS
extern crate alloc;
use alloc::format;
use x86_64::instructions::port::Port;
use crate::profiler::KernelProfiler;

pub struct SerialTelemetry {
    data_port: Port<u8>,
}

impl SerialTelemetry {
    pub const fn new() -> Self {
        Self {
            data_port: Port::new(0x3F8),
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        unsafe {
            self.data_port.write(byte);
        }
    }

    pub fn log_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    pub fn stream_profiler_stats(&mut self, profiler: &KernelProfiler) {
        let switches = profiler.total_context_switches.load(core::sync::atomic::Ordering::Relaxed);
        let avg_latency = profiler.get_average_switch_latency_cycles();
        let util = profiler.calculate_cpu_utilization_permille();

        let report = format!(
            "[TELEMETRY] Context Switches: {} | Avg Latency: {} cycles | CPU Util: {}.{}%\n",
            switches,
            avg_latency,
            util / 10,
            util % 10
        );

        self.log_string(&report);
    }
}
