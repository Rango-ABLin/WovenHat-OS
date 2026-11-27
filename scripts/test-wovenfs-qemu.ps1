# QEMU acceptance for the WovenFS stages (15.1 native filesystem, 15.2 block
# device) without a Python interpreter.
#
# This is a PowerShell port of the two Python harnesses that cover these
# stages, kept to their machine configuration and pass rules:
#
# - 15.1 follows `scripts/test-stage10-runtime.py --stage 15.1`: `q35`, boot
#   disk only.
# - 15.2 follows `scripts/test-wovenfs-device.py`: `pc`, with a disposable
#   blank scratch image attached as the ATA primary master. On `q35` an
#   `if=ide` drive is routed through the ICH9 AHCI controller, where the
#   legacy ports the ATA PIO driver probes are absent, so the disk would be
#   invisible to the filesystem. Raw physical drives are deliberately not
#   supported here; use `scripts/test-wovenfs-physical.ps1`, which gates on
#   disk identity.
#
# Both stages must exit with code 33, emit the stage marker and the SMP online
# line for the requested CPU count, and contain no FAILED/TIMEOUT stage line or
# kernel panic. Evidence is kept under audit-artifacts/ whatever the outcome.

param(
    [Parameter(Mandatory = $true)][ValidateSet('15.1', '15.2')][string]$Stage,
    [ValidateSet(1, 2, 4)][int]$Cpus = 1,
    [int]$TimeoutSeconds = 240,
    [ValidateRange(1, 4096)][int]$SizeMiB = 16,
    [string]$Qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe',
    [string]$Firmware = ''
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if (-not $Firmware) {
    $Firmware = Join-Path (Split-Path $Qemu) 'share\edk2-x86_64-code.fd'
}
if (-not (Test-Path -LiteralPath $Qemu -PathType Leaf) -or
    -not (Test-Path -LiteralPath $Firmware -PathType Leaf)) {
    throw "QEMU ($Qemu) and firmware ($Firmware) files must exist."
}

$feature = 'stage' + $Stage.Replace('.', '-') + '-test'
$marker = @{
    '15.1' = '[S15.1] WovenFS native filesystem: PASSED'
    '15.2' = '[S15.2] WovenFS physical device: PASSED'
}[$Stage]

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$out = Join-Path $root "audit-artifacts\wovenfs-stage$Stage-${Cpus}cpu-$stamp"
New-Item -ItemType Directory -Force -Path $out | Out-Null
$serial = Join-Path $out 'serial.log'

# Reuse the launchers' project-local build tree unless the caller set one.
if (-not $env:CARGO_BUILD_BUILD_DIR) { $env:CARGO_BUILD_BUILD_DIR = Join-Path $root '.cargo-build' }
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $root 'target' }

Push-Location $root
try {
    Write-Host "Building the Stage $Stage image ($feature)..."
    # cargo reports progress on stderr; under Stop that would abort the script,
    # so stderr is collected to a file and the exit code decides.
    $buildErr = Join-Path $out 'build.err'
    $ErrorActionPreference = 'Continue'
    $image = & cargo run --quiet --features $feature -- --print-image 2> $buildErr
    $buildCode = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($buildCode -ne 0) {
        Get-Content -LiteralPath $buildErr | Out-Host
        throw "Build failed (exit $buildCode). Evidence: $out"
    }
    $image = ($image | Select-Object -Last 1).Trim()
}
finally { Pop-Location }

$arguments = @(
    '-accel', 'tcg,tb-size=128', '-m', '256M', '-smp', "$Cpus",
    '-display', 'none', '-serial', "file:$serial", '-no-reboot',
    '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
    '-drive', "if=pflash,unit=0,format=raw,readonly=on,file=$Firmware",
    '-drive', "if=none,id=boot,format=raw,readonly=on,file=$image",
    '-device', 'virtio-blk-pci,drive=boot,bootindex=1'
)
if ($Stage -eq '15.2') {
    $disk = Join-Path $out 'wovenfs.img'
    $stream = [System.IO.File]::Open($disk, [System.IO.FileMode]::CreateNew)
    try { $stream.SetLength([long]$SizeMiB * 1024 * 1024) } finally { $stream.Dispose() }
    Write-Host "Using disposable scratch image $disk ($SizeMiB MiB)"
    $arguments = @('-machine', 'pc') + $arguments +
        @('-drive', "if=ide,index=0,format=raw,file=$disk")
}
else {
    $arguments = @('-machine', 'q35') + $arguments
}

# ProcessStartInfo.Arguments is one string that the child re-splits, so any
# argument containing a space (OVMF under "C:\Program Files") needs quotes.
$quoted = $arguments | ForEach-Object {
    if ($_ -match '\s') { '"' + $_ + '"' } else { $_ }
}
Set-Content -LiteralPath (Join-Path $out 'command.txt') -Encoding utf8 `
    -Value ('"' + $Qemu + '" ' + ($quoted -join ' '))

# System.Diagnostics.Process rather than Start-Process: Windows PowerShell 5.1
# does not reliably surface ExitCode from Start-Process with redirection, and
# the exit code is an independent half of the pass rule.
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = $Qemu
$info.Arguments = ($quoted -join ' ')
$info.WorkingDirectory = $root
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
Write-Host "Booting Stage $Stage on $Cpus CPU..."
$process = [System.Diagnostics.Process]::Start($info)
# Drain both pipes concurrently; a full pipe would otherwise block QEMU.
$stdout = $process.StandardOutput.ReadToEndAsync()
$stderr = $process.StandardError.ReadToEndAsync()
$timedOut = -not $process.WaitForExit($TimeoutSeconds * 1000)
if ($timedOut) {
    # Only the process this script started is terminated.
    $process.Kill()
    $process.WaitForExit()
}
$code = $process.ExitCode
Set-Content -LiteralPath (Join-Path $out 'qemu.log') -Value $stdout.Result -Encoding utf8
Set-Content -LiteralPath (Join-Path $out 'qemu.err') -Value $stderr.Result -Encoding utf8

$log = if (Test-Path -LiteralPath $serial) { [System.IO.File]::ReadAllText($serial) } else { '' }
$errors = @()
if ($timedOut) { $errors += "QEMU timed out after $TimeoutSeconds s" }
foreach ($required in @("[SMP] online=$Cpus expected=$Cpus", $marker)) {
    if (-not $log.Contains($required)) { $errors += "Missing marker: $required" }
}
if ($code -ne 33) { $errors += "QEMU exit $code; expected 33" }
if ($log -match 'KERNEL PANIC|\[S[^\]\n]+\][^\n]*(?:FAILED|TIMEOUT)') {
    $errors += 'Serial log contains a failure, timeout, or panic'
}

foreach ($line in ($log -split "`n" | Where-Object { $_ -match '\[S15\.' })) {
    Write-Host ('  ' + $line.Trim())
}
Set-Content -LiteralPath (Join-Path $out 'result.txt') -Encoding utf8 -Value $(
    if ($errors) { @('FAILED') + $errors } else { "PASS (exit $code)" })

if ($errors) {
    $errors | ForEach-Object { Write-Host $_ }
    $tail = if ($log.Length -gt 4000) { $log.Substring($log.Length - 4000) } else { $log }
    Write-Host $tail
    throw "Stage $Stage FAILED ($Cpus CPU). Evidence: $out"
}
Write-Host "Stage ${Stage}: PASS ($Cpus CPU, exit 33). Evidence: $out"
