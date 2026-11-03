//! Example: Using Bounded Capacity Analyzer

pub struct CompletionPortSubsystem {
    analyzer: crate::bounded_capacity_analyzer::CapacityAnalyzer,
}

impl CompletionPortSubsystem {
    pub const fn new() -> Self {
        CompletionPortSubsystem {
            analyzer: crate::bounded_capacity_analyzer::CapacityAnalyzer::new(),
        }
    }

    pub fn create_port(&mut self) -> Result<u64, i32> {
        let idx = self.analyzer.register(32).map_err(|_| -28)?;
        Ok(idx as u64)
    }

    pub fn associate(&mut self, port_idx: usize, op: u64) -> Result<(), i32> {
        if let Some(m) = self.analyzer.metadata(port_idx) {
            if !m.can_allocate(1) {
                return Err(-28);
            }
        }
        self.analyzer.allocate(port_idx, 1).map_err(|_| -28)
    }

    pub fn disassociate(&mut self, port_idx: usize, _op: u64) {
        self.analyzer.deallocate(port_idx, 1);
    }

    pub fn health_check(&self) -> bool {
        let r = crate::bounded_capacity_analyzer::CapacityReport::from_analyzer(&self.analyzer);
        r.allocation_failures == 0
    }
}

pub struct AsyncOpSubsystem {
    analyzer: crate::bounded_capacity_analyzer::CapacityAnalyzer,
}

impl AsyncOpSubsystem {
    pub const fn new() -> Self {
        AsyncOpSubsystem {
            analyzer: crate::bounded_capacity_analyzer::CapacityAnalyzer::new(),
        }
    }

    pub fn init(&mut self) -> Result<(), i32> {
        self.analyzer.register(256).map_err(|_| -28)?;
        Ok(())
    }

    pub fn submit(&mut self, _op: u64) -> Result<(), i32> {
        self.analyzer.allocate(0, 1).map_err(|_| -28)
    }

    pub fn complete(&mut self, _op: u64) {
        self.analyzer.deallocate(0, 1);
    }
}
