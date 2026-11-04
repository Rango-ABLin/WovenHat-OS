# Stage 10.9 Extensions - Acceptance Gate
$ErrorActionPreference = "Stop"

Write-Host "=== WovenHat Stage 10.9: Extensions Acceptance ==="
Write-Host ""

Write-Host "=== Build kernel with stage10-9-test feature ==="
cargo build -p wovenhat-kernel --target x86_64-unknown-none --release --features stage10-9-test 2>&1
if ($LASTEXITCODE -ne 0) { 
    throw "Stage 10.9 kernel build failed"
}

Write-Host ""
Write-Host "=== Kernel clippy lint (warnings denied) ==="
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage10-9-test -- -D warnings 2>&1
if ($LASTEXITCODE -ne 0) { 
    throw "Stage 10.9 clippy failed"
}

Write-Host ""
Write-Host "=== Stage 10.9 Prerequisites Check ==="
Write-Host "[OK] Kernel compiles with stage10-9-test feature"
Write-Host "[OK] Clippy: zero warnings (warning-denying)"
Write-Host "[OK] Ready for QEMU acceptance matrix (1/2 CPUs)"
Write-Host ""
Write-Host "=== STAGE 10.9 BUILD ACCEPTANCE: PASS ==="
Write-Host "Note: Full QEMU execution deferred to scheduled test run"
Write-Host "Target: 1/2 CPU matrix validation"
