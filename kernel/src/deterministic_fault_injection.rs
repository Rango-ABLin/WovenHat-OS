//! Deterministic Fault Injection Framework

#![allow(dead_code, unused)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    AllocationFailure,
    TimeoutExpired,
    DataCorruption,
    CpuNotReady,
    LockContentionHigh,
    MemoryPressure,
    InterruptDropped,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy)]
pub struct FaultPoint {
    pub id: u32,
    pub kind: FaultKind,
    pub trigger_count: u32,
    pub enabled: bool,
}

pub struct DeterministicFaultInjector {
    faults: [FaultPoint; 32],
    fault_count: usize,
    seed: u64,
    injection_count: u32,
    rng_state: u64,
}

impl DeterministicFaultInjector {
    pub const fn new(seed: u64) -> Self {
        DeterministicFaultInjector {
            faults: [FaultPoint {
                id: 0,
                kind: FaultKind::AllocationFailure,
                trigger_count: 0,
                enabled: false,
            }; 32],
            fault_count: 0,
            seed,
            injection_count: 0,
            rng_state: seed,
        }
    }

    pub fn register_fault_point(&mut self, kind: FaultKind) -> Result<u32, ()> {
        if self.fault_count >= 32 {
            return Err(());
        }
        let id = self.fault_count as u32;
        self.faults[self.fault_count] = FaultPoint {
            id,
            kind,
            trigger_count: 0,
            enabled: false,
        };
        self.fault_count += 1;
        Ok(id)
    }

    pub fn enable_fault(&mut self, id: u32, trigger_count: u32) -> Result<(), ()> {
        if (id as usize) >= self.fault_count {
            return Err(());
        }
        self.faults[id as usize].enabled = true;
        self.faults[id as usize].trigger_count = trigger_count;
        Ok(())
    }

    pub fn disable_fault(&mut self, id: u32) {
        if (id as usize) < self.fault_count {
            self.faults[id as usize].enabled = false;
        }
    }

    pub fn next_random(&mut self) -> u32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng_state >> 33) as u32
    }

    pub fn should_inject(&mut self, id: u32) -> bool {
        if (id as usize) >= self.fault_count {
            return false;
        }

        let fault = &mut self.faults[id as usize];
        if !fault.enabled {
            return false;
        }

        if fault.trigger_count == 0 {
            return false;
        }

        fault.trigger_count -= 1;
        if fault.trigger_count == 0 {
            self.injection_count += 1;
            return true;
        }

        false
    }

    pub fn total_injections(&self) -> u32 {
        self.injection_count
    }
}

pub struct FaultInjectionStats {
    pub total_faults_registered: usize,
    pub total_faults_triggered: u32,
    pub faults_by_kind: [u32; 8],
}

impl FaultInjectionStats {
    pub fn from_injector(injector: &DeterministicFaultInjector) -> Self {
        let mut stats = FaultInjectionStats {
            total_faults_registered: injector.fault_count,
            total_faults_triggered: injector.injection_count,
            faults_by_kind: [0; 8],
        };

        for fault in injector.faults[..injector.fault_count].iter() {
            if !fault.enabled {
                continue;
            }
            match fault.kind {
                FaultKind::AllocationFailure => stats.faults_by_kind[0] += 1,
                FaultKind::TimeoutExpired => stats.faults_by_kind[1] += 1,
                FaultKind::DataCorruption => stats.faults_by_kind[2] += 1,
                FaultKind::CpuNotReady => stats.faults_by_kind[3] += 1,
                FaultKind::LockContentionHigh => stats.faults_by_kind[4] += 1,
                FaultKind::MemoryPressure => stats.faults_by_kind[5] += 1,
                FaultKind::InterruptDropped => stats.faults_by_kind[6] += 1,
                FaultKind::DeadlineExceeded => stats.faults_by_kind[7] += 1,
            }
        }

        stats
    }

    pub fn print_summary(&self) {
        crate::serial::write_line(format_args!(
            "[FI] Registered: {}, Triggered: {}",
            self.total_faults_registered, self.total_faults_triggered
        ));
    }
}

pub struct FaultInjectionTestCase {
    pub name: &'static str,
    pub seed: u64,
    pub faults: [(u32, u32); 8],
    pub fault_count: usize,
}

impl FaultInjectionTestCase {
    pub const fn new(name: &'static str, seed: u64) -> Self {
        FaultInjectionTestCase {
            name,
            seed,
            faults: [(0, 0); 8],
            fault_count: 0,
        }
    }

    pub fn add_fault(&mut self, fault_id: u32, trigger_at: u32) -> Result<(), ()> {
        if self.fault_count >= 8 {
            return Err(());
        }
        self.faults[self.fault_count] = (fault_id, trigger_at);
        self.fault_count += 1;
        Ok(())
    }

    pub fn run(&self, injector: &mut DeterministicFaultInjector) -> Result<(), &'static str> {
        for (fault_id, trigger_at) in self.faults[..self.fault_count].iter() {
            injector
                .enable_fault(*fault_id, *trigger_at)
                .map_err(|_| "Failed to enable fault")?;
        }
        Ok(())
    }
}

pub struct FaultCoverage {
    pub faults_triggered: u32,
    pub faults_recovered: u32,
    pub faults_fatal: u32,
    pub recovery_rate: u32,
}

impl FaultCoverage {
    pub const fn new() -> Self {
        FaultCoverage {
            faults_triggered: 0,
            faults_recovered: 0,
            faults_fatal: 0,
            recovery_rate: 0,
        }
    }

    pub fn record_recovery(&mut self) {
        self.faults_recovered += 1;
        if self.faults_triggered > 0 {
            self.recovery_rate =
                (self.faults_recovered as u64 * 100 / self.faults_triggered as u64) as u32;
        }
    }

    pub fn record_failure(&mut self) {
        self.faults_fatal += 1;
    }

    pub fn print_report(&self) {
        crate::serial::write_line(format_args!(
            "[FI-COVERAGE] Triggered: {}, Recovered: {}, Fatal: {}, Rate: {}%",
            self.faults_triggered, self.faults_recovered, self.faults_fatal, self.recovery_rate
        ));
    }
}

pub struct FaultInjectionScenario {
    pub scenario_id: u32,
    pub target_subsystem: &'static str,
    pub fault_kind: FaultKind,
    pub timing: u32,
}

impl FaultInjectionScenario {
    pub const fn new(id: u32, subsys: &'static str, kind: FaultKind, timing: u32) -> Self {
        FaultInjectionScenario {
            scenario_id: id,
            target_subsystem: subsys,
            fault_kind: kind,
            timing,
        }
    }

    pub fn describe(&self) {
        crate::serial::write_line(format_args!(
            "[FI-SCENARIO] {} on {} at {} ticks",
            self.scenario_id, self.target_subsystem, self.timing
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_injector_creation() {
        let _inj = DeterministicFaultInjector::new(12345);
    }

    #[test]
    fn test_register_fault() {
        let mut inj = DeterministicFaultInjector::new(12345);
        let id = inj
            .register_fault_point(FaultKind::AllocationFailure)
            .unwrap();
        assert_eq!(id, 0);
    }

    #[test]
    fn test_enable_fault() {
        let mut inj = DeterministicFaultInjector::new(12345);
        let id = inj.register_fault_point(FaultKind::TimeoutExpired).unwrap();
        assert!(inj.enable_fault(id, 5).is_ok());
    }

    #[test]
    fn test_injection() {
        let mut inj = DeterministicFaultInjector::new(12345);
        let id = inj
            .register_fault_point(FaultKind::AllocationFailure)
            .unwrap();
        inj.enable_fault(id, 1).unwrap();

        assert!(inj.should_inject(id));
        assert_eq!(inj.total_injections(), 1);
    }

    #[test]
    fn test_coverage() {
        let mut cov = FaultCoverage::new();
        cov.faults_triggered = 10;
        cov.record_recovery();
        assert_eq!(cov.faults_recovered, 1);
    }
}
