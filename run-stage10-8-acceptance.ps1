# Stage 10.8 Completion Ports - Acceptance Gate
$ErrorActionPreference = "Stop"

Write-Host "=== WovenHat Stage 10.8: Completion Ports Acceptance ==="
Write-Host ""

Write-Host "=== Build kernel with stage10-8-test feature ==="
cargo build -p wovenhat-kernel --target x86_64-unknown-none --release --features stage10-8-test 2>&1
if ($LASTEXITCODE -ne 0) { 
    throw "Stage 10.8 kernel build failed"
}

Write-Host ""
Write-Host "=== Kernel clippy lint (warnings denied) ==="
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage10-8-test -- -D warnings 2>&1
if ($LASTEXITCODE -ne 0) { 
    throw "Stage 10.8 clippy failed"
}

Write-Host ""
Write-Host "=== Stage 10.8 Prerequisites Check ==="
Write-Host "[OK] Kernel compiles with stage10-8-test feature"
Write-Host "[OK] Clippy: zero warnings (warning-denying)"
Write-Host "[OK] Ready for QEMU acceptance matrix (1/2/4 CPUs)"
Write-Host ""
Write-Host "=== STAGE 10.8 BUILD ACCEPTANCE: PASS ==="
Write-Host "Note: Full QEMU execution deferred to scheduled test run"
Write-Host "Target: 1/2/4 CPU matrix validation"
