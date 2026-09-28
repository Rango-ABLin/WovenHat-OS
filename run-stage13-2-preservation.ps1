$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

cargo test --test pci_topology
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 host topology tests failed' }

cargo test --test pci_resource
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 PCI resource tests failed' }

cargo test --test pci_bar
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 PCI BAR tests failed' }

cargo test --test pci_assignment
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 PCI assignment tests failed' }

cargo test --test pci_bridge
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 PCI bridge tests failed' }

cargo test --test pci_routing
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 PCI routing tests failed' }

cargo test --test pci_bridge_transaction
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 PCI bridge transaction tests failed' }

cargo build --features stage13-2-test
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 build failed' }

cargo clippy -p wovenhat-kernel --target x86_64-unknown-none --features stage13-2-test -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 warning-denying kernel Clippy failed' }

python -m unittest tests/test_runtime_harness.py
if ($LASTEXITCODE -ne 0) { throw 'Runtime harness regression tests failed' }

foreach ($cpu in 1,2,4) {
    python .\scripts\test-stage10-runtime.py --stage 13.2 --cpus $cpu --timeout 90
    if ($LASTEXITCODE -ne 0) { throw "Stage 13.2 preservation gate failed on $cpu CPUs" }
}

Write-Host '=== STAGE 13.2 TOPOLOGY PRESERVATION GATE: PASS ==='
