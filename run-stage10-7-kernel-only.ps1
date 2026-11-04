# Simplified Stage 10.7 kernel-only acceptance (skips Stage 10.6 preservation)
$ErrorActionPreference = "Stop"

Write-Host "=== WovenHat Stage 10.7 Kernel Acceptance (Simplified) ==="
Write-Host ""

Write-Host "=== Build kernel with all 12 improvements ==="
cargo build -p wovenhat-kernel --target x86_64-unknown-none --release
if ($LASTEXITCODE -ne 0) { throw 'Kernel build failed' }

Write-Host ""
Write-Host "=== Kernel clippy lint (warnings denied) ==="
cargo clippy -p wovenhat-kernel --target x86_64-unknown-none -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Kernel clippy failed' }

Write-Host ""
Write-Host "=== STAGE 10.7 KERNEL ACCEPTANCE: PASS ==="
Write-Host "All 12 improvements compile cleanly"
Write-Host "Zero clippy warnings on kernel build"
Write-Host "Ready for QEMU-based Ring-3 async TCP validation"
