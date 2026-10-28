# Read-only physical TPM/USB qualification preflight.
#
# This script never opens a raw disk, dismounts a volume, changes disk state,
# writes media, or provisions a TPM. It records enough host inventory to decide
# whether the destructive/live qualification gates may be scheduled.

param(
    [string]$Output = ''
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Output) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $Output = Join-Path $root "audit-artifacts/physical-tpm-readiness-$stamp"
}
New-Item -ItemType Directory -Force -Path $Output | Out-Null

function Safe-Value([scriptblock]$Query) {
    try { & $Query } catch { $null }
}

$disks = @(Safe-Value { Get-CimInstance Win32_DiskDrive }) | ForEach-Object {
    $disk = Get-Disk -Number $_.Index -ErrorAction SilentlyContinue
    [ordered]@{
        index = [int]$_.Index
        model = $_.Model
        interface = $_.InterfaceType
        media_type = $_.MediaType
        size_bytes = [long]$_.Size
        serial = if ($_.SerialNumber) { $_.SerialNumber.Trim() } else { $null }
        bus_type = if ($disk) { [string]$disk.BusType } else { $null }
        is_boot = if ($disk) { [bool]$disk.IsBoot } else { $null }
        is_system = if ($disk) { [bool]$disk.IsSystem } else { $null }
        operational_status = if ($disk) { [string]$disk.OperationalStatus } else { $null }
    }
}

$tpmQuery = [ordered]@{ attempted = $true; available = $false; ready = $false; error = $null; details = $null }
try {
    $tpm = Get-CimInstance -Namespace 'root/cimv2/security/microsofttpm' -ClassName Win32_Tpm -ErrorAction Stop |
        Select-Object -First 1
    if ($tpm) {
        $tpmQuery.available = $true
        $tpmQuery.ready = [bool]$tpm.IsEnabled_InitialValue -and [bool]$tpm.IsActivated_InitialValue
        $tpmQuery.details = [ordered]@{
            manufacturer_id = $tpm.ManufacturerId
            manufacturer_version = $tpm.ManufacturerVersion
            spec_version = $tpm.SpecVersion
            enabled = $tpm.IsEnabled_InitialValue
            activated = $tpm.IsActivated_InitialValue
        }
    }
}
catch {
    $tpmQuery.error = $_.Exception.Message
}

$qemu = Get-Command qemu-system-x86_64.exe -ErrorAction SilentlyContinue
$swtpm = Get-Command swtpm.exe -ErrorAction SilentlyContinue
$configPath = Join-Path $root 'target/release-validation/tpm-credential-broker-config.tcb1'
$report = [ordered]@{
    schema = 'WOVENHAT-PHYSICAL-TPM-READINESS-V1'
    generated_utc = [DateTime]::UtcNow.ToString('o')
    repository = [ordered]@{
        root = $root
        commit = (git -C $root rev-parse HEAD).Trim()
        provisioning_record_present = Test-Path -LiteralPath $configPath
    }
    tpm = $tpmQuery
    emulation = [ordered]@{
        qemu_present = [bool]$qemu
        swtpm_present = [bool]$swtpm
        live_qemu_tpm_path_available = [bool]$swtpm
    }
    disks = @($disks)
    usb_targets = @($disks | Where-Object { $_.bus_type -eq 'USB' } | ForEach-Object {
        [ordered]@{
            index = $_.index
            model = $_.model
            size_bytes = $_.size_bytes
            serial = $_.serial
            safe_for_destructive_gate = (-not $_.is_boot) -and (-not $_.is_system)
        }
    })
    safety = [ordered]@{
        read_only = $true
        raw_disk_opened = $false
        disk_state_changed = $false
        media_written = $false
        tpm_provisioned = $false
    }
    readiness = [ordered]@{
        physical_tpm_visible = $tpmQuery.available
        physical_tpm_ready = $tpmQuery.ready
        signed_provisioning_record_available = Test-Path -LiteralPath $configPath
        live_tpm_test_path_available = $tpmQuery.available -or [bool]$swtpm
    }
}

$json = $report | ConvertTo-Json -Depth 8
$json | Set-Content -LiteralPath (Join-Path $Output 'readiness.json') -Encoding UTF8
@(
    "WovenHat physical TPM readiness preflight"
    "Output: $Output"
    "TPM visible: $($report.readiness.physical_tpm_visible)"
    "TPM ready: $($report.readiness.physical_tpm_ready)"
    "QEMU TPM backend: $($report.emulation.live_qemu_tpm_path_available)"
    "Signed provisioning record: $($report.readiness.signed_provisioning_record_available)"
    "USB candidates: $(@($report.usb_targets).Count)"
    "Safety: read-only; no disk or TPM state changed"
) | Set-Content -LiteralPath (Join-Path $Output 'summary.txt') -Encoding UTF8
Get-Content -LiteralPath (Join-Path $Output 'summary.txt')
