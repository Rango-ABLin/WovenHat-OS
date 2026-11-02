//! Example: Using QEMU Performance Regression Detection
//!
//! Demonstrates performance baseline establishment and regression detection

/// Example: Establish performance baseline
pub fn establish_performance_baseline() {
    let mut detector = crate::qemu_perf_regression::RegressionDetector::new(10);  // 10% tolerance
    
    log_info!("perf", "Establishing baseline for Stage 10.8 test");
    
    // Measure syscall performance
    let syscall_latency = 1234_u64;  // ticks
    let _ = detector.set_baseline("syscall_latency", syscall_latency, "ticks", 0);
    
    // Measure boot time
    let boot_time = 2500_u64;  // milliseconds
    let _ = detector.set_baseline("boot_time", boot_time, "ms", 0);
    
    // Measure task switch latency
    let task_switch = 100_u64;  // ticks
    let _ = detector.set_baseline("task_switch_latency", task_switch, "ticks", 0);
    
    log_info!("perf", format_args!(
        "Baseline established: {} baselines",
        detector.baseline_count
    ));
}

/// Example: Run current test and detect regressions
pub fn run_test_with_regression_detection() {
    let mut detector = crate::qemu_perf_regression::RegressionDetector::new(10);  // 10% tolerance
    
    // Set baselines
    let _ = detector.set_baseline("syscall_latency", 1234, "ticks", 0);
    let _ = detector.set_baseline("boot_time", 2500, "ms", 0);
    let _ = detector.set_baseline("task_switch_latency", 100, "ticks", 0);
    
    log_info!("perf", "Running performance test");
    
    // Simulate current test measurements
    let current_syscall = 1456_u64;  // +17.9% regression
    let current_boot = 2750_u64;     // +10% alert
    let current_switch = 95_u64;     // -5% improvement
    
    // Check for regressions
    let mut regression_count = 0;
    
    if detector.has_regression("syscall_latency", current_syscall) {
        log_error!("perf", "REGRESSION: syscall_latency +17.9%");
        regression_count += 1;
    } else {
        log_info!("perf", "PASS: syscall_latency");
    }
    
    if detector.has_regression("boot_time", current_boot) {
        log_warn!("perf", "ALERT: boot_time +10.0%");
    } else {
        log_info!("perf", "PASS: boot_time");
    }
    
    if detector.has_regression("task_switch_latency", current_switch) {
        log_warn!("perf", "Improvement: task_switch_latency -5.0%");
    } else {
        log_info!("perf", "PASS: task_switch_latency");
    }
    
    if regression_count > 0 {
        log_error!("perf", format_args!("FAILED: {} regression(s) detected", regression_count));
    } else {
        log_info!("perf", "PASSED: No regressions detected");
    }
}

/// Example: Track performance trends over time
pub fn track_performance_trend() {
    let mut trend = crate::qemu_perf_regression::PerformanceTrend::new("syscall_latency");
    
    log_info!("perf", "Tracking syscall latency trend");
    
    // Simulate 10 test runs over time
    let samples = [
        1234, 1240, 1250, 1245, 1260,
        1280, 1300, 1310, 1320, 1330,
    ];
    
    for sample in samples.iter() {
        trend.add_sample(*sample);
    }
    
    // Analyze trend
    let avg = trend.average();
    let max = trend.maximum();
    let min = trend.minimum();
    
    log_info!("perf", format_args!(
        "Syscall latency: avg={}, min={}, max={}",
        avg, min, max
    ));
    
    if trend.is_trending_up() {
        log_warn!("perf", "WARNING: Performance trending UP (degrading)");
    } else if trend.is_trending_down() {
        log_info!("perf", "GOOD: Performance trending DOWN (improving)");
    } else {
        log_info!("perf", "Performance stable");
    }
}

/// Example: Apply regression thresholds
pub fn apply_regression_thresholds() {
    let threshold = crate::qemu_perf_regression::RegressionThreshold::new(
        "syscall_latency",
        5,   // Alert at 5%
        15,  // Fail at 15%
    );
    
    log_info!("perf", "Applying regression thresholds");
    
    let test_cases = [
        ("test_1", 2_u32),    // 2% regression - PASS
        ("test_2", 8_u32),    // 8% regression - ALERT
        ("test_3", 20_u32),   // 20% regression - FAIL
    ];
    
    for (name, regression) in test_cases.iter() {
        let status = threshold.evaluate(*regression);
        
        let status_str = match status {
            crate::qemu_perf_regression::ThresholdStatus::Pass => "PASS",
            crate::qemu_perf_regression::ThresholdStatus::Alert => "ALERT",
            crate::qemu_perf_regression::ThresholdStatus::Failed => "FAILED",
        };
        
        log_info!("perf", format_args!(
            "{}: {}% regression -> {}",
            name, regression, status_str
        ));
    }
}

/// Example: Generate performance report
pub fn generate_performance_report() {
    let mut report = crate::qemu_perf_regression::PerformanceReport::new();
    
    report.total_tests = 3;
    report.passed_tests = 1;
    report.regressed_tests = 1;
    report.improved_tests = 1;
    report.regression_percent = 34;
    report.improvement_percent = 33;
    
    log_info!("perf", "Performance Test Report");
    log_info!("perf", format_args!(
        "Total: {}, Passed: {}, Regressed: {}, Improved: {}",
        report.total_tests,
        report.passed_tests,
        report.regressed_tests,
        report.improved_tests
    ));
    
    report.print_summary();
    
    if report.regressed_tests > 0 {
        log_error!("perf", "FAILED: Performance regressions detected");
    } else {
        log_info!("perf", "PASSED: All performance checks passed");
    }
}

/// Example: Monitor for performance budget violations
pub fn monitor_performance_budget() {
    // Define performance budgets
    let syscall_budget = 2000_u64;  // 2000 ticks max
    let boot_budget = 5000_u64;     // 5000 ms max
    let switch_budget = 200_u64;    // 200 ticks max
    
    log_info!("perf", "Checking performance budgets");
    
    let syscall_current = 1456_u64;
    let boot_current = 2750_u64;
    let switch_current = 95_u64;
    
    // Check budgets
    let mut budget_violations = 0;
    
    if syscall_current > syscall_budget {
        log_error!("perf", "BUDGET VIOLATION: syscall_latency");
        budget_violations += 1;
    }
    
    if boot_current > boot_budget {
        log_error!("perf", "BUDGET VIOLATION: boot_time");
        budget_violations += 1;
    }
    
    if switch_current > switch_budget {
        log_error!("perf", "BUDGET VIOLATION: task_switch_latency");
        budget_violations += 1;
    }
    
    if budget_violations > 0 {
        log_error!("perf", format_args!(
            "FAILED: {} budget violation(s)",
            budget_violations
        ));
    } else {
        log_info!("perf", "PASSED: All budgets within limits");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline() {
        establish_performance_baseline();
    }

    #[test]
    fn test_regression_detection() {
        run_test_with_regression_detection();
    }

    #[test]
    fn test_trending() {
        track_performance_trend();
    }

    #[test]
    fn test_thresholds() {
        apply_regression_thresholds();
    }

    #[test]
    fn test_report() {
        generate_performance_report();
    }

    #[test]
    fn test_budget() {
        monitor_performance_budget();
    }
}
