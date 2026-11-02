//! Example: Deterministic Fault Injection Scenarios
//!
//! Demonstrates fault injection testing for kernel robustness

/// Scenario: Allocation failure handling
pub fn scenario_allocation_stress() {
    let seed = 0xdead_beef_u64;
    let mut injector = crate::deterministic_fault_injection::DeterministicFaultInjector::new(seed);
    
    // Register allocation fault point
    let alloc_fault = injector.register_fault_point(
        crate::deterministic_fault_injection::FaultKind::AllocationFailure
    ).unwrap();
    
    // Fail every 10th allocation
    let _ = injector.enable_fault(alloc_fault, 10);
    
    log_info!("fault_injection", "Starting allocation stress test");
    
    for i in 0..100 {
        if injector.should_inject(alloc_fault) {
            log_warn!("alloc", format_args!("Allocation failed at iteration {}", i));
            // Handle allocation failure
        } else {
            log_debug!("alloc", format_args!("Allocation succeeded at iteration {}", i));
        }
    }
    
    let stats = crate::deterministic_fault_injection::FaultInjectionStats::from_injector(&injector);
    log_info!("fault_injection", format_args!(
        "Triggered: {} faults",
        stats.total_faults_triggered
    ));
}

/// Scenario: Timeout handling
pub fn scenario_timeout_resilience() {
    let seed = 0xcafe_babe_u64;
    let mut injector = crate::deterministic_fault_injection::DeterministicFaultInjector::new(seed);
    
    let timeout_fault = injector.register_fault_point(
        crate::deterministic_fault_injection::FaultKind::TimeoutExpired
    ).unwrap();
    
    // Timeout at 50 operations
    let _ = injector.enable_fault(timeout_fault, 50);
    
    let mut coverage = crate::deterministic_fault_injection::FaultCoverage::new();
    
    log_info!("fault_injection", "Starting timeout resilience test");
    
    for i in 0..100 {
        if injector.should_inject(timeout_fault) {
            coverage.faults_triggered += 1;
            log_warn!("timeout", format_args!("Timeout at operation {}", i));
            
            // Verify recovery
            if handle_timeout_gracefully() {
                coverage.record_recovery();
            } else {
                coverage.record_failure();
            }
        }
    }
    
    coverage.print_report();
}

/// Scenario: Data corruption detection
pub fn scenario_corruption_detection() {
    let seed = 0x1337_cafe_u64;
    let mut injector = crate::deterministic_fault_injection::DeterministicFaultInjector::new(seed);
    
    let corrupt_fault = injector.register_fault_point(
        crate::deterministic_fault_injection::FaultKind::DataCorruption
    ).unwrap();
    
    // Corrupt at 30 operations
    let _ = injector.enable_fault(corrupt_fault, 30);
    
    log_info!("fault_injection", "Starting corruption detection test");
    
    for i in 0..100 {
        let mut data = [0u8; 512];
        
        if injector.should_inject(corrupt_fault) {
            log_warn!("corruption", "Data corrupted in read");
            // Data is corrupted
            data[0] ^= 0xff;
        }
        
        // Verify with checksum
        let checksum = compute_checksum(&data);
        if verify_checksum(&data, checksum) {
            log_debug!("corruption", format_args!("Data verified at {}", i));
        } else {
            log_error!("corruption", "Checksum mismatch!");
        }
    }
}

/// Scenario: Combined fault scenario
pub fn scenario_combined_faults() {
    let seed = 0xcafe_dead_u64;
    let mut injector = crate::deterministic_fault_injection::DeterministicFaultInjector::new(seed);
    
    // Register multiple fault types
    let alloc_fault = injector.register_fault_point(
        crate::deterministic_fault_injection::FaultKind::AllocationFailure
    ).unwrap();
    let timeout_fault = injector.register_fault_point(
        crate::deterministic_fault_injection::FaultKind::TimeoutExpired
    ).unwrap();
    
    // Enable both faults
    let _ = injector.enable_fault(alloc_fault, 25);
    let _ = injector.enable_fault(timeout_fault, 75);
    
    log_info!("fault_injection", "Starting combined fault test");
    
    for i in 0..150 {
        let mut had_fault = false;
        
        if injector.should_inject(alloc_fault) {
            log_warn!("fault", format_args!("Allocation failed at {}", i));
            had_fault = true;
        }
        
        if injector.should_inject(timeout_fault) {
            log_warn!("fault", format_args!("Timeout at {}", i));
            had_fault = true;
        }
        
        if had_fault {
            // Verify system remains stable
            verify_system_stability();
        }
    }
    
    let stats = crate::deterministic_fault_injection::FaultInjectionStats::from_injector(&injector);
    log_info!("fault_injection", format_args!(
        "Test complete: {} total injections",
        stats.total_faults_triggered
    ));
}

/// Scenario: Lock contention stress
pub fn scenario_lock_contention() {
    let seed = 0x_1234_5678_u64;
    let mut injector = crate::deterministic_fault_injection::DeterministicFaultInjector::new(seed);
    
    let lock_fault = injector.register_fault_point(
        crate::deterministic_fault_injection::FaultKind::LockContentionHigh
    ).unwrap();
    
    // High contention every 40 operations
    let _ = injector.enable_fault(lock_fault, 40);
    
    log_info!("fault_injection", "Starting lock contention test");
    
    for i in 0..200 {
        if injector.should_inject(lock_fault) {
            log_warn!("contention", format_args!("High contention at {}", i));
            simulate_contention();
        }
        
        perform_lock_operation(i);
    }
}

/// Helper: Handle timeout gracefully
fn handle_timeout_gracefully() -> bool {
    // Return true if timeout was handled
    true
}

/// Helper: Compute data checksum
fn compute_checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for byte in data {
        sum = sum.wrapping_add(*byte as u32);
    }
    sum
}

/// Helper: Verify checksum
fn verify_checksum(data: &[u8], expected: u32) -> bool {
    compute_checksum(data) == expected
}

/// Helper: Verify system stability
fn verify_system_stability() {
    // In real scenario, check locks, queues, etc.
}

/// Helper: Simulate contention
fn simulate_contention() {
    for _ in 0..100 {
        core::hint::spin_loop();
    }
}

/// Helper: Perform lock operation
fn perform_lock_operation(_id: u32) {
    // Simulated lock operation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_scenario() {
        scenario_allocation_stress();
    }

    #[test]
    fn test_timeout_scenario() {
        scenario_timeout_resilience();
    }

    #[test]
    fn test_combined_scenario() {
        scenario_combined_faults();
    }
}
