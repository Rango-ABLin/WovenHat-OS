# Write the release-validated WovenHat image to a USB stick for live
# hardware testing.
#
# THE TARGET DISK IS ERASED. The disk is identified by model, exact byte size
# and serial number, and the script refuses to run against anything else, or
# against a system, boot or non-USB disk. The image must be the one recorded
# by scripts/test-release.py (target/release-validation/validated-image.json),
# and its SHA-256 must still match that record.
#
# After writing, the image region is read back and compared byte for byte.
# The rest of the stick then becomes the FAT32 partition labelled WOVENLOG
# where each WovenHat boot keeps its log (Stage 13.19).
#
# Requires an Administrator shell: raw \\.\PhysicalDriveN access is privileged.

param(
    [string]$ExpectedModel = 'VendorCo ProductCode USB Device',
    [long]$ExpectedSize = 8052549120,
    [string]$ExpectedSerial = '5474001281346643368',
    [string]$Manifest = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release-validation\validated-image.json'),
    [string]$Log = ''
)

$ErrorActionPreference = 'Stop'
if ($Log) { Start-Transcript -Path $Log -Force | Out-Null }
try {
    if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this from an Administrator PowerShell window; raw physical disk access is privileged.'
    }

    # ---- image -----------------------------------------------------------
    $record = Get-Content -Raw $Manifest | ConvertFrom-Json
    $image = $record.image
    $hash = (Get-FileHash -Algorithm SHA256 $image).Hash.ToLowerInvariant()
    if ($hash -ne $record.image_sha256) {
        throw "Refusing: $image has SHA-256 $hash, but the release record says $($record.image_sha256)."
    }
    $length = (Get-Item $image).Length
    if ($length % 512 -ne 0) { throw "Refusing: image length $length is not a whole number of sectors." }
    Write-Host "Image: $image ($length bytes, SHA-256 $hash)"

    # ---- identity gate ---------------------------------------------------
    # A disk number alone is not an identity: removable devices are renumbered
    # between plug events, and the system disk must never be reachable by an
    # off-by-one.
    $candidates = @(Get-CimInstance Win32_DiskDrive | Where-Object {
        $_.Model -eq $ExpectedModel -and
        [long]$_.Size -eq $ExpectedSize -and
        $_.SerialNumber.Trim() -eq $ExpectedSerial
    })
    if ($candidates.Count -ne 1) {
        throw "Refusing: $($candidates.Count) disks match model '$ExpectedModel', $ExpectedSize bytes, serial $ExpectedSerial."
    }
    $number = $candidates[0].Index
    $disk = Get-Disk -Number $number
    if ($disk.IsSystem -or $disk.IsBoot) { throw 'Refusing: that disk is a system or boot disk.' }
    if ($disk.BusType -ne 'USB') { throw "Refusing: that disk is on bus $($disk.BusType), not USB." }
    if ($disk.SerialNumber.Trim() -ne $ExpectedSerial) { throw 'Refusing: Get-Disk reports a different serial.' }
    if ($length -gt $ExpectedSize) { throw 'Refusing: the image is larger than the disk.' }
    Write-Host "Target verified: PhysicalDrive$number / $($candidates[0].Model) / $ExpectedSize bytes / serial $ExpectedSerial"

    # ---- erase and write -------------------------------------------------
    # Windows refuses raw writes over a mounted volume. Lock and dismount
    # every volume on the target disk (and only that disk), and hold the
    # locks until the write is done. A stick formatted without a partition
    # table is one volume starting at sector 0, which Clear-Disk leaves in
    # place, so the locks are what make the write possible.
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class VolumeLock {
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFile(string name, uint access, uint share, IntPtr security,
        uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool DeviceIoControl(SafeFileHandle device, uint code, IntPtr input, uint inputSize,
        IntPtr output, uint outputSize, out uint returned, IntPtr overlapped);
    const uint FSCTL_LOCK_VOLUME = 0x00090018, FSCTL_DISMOUNT_VOLUME = 0x00090020;
    public static SafeFileHandle LockAndDismount(string path) {
        var handle = CreateFile(path, 0xC0000000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero);
        if (handle.IsInvalid) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "open " + path);
        uint returned;
        if (!DeviceIoControl(handle, FSCTL_LOCK_VOLUME, IntPtr.Zero, 0, IntPtr.Zero, 0, out returned, IntPtr.Zero))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "lock " + path);
        if (!DeviceIoControl(handle, FSCTL_DISMOUNT_VOLUME, IntPtr.Zero, 0, IntPtr.Zero, 0, out returned, IntPtr.Zero))
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "dismount " + path);
        return handle;
    }
}
'@
    $locks = @()
    foreach ($volume in @(Get-Partition -DiskNumber $number -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue)) {
        $path = $volume.Path.TrimEnd('\') -replace '^\\\\\?\\', '\\.\'
        $locks += [VolumeLock]::LockAndDismount($path)
        Write-Host "Locked and dismounted $path"
    }
    $stream =[System.IO.FileStream]::new("\\.\PhysicalDrive$number", [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::ReadWrite, 4096,
        [System.IO.FileOptions]::WriteThrough)
    $source = [System.IO.File]::OpenRead($image)
    try {
        $buffer = New-Object byte[] (4MB)
        while (($read = $source.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $stream.Write($buffer, 0, $read)
        }
        $stream.Flush()
        # Verify: read the image region back and hash it.
        $stream.Position = 0
        $sha = [System.Security.Cryptography.SHA256]::Create()
        $offset = 0L
        while ($offset -lt $length) {
            $want = [int][Math]::Min($buffer.Length, $length - $offset)
            $got = 0
            while ($got -lt $want) {
                $n = $stream.Read($buffer, $got, $want - $got)
                if ($n -le 0) { throw "Read-back ended early at offset $($offset + $got)." }
                $got += $n
            }
            [void]$sha.TransformBlock($buffer, 0, $got, $null, 0)
            $offset += $got
        }
        [void]$sha.TransformFinalBlock($buffer, 0, 0)
        $readBack = -join ($sha.Hash | ForEach-Object { $_.ToString('x2') })
        if ($readBack -ne $hash) { throw "Read-back SHA-256 $readBack does not match the image $hash." }
        Write-Host "Written and verified: $offset bytes, read-back SHA-256 matches."
    } finally {
        $source.Dispose()
        $stream.Dispose()
        foreach ($lock in $locks) { $lock.Dispose() }
    }
    Update-Disk -Number $number

    # ---- the live-test log partition (Stage 13.19) ------------------------
    # The rest of the stick becomes a FAT32 partition labelled WOVENLOG: the
    # one volume a WovenHat boot writes, keeping LOGS\BOOTnnnn.TXT for every
    # boot so the stick brings the logs back.
    $logPartition = New-Partition -DiskNumber $number -UseMaximumSize -GptType '{ebd0a0a2-b9e5-4433-87c0-68b6b72699c7}'
    $null = Format-Volume -Partition $logPartition -FileSystem FAT32 -NewFileSystemLabel 'WOVENLOG' -Force -Confirm:$false
    $logPartition = Get-Partition -DiskNumber $number -PartitionNumber $logPartition.PartitionNumber
    # The kernel recognises the partition by the label in its boot sector.
    # Windows' FAT32 format keeps the label only in the root directory and
    # leaves NO NAME in the boot sector, so set it there, in the boot sector
    # and its backup (sector 6), with the volume locked.
    $volumePath = (($logPartition | Get-Volume).Path.TrimEnd('\')) -replace '^\\\\\?\\', '\\.\'
    $volumeLock = [VolumeLock]::LockAndDismount($volumePath)
    try {
        $stream = [System.IO.FileStream]::new("\\.\PhysicalDrive$number", [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::ReadWrite, 4096,
            [System.IO.FileOptions]::WriteThrough)
        try {
            $labelBytes = [System.Text.Encoding]::ASCII.GetBytes('WOVENLOG   ')
            foreach ($sectorIndex in 0, 6) {
                $sector = New-Object byte[] 512
                $stream.Position = $logPartition.Offset + $sectorIndex * 512
                $null = $stream.Read($sector, 0, 512)
                if ([System.Text.Encoding]::ASCII.GetString($sector, 82, 8) -ne 'FAT32   ') {
                    throw "Sector $sectorIndex of the log partition is not a FAT32 boot sector."
                }
                [Array]::Copy($labelBytes, 0, $sector, 71, 11)
                $stream.Position = $logPartition.Offset + $sectorIndex * 512
                $stream.Write($sector, 0, 512)
            }
            $stream.Flush()
        } finally {
            $stream.Dispose()
        }
    } finally {
        $volumeLock.Dispose()
    }
    $stream = [System.IO.FileStream]::new("\\.\PhysicalDrive$number", [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
    try {
        $boot = New-Object byte[] 512
        $stream.Position = $logPartition.Offset
        $null = $stream.Read($boot, 0, 512)
    } finally {
        $stream.Dispose()
    }
    $label = [System.Text.Encoding]::ASCII.GetString($boot, 71, 11)
    $fileSystem = [System.Text.Encoding]::ASCII.GetString($boot, 82, 8)
    if ($label -ne 'WOVENLOG   ' -or $fileSystem -ne 'FAT32   ') {
        throw "The log partition's boot sector reads label '$label', file system '$fileSystem'."
    }
    Write-Host "Log partition: $([math]::Round($logPartition.Size / 1MB)) MiB FAT32, label WOVENLOG at offset $($logPartition.Offset)."
    $letter = ($logPartition | Get-Volume).DriveLetter
    if ($letter) {
        Set-Content -Path "${letter}:\README.TXT" -Encoding ASCII -Value @(
            'WovenHat OS live-test stick',
            '',
            'Boot a machine from this stick (UEFI, Secure Boot off). Each boot writes',
            'LOGS\BOOTnnnn.TXT here: a summary of every problem the boot reported,',
            'the hardware it found, and the full kernel log. In the WovenHat shell,',
            '"note <text>" adds your own observation and "savelog" saves at once.',
            'Bring the stick back to read and fix what the logs show.')
    }
    Get-Partition -DiskNumber $number | Format-Table PartitionNumber, Offset, Size, GptType -AutoSize | Out-String | Write-Host
    Write-Host 'DONE'
} catch {
    Write-Host "FAILED: $_"
    exit 1
} finally {
    if ($Log) { Stop-Transcript | Out-Null }
}
