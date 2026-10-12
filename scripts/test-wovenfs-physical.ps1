# Physical WovenFS qualification against a real USB block device.
#
# This boots the WovenHat kernel under QEMU with a raw physical disk passed
# through, so the Stage 15.2 self-test drives genuine USB-backed storage
# through the ATA path rather than a RAM image or a file-backed scratch disk.
#
# THE TARGET DISK IS ERASED. The disk is identified by model, exact byte size
# and serial number, and the script refuses to run against anything else.
#
# Requires an Administrator shell: raw \\.\PhysicalDriveN access is privileged.

param(
    # Omit -DiskNumber to locate the target by identity. Removable devices are
    # renumbered between plug events -- this disk moved from 2 to 1 when another
    # USB device was removed -- so a number is not an identity and must never be
    # the only thing standing between a test and the wrong disk.
    [int]$DiskNumber = -1,
    [string]$ExpectedModel = 'VendorCo ProductCode USB Device',
    [long]$ExpectedSize = 8052549120,
    [string]$ExpectedSerial = '5474001281346643368',
    [ValidateSet(1, 2, 4)][int]$Cpus = 1,
    [int]$TimeoutSeconds = 240,
    [string]$Qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe',
    [switch]$Inspect
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this from an Administrator PowerShell window; raw physical disk access is privileged.'
}

# ---- identity gate -------------------------------------------------------
# Refuse unless every field matches. A disk number alone is not an identity:
# removable devices are renumbered between plug events, and the system disk
# must never be reachable by an off-by-one.

$candidates = @(Get-CimInstance Win32_DiskDrive | Where-Object {
    $_.Model -eq $ExpectedModel -and
    [long]$_.Size -eq $ExpectedSize -and
    $_.SerialNumber.Trim() -eq $ExpectedSerial
})
if ($candidates.Count -eq 0) {
    throw "No disk matches the expected identity (model '$ExpectedModel', $ExpectedSize bytes, serial $ExpectedSerial). Is it plugged in?"
}
if ($candidates.Count -gt 1) {
    throw "Refusing: $($candidates.Count) disks share that identity, so the target is ambiguous."
}
$disk = $candidates[0]
if ($DiskNumber -ge 0 -and $disk.Index -ne $DiskNumber) {
    throw "Refusing: the matching disk is index $($disk.Index), but -DiskNumber $DiskNumber was requested."
}
$DiskNumber = $disk.Index
Write-Host "Located target by identity at PhysicalDrive$DiskNumber."

$actualSerial = ($disk.SerialNumber | ForEach-Object { $_.Trim() })
if ($disk.Model -ne $ExpectedModel) {
    throw "Refusing: model is '$($disk.Model)', expected '$ExpectedModel'."
}
if ([long]$disk.Size -ne $ExpectedSize) {
    throw "Refusing: size is $($disk.Size) bytes, expected $ExpectedSize."
}
if ($actualSerial -ne $ExpectedSerial) {
    throw "Refusing: serial is '$actualSerial', expected '$ExpectedSerial'."
}
$system = Get-Disk -Number $DiskNumber
if ($system.IsSystem -or $system.IsBoot) {
    throw 'Refusing: that disk is a system or boot disk.'
}

Write-Host "Target verified: PhysicalDrive$DiskNumber / $($disk.Model) / $($disk.Size) bytes"

if ($Inspect) {
    Write-Host 'Inspect mode: reading the first sectors, writing nothing.'
    $stream = [System.IO.FileStream]::new(
        "\\.\PhysicalDrive$DiskNumber", [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
    try {
        $buffer = New-Object byte[] 512
        $null = $stream.Read($buffer, 0, 512)
        $magic = [System.Text.Encoding]::ASCII.GetString($buffer, 0, 8)
        Write-Host ("LBA0 first 8 bytes: '" + $magic + "'")
        if ($magic -eq 'WOVENFS1') {
            Write-Host 'This disk already carries a WovenFS superblock.'
            $sequence = [BitConverter]::ToUInt64($buffer, 12)
            Write-Host "Checkpoint sequence: $sequence"
        }
    }
    finally { $stream.Dispose() }
    return
}

# ---- dismount only this disk's volumes ----------------------------------
# Drive letters are derived from the verified target, never hardcoded, so this
# cannot dismount a volume belonging to another disk.

# Remove every volume on the target before handing it to QEMU.
#
# Windows refuses raw sector writes that land inside a mounted volume's extent,
# and `fsutil volume dismount` only unmounts until the next access -- the volume
# comes straight back, so QEMU's writes are rejected with no error of its own
# and the guest simply sees I/O failures. Clearing the partition table leaves
# nothing for Windows to mount or defend, which is what makes raw access work.
#
# This is the destructive step. The identity gate above is what stands between
# it and the wrong disk.
Write-Host 'Removing existing volumes so Windows cannot block raw writes...'
try {
    Clear-Disk -Number $DiskNumber -RemoveData -RemoveOEM -Confirm:$false -ErrorAction Stop
    Write-Host 'Partition table cleared; the disk is now raw.'
}
catch {
    Write-Host "Clear-Disk reported: $($_.Exception.Message)"
    Write-Host 'Continuing: the disk may already be raw.'
}

$letters = Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue |
    Where-Object { $_.DriveLetter } |
    ForEach-Object { "$($_.DriveLetter):" }
foreach ($letter in $letters) {
    Write-Host "Dismounting $letter (partition of PhysicalDrive$DiskNumber)"
    & fsutil volume dismount $letter | Out-Host
}
# Taking the disk offline is the belt-and-braces step, but Windows refuses it
# for removable media ("Removable media cannot be set to offline"). The volume
# dismounts above are what actually release Windows' locks, so an offline
# failure on a removable device is expected and not fatal.
$wentOffline = $false
try {
    Set-Disk -Number $DiskNumber -IsOffline $true -ErrorAction Stop
    $wentOffline = $true
    Write-Host 'Disk taken offline.'
}
catch {
    Write-Host 'Disk is removable and cannot be set offline; relying on the volume dismounts.'
}

try {
    $env:CARGO_BUILD_BUILD_DIR = Join-Path $root '.cargo-build'
    $env:CARGO_TARGET_DIR = Join-Path $root 'target'
    $env:CARGO_INCREMENTAL = '0'

    Push-Location $root
    try {
        Write-Host 'Building the Stage 15.2 image...'
        $image = & cargo run --quiet --features stage15-2-test -- --print-image
        if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
        $image = $image.Trim()
    }
    finally { Pop-Location }

    $firmware = Join-Path (Split-Path $Qemu) 'share/edk2-x86_64-code.fd'
    if (-not (Test-Path $firmware)) {
        $firmware = Join-Path (Split-Path $Qemu) 'share/edk2-x86_64-code.fd'
    }
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $out = Join-Path $root "audit-artifacts/wovenfs-physical-$Cpus cpu-$stamp".Replace(' ', '')
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    $serial = Join-Path $out 'serial.log'

    Write-Host "Booting with \\.\PhysicalDrive$DiskNumber attached as the ATA primary master..."
    # `pc`, not `q35`: the kernel's ATA PIO driver probes the legacy primary
    # channel ports. On q35 an `if=ide` drive is routed through the ICH9 AHCI
    # controller instead, where those ports are absent, so the disk would be
    # invisible to the filesystem.
    $arguments = @(
        '-accel', 'tcg,tb-size=128', '-machine', 'pc', '-m', '256M', '-smp', "$Cpus",
        '-display', 'none', '-serial', "file:$serial", '-no-reboot',
        '-device', 'isa-debug-exit,iobase=0xf4,iosize=0x04',
        '-drive', "if=pflash,unit=0,format=raw,readonly=on,file=$firmware",
        '-drive', "if=none,id=boot,format=raw,readonly=on,file=$image",
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-drive', "if=ide,index=0,format=raw,cache=none,file=\\.\PhysicalDrive$DiskNumber"
    )
    # Start-Process joins ArgumentList with spaces and the child re-splits it, so
    # any argument containing a space -- such as the OVMF path under
    # "C:\Program Files" -- must carry its own quotes or QEMU receives a
    # truncated filename.
    $quoted = $arguments | ForEach-Object {
        if ($_ -match '\s') { '"' + $_ + '"' } else { $_ }
    }
    # Launched through System.Diagnostics.Process rather than Start-Process.
    # Start-Process with -PassThru and stream redirection does not reliably
    # surface ExitCode on Windows PowerShell 5.1: it came back empty, which made
    # a passing run look like a failure. The exit code is not optional here --
    # the serial marker says the self-test passed, while the code independently
    # proves the kernel reached its success exit path rather than stopping
    # afterwards.
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Qemu
    $info.Arguments = ($quoted -join ' ')
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($info)
    # Drain both pipes concurrently; a full pipe would otherwise block QEMU.
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        $process.Kill()
        throw "QEMU timed out after $TimeoutSeconds s. Evidence: $out"
    }
    $code = $process.ExitCode
    Set-Content -Path (Join-Path $out 'qemu.log') -Value $stdout.Result -Encoding utf8
    Set-Content -Path (Join-Path $out 'qemu.err') -Value $stderr.Result -Encoding utf8

    $log = if (Test-Path $serial) { Get-Content $serial -Raw } else { '' }
    $marker = '[S15.1] WovenFS native filesystem: PASSED'
    $physical = '[S15.2] WovenFS physical device: PASSED'
    $ok = ($code -eq 33) -and ($log -match [regex]::Escape($physical))

    Write-Host ''
    Write-Host "QEMU exit code: $code (33 is the success code)"
    foreach ($line in ($log -split "`n" | Where-Object { $_ -match '\[S15\.' })) {
        Write-Host ("  " + $line.Trim())
    }
    Write-Host "Evidence: $out"
    if (-not $ok) {
        throw "PHYSICAL WOVENFS QUALIFICATION FAILED. Evidence: $out"
    }
    Write-Host ''
    Write-Host "=== PHYSICAL WOVENFS QUALIFICATION PASSED ($Cpus CPU) ==="
}
finally {
    if ($wentOffline) {
        Write-Host 'Bringing the disk back online.'
        Set-Disk -Number $DiskNumber -IsOffline $false -ErrorAction SilentlyContinue
    }
    Write-Host ''
    Write-Host 'The disk now carries a WovenFS volume, which Windows cannot read.'
    Write-Host 'Windows may offer to format it; decline unless you want it back as NTFS.'
    Write-Host 'To confirm the WovenFS superblock instead, re-run with -Inspect.'
}
