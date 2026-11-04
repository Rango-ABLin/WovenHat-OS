//! Multi-Agent Coordination Dashboard

#![allow(dead_code, unused)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Working,
    Waiting,
    Blocked,
    Complete,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct AgentState {
    pub agent_id: u32,
    pub status: AgentStatus,
    pub current_task: u32,
    pub tasks_completed: u32,
    pub last_heartbeat: u64,
}

impl AgentState {
    pub const fn new(id: u32) -> Self {
        AgentState {
            agent_id: id,
            status: AgentStatus::Idle,
            current_task: 0,
            tasks_completed: 0,
            last_heartbeat: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy)]
pub struct Task {
    pub task_id: u32,
    pub assigned_to: u32,
    pub status: TaskStatus,
    pub priority: u32,
    pub dependencies: [u32; 4],
    pub dep_count: usize,
}

impl Task {
    pub const fn new(id: u32, priority: u32) -> Self {
        Task {
            task_id: id,
            assigned_to: 0,
            status: TaskStatus::Queued,
            priority,
            dependencies: [0; 4],
            dep_count: 0,
        }
    }

    pub fn add_dependency(&mut self, task_id: u32) -> Result<(), ()> {
        if self.dep_count >= 4 {
            return Err(());
        }
        self.dependencies[self.dep_count] = task_id;
        self.dep_count += 1;
        Ok(())
    }
}

pub struct CoordinationDashboard {
    pub agents: [AgentState; 8],
    pub agent_count: usize,
    pub tasks: [Task; 32],
    pub task_count: usize,
}

impl CoordinationDashboard {
    pub const fn new() -> Self {
        CoordinationDashboard {
            agents: [AgentState::new(0); 8],
            agent_count: 0,
            tasks: [Task::new(0, 0); 32],
            task_count: 0,
        }
    }

    pub fn register_agent(&mut self, agent_id: u32) -> Result<(), ()> {
        if self.agent_count >= 8 {
            return Err(());
        }
        self.agents[self.agent_count] = AgentState::new(agent_id);
        self.agent_count += 1;
        Ok(())
    }

    pub fn add_task(&mut self, task: Task) -> Result<(), ()> {
        if self.task_count >= 32 {
            return Err(());
        }
        self.tasks[self.task_count] = task;
        self.task_count += 1;
        Ok(())
    }

    pub fn assign_task(&mut self, task_id: u32, agent_id: u32) -> Result<(), ()> {
        for task in self.tasks[..self.task_count].iter_mut() {
            if task.task_id == task_id {
                task.assigned_to = agent_id;
                task.status = TaskStatus::Assigned;
                return Ok(());
            }
        }
        Err(())
    }

    pub fn update_agent_status(&mut self, agent_id: u32, status: AgentStatus) -> Result<(), ()> {
        for agent in self.agents[..self.agent_count].iter_mut() {
            if agent.agent_id == agent_id {
                agent.status = status;
                return Ok(());
            }
        }
        Err(())
    }

    pub fn count_working_agents(&self) -> usize {
        self.agents[..self.agent_count]
            .iter()
            .filter(|a| a.status == AgentStatus::Working)
            .count()
    }

    pub fn count_idle_agents(&self) -> usize {
        self.agents[..self.agent_count]
            .iter()
            .filter(|a| a.status == AgentStatus::Idle)
            .count()
    }

    pub fn count_blocked_agents(&self) -> usize {
        self.agents[..self.agent_count]
            .iter()
            .filter(|a| a.status == AgentStatus::Blocked)
            .count()
    }

    pub fn total_tasks_completed(&self) -> u32 {
        self.agents[..self.agent_count]
            .iter()
            .map(|a| a.tasks_completed)
            .sum()
    }
}

pub struct CollisionDetector {
    pub active_files: [&'static str; 16],
    pub file_count: usize,
}

impl CollisionDetector {
    pub const fn new() -> Self {
        CollisionDetector {
            active_files: [""; 16],
            file_count: 0,
        }
    }

    pub fn try_acquire(&mut self, filename: &'static str) -> Result<(), ()> {
        for active in self.active_files[..self.file_count].iter() {
            if *active == filename {
                return Err(()); // Collision: file already in use
            }
        }
        if self.file_count >= 16 {
            return Err(()); // No capacity
        }
        self.active_files[self.file_count] = filename;
        self.file_count += 1;
        Ok(())
    }

    pub fn release(&mut self, filename: &str) {
        if let Some(pos) = self.active_files[..self.file_count]
            .iter()
            .position(|f| *f == filename)
        {
            self.active_files[pos] = "";
            self.file_count -= 1;
        }
    }

    pub fn is_in_use(&self, filename: &str) -> bool {
        self.active_files[..self.file_count].contains(&filename)
    }
}

pub struct DashboardReport {
    pub total_agents: usize,
    pub working_agents: usize,
    pub idle_agents: usize,
    pub blocked_agents: usize,
    pub total_tasks: usize,
    pub tasks_completed: u32,
    pub tasks_in_progress: usize,
}

impl DashboardReport {
    pub fn print_summary(&self) {
        crate::serial::write_line(format_args!(
            "[COORD] Agents: {} (working: {}, idle: {}, blocked: {})",
            self.total_agents, self.working_agents, self.idle_agents, self.blocked_agents
        ));
        crate::serial::write_line(format_args!(
            "[COORD] Tasks: {} total, {} completed, {} in progress",
            self.total_tasks, self.tasks_completed, self.tasks_in_progress
        ));
    }
}

pub struct WorkloadBalancer {
    pub agent_loads: [u32; 8],
    pub agent_count: usize,
}

impl WorkloadBalancer {
    pub const fn new(count: usize) -> Self {
        WorkloadBalancer {
            agent_loads: [0; 8],
            agent_count: count,
        }
    }

    pub fn assign_task_to_least_busy(&self) -> u32 {
        let mut min_load = u32::MAX;
        let mut best_agent = 0;

        for i in 0..self.agent_count {
            if self.agent_loads[i] < min_load {
                min_load = self.agent_loads[i];
                best_agent = i as u32;
            }
        }
        best_agent
    }

    pub fn record_task_assignment(&mut self, agent_id: u32) {
        if (agent_id as usize) < self.agent_count {
            self.agent_loads[agent_id as usize] += 1;
        }
    }

    pub fn average_load(&self) -> u32 {
        let total: u32 = self.agent_loads[..self.agent_count].iter().sum();
        if self.agent_count == 0 {
            0
        } else {
            total / self.agent_count as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard() {
        let mut dash = CoordinationDashboard::new();
        let _ = dash.register_agent(1);
        assert_eq!(dash.agent_count, 1);
    }

    #[test]
    fn test_task_assignment() {
        let mut dash = CoordinationDashboard::new();
        let _ = dash.register_agent(1);
        let task = Task::new(1, 5);
        let _ = dash.add_task(task);
        assert!(dash.assign_task(1, 1).is_ok());
    }

    #[test]
    fn test_collision_detection() {
        let mut detector = CollisionDetector::new();
        assert!(detector.try_acquire("file1.rs").is_ok());
        assert!(detector.try_acquire("file1.rs").is_err()); // Collision!
    }

    #[test]
    fn test_workload_balance() {
        let mut balancer = WorkloadBalancer::new(3);
        let agent = balancer.assign_task_to_least_busy();
        balancer.record_task_assignment(agent);
        assert_eq!(balancer.average_load(), 0);
    }
}

pub struct CriticalFileLockRegistry;

impl CriticalFileLockRegistry {
    pub const CRITICAL_FILES: &'static [&'static str] = &[
        "kernel/src/memory_safety_audit.rs",
        "kernel/src/telemetry_hooks.rs",
        "kernel/src/structured_logging.rs",
        "kernel/src/bounded_capacity_analyzer.rs",
        "kernel/src/deterministic_fault_injection.rs",
        "kernel/src/compiler_fuzzing.rs",
        "kernel/src/qemu_perf_regression.rs",
        "kernel/src/agent_coordination.rs",
        "docs/agent-coordination.md",
        "docs/stage-status.md",
        "Cargo.toml",
        "kernel/Cargo.toml",
    ];
}

impl CoordinationDashboard {
    pub fn all_dependencies_complete(&self, task: &Task) -> bool {
        for i in 0..task.dep_count {
            let dep_id = task.dependencies[i];
            if let Some(dep_task) = self.tasks[..self.task_count]
                .iter()
                .find(|t| t.task_id == dep_id)
            {
                if dep_task.status != TaskStatus::Completed {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn can_start_task(&self, task: &Task) -> bool {
        task.status == TaskStatus::Assigned && self.all_dependencies_complete(task)
    }

    pub fn mark_task_in_progress(&mut self, task_id: u32) -> Result<(), ()> {
        // First check if task exists and dependencies are met
        let task_ready = {
            self.tasks[..self.task_count]
                .iter()
                .find(|t| t.task_id == task_id)
                .map(|t| self.can_start_task(t))
                .unwrap_or(false)
        };

        // Then update task status if ready
        if task_ready {
            for task in self.tasks[..self.task_count].iter_mut() {
                if task.task_id == task_id {
                    task.status = TaskStatus::InProgress;
                    return Ok(());
                }
            }
        }
        Err(())
    }

    pub fn mark_task_completed(&mut self, task_id: u32) -> Result<(), ()> {
        for task in self.tasks[..self.task_count].iter_mut() {
            if task.task_id == task_id {
                task.status = TaskStatus::Completed;
                if let Ok(agent) = self.agents[..self.agent_count]
                    .iter_mut()
                    .find(|a| a.agent_id == task.assigned_to)
                    .ok_or(())
                {
                    agent.tasks_completed += 1;
                }
                return Ok(());
            }
        }
        Err(())
    }

    pub fn find_next_ready_task(&self) -> Option<u32> {
        self.tasks[..self.task_count]
            .iter()
            .filter(|t| self.can_start_task(t))
            .min_by_key(|t| core::cmp::Reverse(t.priority))
            .map(|t| t.task_id)
    }

    pub fn detect_deadlock(&self) -> bool {
        let queued_count = self.tasks[..self.task_count]
            .iter()
            .filter(|t| t.status == TaskStatus::Queued || t.status == TaskStatus::Blocked)
            .count();

        if queued_count == 0 {
            return false;
        }

        let idle_count = self.count_idle_agents();
        let working_count = self.count_working_agents();

        idle_count > 0
            && !self.tasks[..self.task_count]
                .iter()
                .any(|t| self.can_start_task(t))
    }
}

#[cfg(all(test, feature = "stage_11"))]
mod agent_tests {
    use super::*;

    #[test]
    fn test_dependency_resolution() {
        let mut dash = CoordinationDashboard::new();
        let mut task1 = Task::new(1, 5);
        let mut task2 = Task::new(2, 5);
        task2.add_dependency(1).ok();

        dash.add_task(task1).ok();
        dash.add_task(task2).ok();

        assert!(!dash.all_dependencies_complete(&task2));
        dash.mark_task_completed(1).ok();
        assert!(dash.all_dependencies_complete(&task2));
    }

    #[test]
    fn test_task_can_start() {
        let mut dash = CoordinationDashboard::new();
        dash.register_agent(1).ok();
        let mut task = Task::new(1, 5);
        dash.add_task(task).ok();
        dash.assign_task(1, 1).ok();
        assert!(dash.can_start_task(&dash.tasks[0]));
    }

    #[test]
    fn test_collision_multiple_files() {
        let mut detector = CollisionDetector::new();
        assert!(detector.try_acquire("file1.rs").is_ok());
        assert!(detector.try_acquire("file2.rs").is_ok());
        assert!(detector.try_acquire("file3.rs").is_ok());
        assert_eq!(detector.file_count, 3);

        assert!(detector.try_acquire("file1.rs").is_err());
        assert!(detector.is_in_use("file2.rs"));

        detector.release("file1.rs");
        assert!(detector.try_acquire("file1.rs").is_ok());
    }

    #[test]
    fn test_workload_distribution() {
        let mut balancer = WorkloadBalancer::new(3);
        let agent1 = balancer.assign_task_to_least_busy();
        balancer.record_task_assignment(agent1);

        let agent2 = balancer.assign_task_to_least_busy();
        assert_ne!(agent1, agent2);

        balancer.record_task_assignment(agent2);
        let agent3 = balancer.assign_task_to_least_busy();
        assert_eq!(balancer.average_load(), 1);
    }

    #[test]
    fn test_critical_file_registry() {
        assert!(CriticalFileLockRegistry::CRITICAL_FILES.len() >= 12);
        assert!(
            CriticalFileLockRegistry::CRITICAL_FILES.contains(&"kernel/src/memory_safety_audit.rs")
        );
    }

    #[test]
    fn test_deadlock_detection() {
        let mut dash = CoordinationDashboard::new();
        dash.register_agent(1).ok();

        let mut task1 = Task::new(1, 5);
        let mut task2 = Task::new(2, 5);
        task2.add_dependency(1).ok();

        dash.add_task(task1).ok();
        dash.add_task(task2).ok();
        dash.assign_task(1, 1).ok();
        dash.assign_task(2, 1).ok();

        assert!(!dash.detect_deadlock());
    }
}

// Integration Tests for Multi-Agent Coordination

#[cfg(all(test, feature = "stage_11"))]
mod integration_tests {
    use super::*;

    #[test]
    fn test_two_agent_coordination_scenario() {
        let mut dash = CoordinationDashboard::new();
        dash.register_agent(1).ok();
        dash.register_agent(2).ok();

        let task1 = Task::new(1, 10);
        let mut task2 = Task::new(2, 5);
        task2.add_dependency(1).ok();

        dash.add_task(task1).ok();
        dash.add_task(task2).ok();

        assert!(dash.assign_task(1, 1).is_ok());
        assert!(dash.assign_task(2, 2).is_ok());

        let task2_ref = &dash.tasks[1];
        assert!(!dash.can_start_task(task2_ref));

        assert!(dash.mark_task_completed(1).is_ok());

        let task2_ref = &dash.tasks[1];
        assert!(dash.can_start_task(task2_ref));

        assert_eq!(dash.agent_count, 2);
    }

    #[test]
    fn test_priority_scheduling_behavior() {
        let mut dash = CoordinationDashboard::new();
        dash.register_agent(1).ok();

        let task_low = Task::new(1, 1);
        let task_high = Task::new(2, 10);
        let task_medium = Task::new(3, 5);

        dash.add_task(task_low).ok();
        dash.add_task(task_high).ok();
        dash.add_task(task_medium).ok();

        dash.assign_task(1, 1).ok();
        dash.assign_task(2, 1).ok();
        dash.assign_task(3, 1).ok();

        if let Some(next_task_id) = dash.find_next_ready_task() {
            assert_eq!(next_task_id, 2);
        }
    }

    #[test]
    fn test_agent_health_monitoring() {
        let mut dash = CoordinationDashboard::new();
        dash.register_agent(1).ok();
        dash.register_agent(2).ok();

        assert_eq!(dash.count_idle_agents(), 2);
        assert_eq!(dash.count_working_agents(), 0);

        dash.update_agent_status(1, AgentStatus::Working).ok();
        assert_eq!(dash.count_idle_agents(), 1);
        assert_eq!(dash.count_working_agents(), 1);

        dash.update_agent_status(1, AgentStatus::Blocked).ok();
        assert_eq!(dash.count_working_agents(), 0);
        assert_eq!(dash.count_blocked_agents(), 1);

        dash.update_agent_status(1, AgentStatus::Complete).ok();
        let agent = &dash.agents[0];
        assert_eq!(agent.status, AgentStatus::Complete);
    }

    #[test]
    fn test_end_to_end_coordination_complex_graph() {
        let mut dash = CoordinationDashboard::new();

        for i in 1..=3 {
            dash.register_agent(i).ok();
        }

        let task1 = Task::new(1, 10);
        let mut task2 = Task::new(2, 8);
        let mut task3 = Task::new(3, 8);
        let mut task4 = Task::new(4, 5);

        task2.add_dependency(1).ok();
        task3.add_dependency(1).ok();
        task4.add_dependency(2).ok();
        task4.add_dependency(3).ok();

        dash.add_task(task1).ok();
        dash.add_task(task2).ok();
        dash.add_task(task3).ok();
        dash.add_task(task4).ok();

        for task_id in 1..=4 {
            let agent_id = ((task_id - 1) % 3) + 1;
            dash.assign_task(task_id, agent_id).ok();
        }

        assert_eq!(dash.find_next_ready_task(), Some(1));

        dash.mark_task_completed(1).ok();

        let ready_task = dash.find_next_ready_task();
        assert!(ready_task == Some(2) || ready_task == Some(3));

        dash.mark_task_completed(2).ok();
        dash.mark_task_completed(3).ok();

        assert_eq!(dash.find_next_ready_task(), Some(4));

        assert!(dash.mark_task_in_progress(4).is_ok());
        assert_eq!(dash.tasks[3].status, TaskStatus::InProgress);
    }

    #[test]
    fn test_workload_balancing_across_agents() {
        let mut balancer = WorkloadBalancer::new(4);

        for _ in 0..10 {
            let agent = balancer.assign_task_to_least_busy();
            balancer.record_task_assignment(agent);
        }

        let avg = balancer.average_load();
        assert!(avg >= 2);

        for i in 0..4 {
            assert!(balancer.agent_loads[i] <= 3);
        }
    }

    #[test]
    fn test_agent_health_reporting() {
        let mut dash = CoordinationDashboard::new();
        dash.register_agent(1).ok();
        dash.register_agent(2).ok();

        let task1 = Task::new(1, 5);
        let task2 = Task::new(2, 5);

        dash.add_task(task1).ok();
        dash.add_task(task2).ok();

        dash.assign_task(1, 1).ok();
        dash.assign_task(2, 2).ok();

        dash.mark_task_in_progress(1).ok();
        dash.mark_task_completed(1).ok();

        let report = DashboardReport {
            total_agents: dash.agent_count,
            working_agents: dash.count_working_agents(),
            idle_agents: dash.count_idle_agents(),
            blocked_agents: dash.count_blocked_agents(),
            total_tasks: dash.task_count,
            tasks_completed: dash.total_tasks_completed(),
            tasks_in_progress: dash.tasks[..dash.task_count]
                .iter()
                .filter(|t| t.status == TaskStatus::InProgress)
                .count(),
        };

        assert_eq!(report.total_agents, 2);
        assert_eq!(report.total_tasks, 2);
        assert_eq!(report.tasks_completed, 1);
    }

    #[test]
    fn test_multiple_agents_same_task_dependency() {
        let mut dash = CoordinationDashboard::new();

        for i in 1..=3 {
            dash.register_agent(i).ok();
        }

        let task1 = Task::new(1, 10);
        let mut task2 = Task::new(2, 5);
        let mut task3 = Task::new(3, 5);
        let mut task4 = Task::new(4, 5);

        task2.add_dependency(1).ok();
        task3.add_dependency(1).ok();
        task4.add_dependency(1).ok();

        dash.add_task(task1).ok();
        dash.add_task(task2).ok();
        dash.add_task(task3).ok();
        dash.add_task(task4).ok();

        dash.assign_task(1, 1).ok();
        dash.assign_task(2, 2).ok();
        dash.assign_task(3, 3).ok();
        dash.assign_task(4, 1).ok();

        dash.mark_task_completed(1).ok();

        assert!(dash.can_start_task(&dash.tasks[1]));
        assert!(dash.can_start_task(&dash.tasks[2]));
        assert!(dash.can_start_task(&dash.tasks[3]));
    }
}
