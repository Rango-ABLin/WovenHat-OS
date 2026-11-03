# Capability-Driven Code Organization

## Overview

Organize kernel and application code around security capabilities with compile-time enforcement.

## Key Concept

Every module declares which capabilities it requires via Cargo.toml feature flags.
The build fails if a capability is used without its feature enabled.

## Pattern

Before (no compile-time checking):
```rust
pub mod camera {
    pub fn take_photo() {
        // May or may not have camera hardware access
        // No enforcement
    }
}
```

After (capability-driven):
```rust
#[cfg(feature = "cap-camera")]
pub mod camera {
    /// Requires cap-camera feature
    pub fn take_photo() -> Result<Photo, Error> {
        // Compile fails if cap-camera not enabled
    }
}
```

## Feature Flags (Cargo.toml)

```toml
[features]
# Individual capabilities
cap-console = []
cap-timer-read = []
cap-file-read = []
cap-file-write = ["cap-file-read"]  # Depends on file-read
cap-network-io = []
cap-camera = []
cap-device-io = []

# Capability groups
core = ["cap-console", "cap-timer-read"]
storage = ["cap-file-read", "cap-file-write"]
desktop = ["core", "storage", "cap-device-io"]
```

## Module Declaration

Add to module's lib.rs:

```rust
#[cfg(feature = "cap-camera")]
mod camera;

#[cfg(feature = "cap-network-io")]
mod network;
```

See: examples/capability_organization/camera_module.rs

## Compile-Time Safety

Using a capability without enabling it:

```bash
// Attempt to use camera without feature
let photo = camera::take_photo();

// Error[E0433]: cannot find module `camera` in this scope
// Help: a module with this name does exist in this crate
// Note: it's behind the `cap-camera` feature
```

## Testing Strategy

```bash
# Test core only
cargo test --no-default-features --features cap-timer-read

# Test with specific capabilities
cargo test --no-default-features --features cap-camera,cap-device-io

# Test everything
cargo test --all-features
```

## Benefits

1. Self-Documenting Code
   - cfg guards show module dependencies
   - No guessing what a module needs

2. Compile-Time Safety
   - Wrong capabilities = build error
   - Caught before runtime

3. Binary Size Control
   - Unused features don't compile in
   - Zero runtime overhead

4. Security Verification
   - Clear capability boundaries
   - Auditable capability graph

5. Modular Development
   - Test each capability independently
   - Easier to reason about security model

## Integration Checklist

- [ ] Add cap-* features to Cargo.toml
- [ ] Wrap module code in cfg guards
- [ ] Test with different feature combinations
- [ ] Audit for unguarded capability usage
- [ ] Update architecture documentation
- [ ] Verify binary size (should be smaller)

## Example Use Cases

### Desktop Build
```bash
cargo build --features desktop
# Includes: console, timer, files, network, devices, display
# Excludes: camera, microphone, gpu, usb
```

### Server Build
```bash
cargo build --no-default-features --features core,storage,networking
# Minimal: no display, input, or device-specific code
```

### Full-Featured Build
```bash
cargo build --all-features
# Includes everything
```

## Implementation Timeline

Phase 1 (Week 1): Define capabilities in Cargo.toml
Phase 2 (Week 2): Add cfg guards to modules
Phase 3 (Week 3): Test feature combinations
Phase 4 (Week 4): Audit capability usage

## Verification

After integration, run:

```bash
# Find all unguarded capability usage
grep -r "cap_" kernel/src/ | grep -v "#\[cfg(feature"

# Should be empty or only in documentation
```

## Next Steps

1. Define all capabilities in Cargo.toml
2. Add #[cfg] guards to kernel modules
3. Test: cargo test --all-features
4. Test: cargo test --no-default-features
5. Document capability matrix

