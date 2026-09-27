$ErrorActionPreference = 'Stop'
& "$PSScriptRoot\run-stage13-2-preservation.ps1"
if ($LASTEXITCODE -ne 0) { throw 'Stage 13.2 preservation gate failed' }

Write-Host '=== STAGE 13.2 INCREMENT: PASS; FULL STAGE ACCEPTANCE REMAINS BLOCKED ==='
Write-Host 'Pending: BAR allocation/rebalance, bridge resource routing, generic MSI/MSI-X allocation/programming, PCIe hotplug, and driver-binding integration.'
