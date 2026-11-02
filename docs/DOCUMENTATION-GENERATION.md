# Documentation Generation from Source

## Overview

The documentation generation framework automatically extracts and generates
API documentation, architecture diagrams, and capability matrices from kernel
source code. Keeps documentation synchronized with implementation.

## Architecture

### Core Components

#### DocComment

Extracts doc comments from source:

```rust
pub struct DocComment {
    pub lines: [&'static str; 32],
    pub line_count: usize,
}
```

Stores multi-line documentation comments for processing.

#### ApiDocumentation

Per-module API documentation:

```rust
pub struct ApiDocumentation {
    pub module: &'static str,
    pub description: &'static str,
    pub functions: [ApiFunction; 16],
    pub function_count: usize,
}
```

Tracks functions with signatures and safety levels.

#### FunctionSafety

Safety levels for each function:

- Safe: No preconditions, always safe
- Unsafe: Requires unsafe code or precondition
- SafeWithPrecondition: Safe if conditions met

#### ArchitectureGraph

Module dependency tracking:

```rust
pub struct ArchitectureGraph {
    pub modules: [&'static str; 32],
    pub dependencies: [ModuleDependency; 64],
}
```

Enables dependency analysis and circular dependency detection.

#### CapabilityMatrix

Subsystem-to-capability mapping:

```rust
pub struct CapabilityMatrix {
    pub capabilities: [&'static str; 32],
    pub subsystems: [&'static str; 16],
    pub matrix: [[bool; 32]; 16],
}
```

Shows which subsystems implement which capabilities.

#### DocumentationRenderer

Renders extracted documentation:

```rust
pub fn render_module(&self, api_doc: &ApiDocumentation)
pub fn render_architecture(&self, graph: &ArchitectureGraph)
pub fn render_capabilities(&self, matrix: &CapabilityMatrix)
```

Outputs markdown-formatted documentation.

#### DocumentationIndex

Master index of all documented modules:

```rust
pub struct DocumentationIndex {
    pub modules: [&'static str; 32],
    pub generated_at: u64,
}
```

Tracks documentation generation timestamp.

## Documentation Generation Workflow

### Phase 1: Extract

1. Parse kernel source files
2. Extract doc comments (//! and ///)
3. Parse function signatures
4. Identify function safety levels
5. Store in ApiDocumentation

### Phase 2: Analyze

1. Build module dependency graph
2. Build capability matrix from feature flags
3. Detect circular dependencies
4. Measure API surface area
5. Track safety-critical functions

### Phase 3: Generate

1. Render module API documentation
2. Render architecture diagrams
3. Render capability matrices
4. Create index of all modules
5. Generate cross-references

### Phase 4: Validate

1. Verify all public APIs documented
2. Check for orphaned documentation
3. Validate cross-references
4. Report coverage statistics

## Generated Documentation

### Module API Documentation

For each kernel module:

```markdown
# Module: completion_port

Per-process completion ports for async I/O notification.

## Functions

### create

```rust
pub fn create(owner: u64) -> Result<u64, ()>
```

Safety: Safe

Creates a new completion port. Returns port handle or error.
```

### Architecture Documentation

Dependency graph showing:

```
Modules (45):
- completion_port: 2 dependencies, 3 dependents
- async_op: 1 dependency, 5 dependents
- task: 4 dependencies, 12 dependents
- memory: 0 dependencies, 8 dependents
```

Circular dependency detection:

```
No circular dependencies found
Dependency depth: 8 levels
```

### Capability Matrix

Subsystem-to-capability mapping:

```
Capability Matrix:
- completion_port: 3 capabilities (async_wait, event_notification, batch_completion)
- async_op: 4 capabilities (allocation, tracking, completion, cancellation)
- memory: 5 capabilities (allocation, deallocation, mapping, pinning, protection)
```

### Coverage Statistics

Documentation completeness:

```
API Coverage: 142/145 functions documented (97.9%)
Module Coverage: 45/45 modules documented (100%)
Safety Level: 3 unsafe, 32 safe, 110 precondition-safe
```

## Integration with Source

### In Kernel Modules

Add doc comments to enable generation:

```rust
/// Per-process completion ports
///
/// Ports enable batching of async operation completions.
/// Operations signal completion by writing to port queue.
pub struct Port {
    // ...
}

impl Port {
    /// Create a new completion port
    /// 
    /// Safety: Safe. Allocates from fixed pool.
    pub fn create(owner: u64) -> Result<u64, ()> {
        // ...
    }
}
```

### Module Structure

Register modules for documentation:

```rust
mod completion_port;  // Auto-documented
mod async_op;         // Auto-documented
mod memory;           // Auto-documented
```

## Automatic Checks

During build, verify:

1. All public items have documentation
2. Doc comments are syntactically valid
3. Cross-referenced modules exist
4. No circular dependencies
5. Safety levels correctly declared
6. Capability declarations complete

## Output Formats

### Markdown

Human-readable markdown:

```markdown
# WovenHat-OS Kernel API Documentation

Generated: 2026-11-02 14:30:00 UTC

## Table of Contents

- [completion_port](#completion_port)
- [async_op](#async_op)
- [memory](#memory)
```

### JSON

Machine-parseable JSON:

```json
{
  "modules": [
    {
      "name": "completion_port",
      "functions": [
        {
          "name": "create",
          "signature": "pub fn create(owner: u64) -> Result<u64, ()>",
          "safety": "Safe"
        }
      ]
    }
  ]
}
```

## Usage

### Generate Full Documentation

```bash
cargo build --release
cargo run --features doc-gen -- --output docs/generated
```

### Generate Specific Module

```bash
cargo run --features doc-gen -- --module completion_port
```

### Validate Documentation

```bash
cargo run --features doc-gen -- --validate
```

## Coverage Metrics

Track documentation quality:

- Public items documented: X%
- Functions with examples: Y%
- Safety levels specified: Z%
- Cross-references valid: W%

## Continuous Integration

On each commit:

1. Generate documentation
2. Validate completeness
3. Check for regressions
4. Update generated docs
5. Commit changes (if enabled)

## Performance

Documentation generation:

- Parsing: O(n) where n = source lines
- Analysis: O(m * k) where m = modules, k = avg dependencies
- Generation: O(m + f) where f = functions
- Validation: O(total_symbols)

Typical full generation: < 1 second

## Best Practices

1. **Keep docs updated**: Edit source comments, not generated docs
2. **Use examples**: Show common usage patterns
3. **Document preconditions**: Clearly state safety requirements
4. **Link related**: Cross-reference related modules
5. **Version docs**: Include version in generated output

## Future Enhancements

Planned improvements:

1. **Automatic examples**: Extract from tests
2. **Performance docs**: Document timing guarantees
3. **Memory usage**: Document allocation patterns
4. **Security docs**: Document security properties
5. **Visualization**: Generate architecture diagrams (SVG)
