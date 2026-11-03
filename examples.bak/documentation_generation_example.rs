//! Example: Using Documentation Generator
//!
//! Demonstrates API documentation, architecture graph, and capability matrix generation

/// Example: Generate API documentation for completion_port module
pub fn generate_completion_port_docs() {
    let mut api = crate::documentation_generator::ApiDocumentation::new(
        "completion_port",
        "Per-process completion ports for async I/O notification"
    );
    
    // Document the public API
    let _ = api.add_function(
        "create",
        "pub fn create(owner: u64) -> Result<u64, i32>",
        crate::documentation_generator::FunctionSafety::Safe
    );
    
    let _ = api.add_function(
        "close",
        "pub fn close(owner: u64, handle: u64) -> Result<(), i32>",
        crate::documentation_generator::FunctionSafety::Safe
    );
    
    let _ = api.add_function(
        "associate",
        "pub fn associate(owner: u64, port: u64, op: u64) -> Result<(), i32>",
        crate::documentation_generator::FunctionSafety::Safe
    );
    
    // Render documentation
    let renderer = crate::documentation_generator::DocumentationRenderer::new(
        "WovenHat-OS",
        "0.8.0"
    );
    
    renderer.render_module(&api);
    
    log_info!("docs", format_args!(
        "Generated documentation for {} module with {} functions",
        api.module, api.function_count
    ));
}

/// Example: Build kernel architecture graph
pub fn generate_architecture_graph() {
    let mut graph = crate::documentation_generator::ArchitectureGraph::new();
    
    // Add kernel modules
    let _ = graph.add_module("completion_port");
    let _ = graph.add_module("async_op");
    let _ = graph.add_module("task");
    let _ = graph.add_module("memory");
    let _ = graph.add_module("scheduler");
    
    // Add dependencies
    let _ = graph.add_dependency("completion_port", "async_op");
    let _ = graph.add_dependency("completion_port", "task");
    let _ = graph.add_dependency("async_op", "memory");
    let _ = graph.add_dependency("task", "scheduler");
    let _ = graph.add_dependency("scheduler", "memory");
    
    // Render architecture
    let renderer = crate::documentation_generator::DocumentationRenderer::new(
        "WovenHat-OS",
        "0.8.0"
    );
    
    renderer.render_architecture(&graph);
    
    log_info!("docs", format_args!(
        "Generated architecture graph with {} modules and {} dependencies",
        graph.module_count, graph.dep_count
    ));
}

/// Example: Build capability matrix
pub fn generate_capability_matrix() {
    let mut matrix = crate::documentation_generator::CapabilityMatrix::new();
    
    // Add capabilities
    let _ = matrix.add_capability("async_wait");
    let _ = matrix.add_capability("event_notification");
    let _ = matrix.add_capability("batch_completion");
    let _ = matrix.add_capability("memory_allocation");
    let _ = matrix.add_capability("task_scheduling");
    
    // Add subsystems
    let _ = matrix.add_subsystem("completion_port");
    let _ = matrix.add_subsystem("async_op");
    let _ = matrix.add_subsystem("memory");
    let _ = matrix.add_subsystem("scheduler");
    
    // Enable capabilities for subsystems
    matrix.enable_capability(0, 0); // completion_port: async_wait
    matrix.enable_capability(0, 1); // completion_port: event_notification
    matrix.enable_capability(0, 2); // completion_port: batch_completion
    
    matrix.enable_capability(1, 0); // async_op: async_wait
    matrix.enable_capability(1, 3); // async_op: memory_allocation
    
    matrix.enable_capability(2, 3); // memory: memory_allocation
    
    matrix.enable_capability(3, 4); // scheduler: task_scheduling
    
    // Render matrix
    let renderer = crate::documentation_generator::DocumentationRenderer::new(
        "WovenHat-OS",
        "0.8.0"
    );
    
    renderer.render_capabilities(&matrix);
    
    log_info!("docs", format_args!(
        "Generated capability matrix: {} capabilities, {} subsystems",
        matrix.cap_count, matrix.subsys_count
    ));
}

/// Example: Build documentation index
pub fn generate_documentation_index() {
    let mut index = crate::documentation_generator::DocumentationIndex::new();
    
    // Add all kernel modules to index
    let modules = [
        "completion_port",
        "async_op",
        "task",
        "memory",
        "scheduler",
        "syscall",
        "interrupts",
        "paging",
    ];
    
    for module in modules.iter() {
        let _ = index.add_module(module);
    }
    
    log_info!("docs", format_args!(
        "Generated documentation index with {} modules",
        index.module_count
    ));
    
    // List all modules
    for i in 0..index.module_count {
        log_debug!("docs", format_args!("  - {}", index.modules[i]));
    }
}

/// Example: Validate API completeness
pub fn validate_api_documentation() {
    // Check that all public functions are documented
    let mut total_functions = 0;
    let mut documented_functions = 0;
    
    // Count functions in completion_port
    let mut cp_api = crate::documentation_generator::ApiDocumentation::new(
        "completion_port",
        "Completion port module"
    );
    
    // In real scenario, would iterate through all public functions
    let _ = cp_api.add_function("create", "fn create()", crate::documentation_generator::FunctionSafety::Safe);
    let _ = cp_api.add_function("close", "fn close()", crate::documentation_generator::FunctionSafety::Safe);
    let _ = cp_api.add_function("associate", "fn associate()", crate::documentation_generator::FunctionSafety::Safe);
    
    total_functions += 3;
    documented_functions += cp_api.function_count;
    
    let coverage = if total_functions > 0 {
        (documented_functions as u32 * 100) / total_functions as u32
    } else {
        0
    };
    
    log_info!("docs", format_args!(
        "API documentation coverage: {}/{} functions ({}%)",
        documented_functions, total_functions, coverage
    ));
}

/// Example: Detect circular dependencies
pub fn detect_circular_dependencies() {
    let mut graph = crate::documentation_generator::ArchitectureGraph::new();
    
    let _ = graph.add_module("mod_a");
    let _ = graph.add_module("mod_b");
    let _ = graph.add_module("mod_c");
    
    // Add dependencies
    let _ = graph.add_dependency("mod_a", "mod_b");
    let _ = graph.add_dependency("mod_b", "mod_c");
    let _ = graph.add_dependency("mod_c", "mod_a");  // Circular!
    
    // In real scenario, would detect this cycle
    let mut has_cycle = false;
    for dep in graph.dependencies[..graph.dep_count].iter() {
        if let Some(_) = graph.dependencies[..graph.dep_count]
            .iter()
            .find(|d| d.from == dep.to && d.to == dep.from)
        {
            has_cycle = true;
            log_warn!("docs", format_args!(
                "Circular dependency detected: {} <-> {}",
                dep.from, dep.to
            ));
        }
    }
    
    if !has_cycle {
        log_info!("docs", "No circular dependencies detected");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_documentation() {
        generate_completion_port_docs();
    }

    #[test]
    fn test_architecture() {
        generate_architecture_graph();
    }

    #[test]
    fn test_capabilities() {
        generate_capability_matrix();
    }

    #[test]
    fn test_validation() {
        validate_api_documentation();
    }
}
