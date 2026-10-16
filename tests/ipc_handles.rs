//! Compile the actual production allocator, not a second implementation.
//! IRQ locking, reference ownership and WovenGuard are covered by QEMU probes.

#[test]
fn actual_ipc_allocator_exhaustion_regressions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // Hash the exact source consumed by the extraction runner. Python is
    // already a required dependency of the repository's acceptance harnesses.
    let result = std::process::Command::new("python")
        .current_dir(root)
        .args([
            "-c",
            "import hashlib, runpy, sys; from pathlib import Path; p=Path('kernel/src/ipc.rs'); sys.argv=['scripts/test-ipc-handle-generation.py', '--expected-source-sha256', hashlib.sha256(p.read_bytes()).hexdigest()]; runpy.run_path(sys.argv[0], run_name='__main__')",
        ])
        .output()
        .expect("Python is required for actual-source IPC regression compilation");
    assert!(
        result.status.success(),
        "IPC allocator regression failed ({}):\n{}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
