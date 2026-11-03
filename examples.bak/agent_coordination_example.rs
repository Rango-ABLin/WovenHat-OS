//! Example: Multi-Agent Coordination Dashboard

pub fn coordinate_agents() {
    let mut dashboard = crate::agent_coordination::CoordinationDashboard::new();
    let mut balancer = crate::agent_coordination::WorkloadBalancer::new(2);
    let mut collision = crate::agent_coordination::CollisionDetector::new();
    
    log_info!("coord", "Initializing agent coordination");
    
    // Register agents
    let _ = dashboard.register_agent(1);  // Claude Code
    let _ = dashboard.register_agent(2);  // Codex
    
    // Create improvement tasks
    for i in 1..=12 {
        let priority = (12 - i as u32) + 1;
        let task = crate::agent_coordination::Task::new(i, priority);
        let _ = dashboard.add_task(task);
    }
    
    log_info!("coord", format_args!("Created {} tasks", dashboard.task_count));
    
    // Assign tasks based on workload
    for task_id in 1..=12 {
        let agent = balancer.assign_task_to_least_busy();
        let _ = dashboard.assign_task(task_id, agent);
        balancer.record_task_assignment(agent);
        
        log_debug!("coord", format_args!(
            "Task {} assigned to agent {}",
            task_id, agent
        ));
    }
    
    // Simulate agent work
    let _ = dashboard.update_agent_status(1, crate::agent_coordination::AgentStatus::Working);
    let _ = dashboard.update_agent_status(2, crate::agent_coordination::AgentStatus::Working);
    
    // Report status
    log_info!("coord", format_args!(
        "System: {} agents, {} tasks",
        dashboard.agent_count, dashboard.task_count
    ));
    log_info!("coord", format_args!(
        "Status: {} working, {} idle, {} blocked",
        dashboard.count_working_agents(),
        dashboard.count_idle_agents(),
        dashboard.count_blocked_agents()
    ));
}

pub fn detect_collisions() {
    let mut detector = crate::agent_coordination::CollisionDetector::new();
    
    log_info!("coord", "Testing collision detection");
    
    // Agent 1 locks file
    match detector.try_acquire("completion_port.rs") {
        Ok(()) => log_info!("coord", "Agent 1: Acquired completion_port.rs"),
        Err(()) => log_warn!("coord", "Agent 1: Collision on completion_port.rs"),
    }
    
    // Agent 2 tries same file
    match detector.try_acquire("completion_port.rs") {
        Ok(()) => log_info!("coord", "Agent 2: Acquired completion_port.rs"),
        Err(()) => log_warn!("coord", "Agent 2: BLOCKED - completion_port.rs in use"),
    }
    
    // Agent 1 releases
    detector.release("completion_port.rs");
    log_info!("coord", "Agent 1: Released completion_port.rs");
    
    // Agent 2 retries
    match detector.try_acquire("completion_port.rs") {
        Ok(()) => log_info!("coord", "Agent 2: Now acquired completion_port.rs"),
        Err(()) => log_warn!("coord", "Agent 2: Still blocked"),
    }
}

pub fn balance_workload() {
    let mut balancer = crate::agent_coordination::WorkloadBalancer::new(3);
    
    log_info!("coord", "Balancing workload across 3 agents");
    
    // Assign 9 tasks
    for i in 0..9 {
        let agent = balancer.assign_task_to_least_busy();
        balancer.record_task_assignment(agent);
        
        log_debug!("coord", format_args!(
            "Task {} assigned to agent {}",
            i, agent
        ));
    }
    
    log_info!("coord", format_args!(
        "Average load per agent: {}",
        balancer.average_load()
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordination() {
        coordinate_agents();
    }

    #[test]
    fn test_collisions() {
        detect_collisions();
    }

    #[test]
    fn test_balance() {
        balance_workload();
    }
}
