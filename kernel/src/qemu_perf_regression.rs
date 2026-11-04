//! QEMU Performance Regression Detection

#![allow(dead_code, unused)]

#[derive(Debug, Clone, Copy)]
pub struct PerformanceMetric {
    pub name: &'static str,
    pub value: u64,
    pub unit: &'static str,
}

impl PerformanceMetric {
    pub const fn new(name: &'static str, value: u64, unit: &'static str) -> Self {
        PerformanceMetric { name, value, unit }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PerformanceBenchmark {
    pub test_name: &'static str,
    pub metrics: [PerformanceMetric; 8],
    pub metric_count: usize,
    pub timestamp: u64,
}

impl PerformanceBenchmark {
    pub const fn new(test_name: &'static str, timestamp: u64) -> Self {
        PerformanceBenchmark {
            test_name,
            metrics: [PerformanceMetric::new("", 0, ""); 8],
            metric_count: 0,
            timestamp,
        }
    }

    pub fn add_metric(&mut self, metric: PerformanceMetric) -> Result<(), ()> {
        if self.metric_count >= 8 {
            return Err(());
        }
        self.metrics[self.metric_count] = metric;
        self.metric_count += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PerformanceBaseline {
    pub test_name: &'static str,
    pub baseline_value: u64,
    pub unit: &'static str,
    pub created_at: u64,
}

impl PerformanceBaseline {
    pub const fn new(test_name: &'static str, value: u64, unit: &'static str, ts: u64) -> Self {
        PerformanceBaseline {
            test_name,
            baseline_value: value,
            unit,
            created_at: ts,
        }
    }

    pub fn check_regression(&self, current_value: u64, tolerance_percent: u32) -> bool {
        let max_allowed = self.baseline_value + (self.baseline_value * tolerance_percent as u64 / 100);
        current_value > max_allowed
    }

    pub fn regression_percent(&self, current_value: u64) -> u32 {
        if self.baseline_value == 0 {
            return 0;
        }
        if current_value <= self.baseline_value {
            return 0;
        }
        let diff = current_value - self.baseline_value;
        ((diff * 100) / self.baseline_value) as u32
    }
}

pub struct RegressionDetector {
    baselines: [PerformanceBaseline; 32],
    baseline_count: usize,
    tolerance_percent: u32,
}

impl RegressionDetector {
    pub const fn new(tolerance_percent: u32) -> Self {
        RegressionDetector {
            baselines: [PerformanceBaseline::new("", 0, "", 0); 32],
            baseline_count: 0,
            tolerance_percent,
        }
    }

    pub fn set_baseline(
        &mut self,
        test_name: &'static str,
        value: u64,
        unit: &'static str,
        timestamp: u64,
    ) -> Result<(), ()> {
        if self.baseline_count >= 32 {
            return Err(());
        }
        self.baselines[self.baseline_count] =
            PerformanceBaseline::new(test_name, value, unit, timestamp);
        self.baseline_count += 1;
        Ok(())
    }

    pub fn check_regression(&self, test_name: &str, current_value: u64) -> Option<u32> {
        for baseline in self.baselines[..self.baseline_count].iter() {
            if baseline.test_name == test_name {
                if baseline.check_regression(current_value, self.tolerance_percent) {
                    return Some(baseline.regression_percent(current_value));
                } else {
                    return Some(0); // No regression
                }
            }
        }
        None // No baseline found
    }

    pub fn has_regression(&self, test_name: &str, current_value: u64) -> bool {
        for baseline in self.baselines[..self.baseline_count].iter() {
            if baseline.test_name == test_name {
                return baseline.check_regression(current_value, self.tolerance_percent);
            }
        }
        false
    }
}

pub struct PerformanceReport {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub regressed_tests: usize,
    pub improved_tests: usize,
    pub improvement_percent: u32,
    pub regression_percent: u32,
}

impl PerformanceReport {
    pub const fn new() -> Self {
        PerformanceReport {
            total_tests: 0,
            passed_tests: 0,
            regressed_tests: 0,
            improved_tests: 0,
            improvement_percent: 0,
            regression_percent: 0,
        }
    }

    pub fn print_summary(&self) {
        crate::serial::write_line(format_args!(
            "[PERF] Tests: {}, Passed: {}, Regressed: {}, Improved: {}",
            self.total_tests, self.passed_tests, self.regressed_tests, self.improved_tests
        ));
        crate::serial::write_line(format_args!(
            "[PERF] Overall: {} improvement, {} regression",
            self.improvement_percent, self.regression_percent
        ));
    }
}

pub struct PerformanceTrend {
    pub test_name: &'static str,
    pub samples: [u64; 16],
    pub sample_count: usize,
}

impl PerformanceTrend {
    pub const fn new(test_name: &'static str) -> Self {
        PerformanceTrend {
            test_name,
            samples: [0; 16],
            sample_count: 0,
        }
    }

    pub fn add_sample(&mut self, value: u64) {
        if self.sample_count < 16 {
            self.samples[self.sample_count] = value;
            self.sample_count += 1;
        } else {
            // Ring buffer: shift and add
            for i in 0..15 {
                self.samples[i] = self.samples[i + 1];
            }
            self.samples[15] = value;
        }
    }

    pub fn average(&self) -> u64 {
        if self.sample_count == 0 {
            return 0;
        }
        let sum: u64 = self.samples[..self.sample_count].iter().sum();
        sum / self.sample_count as u64
    }

    pub fn maximum(&self) -> u64 {
        self.samples[..self.sample_count]
            .iter()
            .max()
            .copied()
            .unwrap_or(0)
    }

    pub fn minimum(&self) -> u64 {
        self.samples[..self.sample_count]
            .iter()
            .min()
            .copied()
            .unwrap_or(0)
    }

    pub fn is_trending_up(&self) -> bool {
        if self.sample_count < 3 {
            return false;
        }

        let recent = &self.samples[self.sample_count - 3..self.sample_count];
        recent[0] < recent[1] && recent[1] < recent[2]
    }

    pub fn is_trending_down(&self) -> bool {
        if self.sample_count < 3 {
            return false;
        }

        let recent = &self.samples[self.sample_count - 3..self.sample_count];
        recent[0] > recent[1] && recent[1] > recent[2]
    }
}

pub struct RegressionThreshold {
    pub metric_name: &'static str,
    pub alert_threshold: u32,
    pub fail_threshold: u32,
}

impl RegressionThreshold {
    pub const fn new(
        name: &'static str,
        alert: u32,
        fail: u32,
    ) -> Self {
        RegressionThreshold {
            metric_name: name,
            alert_threshold: alert,
            fail_threshold: fail,
        }
    }

    pub fn evaluate(&self, regression_percent: u32) -> ThresholdStatus {
        if regression_percent >= self.fail_threshold {
            ThresholdStatus::Failed
        } else if regression_percent >= self.alert_threshold {
            ThresholdStatus::Alert
        } else {
            ThresholdStatus::Pass
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdStatus {
    Pass,
    Alert,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_creation() {
        let base = PerformanceBaseline::new("test", 1000, "ms", 0);
        assert_eq!(base.baseline_value, 1000);
    }

    #[test]
    fn test_regression_detection() {
        let base = PerformanceBaseline::new("test", 1000, "ms", 0);
        assert!(!base.check_regression(1050, 10)); // Within tolerance
        assert!(base.check_regression(1150, 10));  // Over tolerance
    }

    #[test]
    fn test_regression_percent() {
        let base = PerformanceBaseline::new("test", 1000, "ms", 0);
        assert_eq!(base.regression_percent(1100), 10);
        assert_eq!(base.regression_percent(1500), 50);
    }

    #[test]
    fn test_performance_trend() {
        let mut trend = PerformanceTrend::new("test");
        trend.add_sample(100);
        trend.add_sample(110);
        trend.add_sample(120);
        
        assert_eq!(trend.average(), 110);
        assert_eq!(trend.maximum(), 120);
        assert_eq!(trend.minimum(), 100);
        assert!(trend.is_trending_up());
    }

    #[test]
    fn test_threshold() {
        let thresh = RegressionThreshold::new("latency", 5, 15);
        
        assert_eq!(thresh.evaluate(2), ThresholdStatus::Pass);
        assert_eq!(thresh.evaluate(10), ThresholdStatus::Alert);
        assert_eq!(thresh.evaluate(20), ThresholdStatus::Failed);
    }
}
