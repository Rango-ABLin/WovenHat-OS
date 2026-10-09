param(
    [Parameter(Mandatory = $true)]
    [string]$Image,
    [int]$DiskNumber = 1
)

$ErrorActionPreference = 'Stop'

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this script from an Administrator PowerShell window.'
}

$disk = Get-CimInstance Win32_DiskDrive | Where-Object { $_.Index -eq $DiskNumber }
if (-not $disk) { throw "PhysicalDrive$DiskNumber was not found." }
if ($disk.Model -ne 'USB2.0 Flash Disk USB Device' -or [int64]$disk.Size -ne 518192640) {
    throw "Refusing target identity: model='$($disk.Model)' size='$($disk.Size)'."
}

$imageFile = Get-Item -LiteralPath $Image
$imageHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $imageFile.FullName).Hash
if ($imageFile.Length -ge [int64]$disk.Size) { throw 'The image does not fit on the target disk.' }

Write-Host "Target: PhysicalDrive$DiskNumber / $($disk.Model) / $($disk.Size) bytes"
Write-Host "Image:  $($imageFile.FullName) / $($imageFile.Length) bytes"
Write-Host "SHA256: $imageHash"

# Dismount only volumes that belong to the validated target disk. A hardcoded
# drive letter would dismount whatever happens to be mounted there, which need
# not be a partition of this disk.
$targetLetters = Get-Partition -DiskNumber $DiskNumber -ErrorAction SilentlyContinue |
    Where-Object { $_.DriveLetter } |
    ForEach-Object { "$($_.DriveLetter):" }
if (-not $targetLetters) {
    Write-Host "No mounted volumes on PhysicalDrive$DiskNumber; nothing to dismount."
}
foreach ($letter in $targetLetters) {
    Write-Host "Dismounting $letter (partition of PhysicalDrive$DiskNumber)"
    & fsutil volume dismount $letter | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Could not dismount $letter." }
}

$device = "\\.\PhysicalDrive$DiskNumber"
$buffer = New-Object byte[] (1024 * 1024)
$inputStream = [System.IO.File]::OpenRead($imageFile.FullName)
try {
    $outputStream = [System.IO.FileStream]::new(
        $device, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite,
        [System.IO.FileShare]::None)
    try {
        while (($count = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $outputStream.Write($buffer, 0, $count)
        }
        $outputStream.Flush($true)
    }
    finally { $outputStream.Dispose() }
}
finally { $inputStream.Dispose() }

$readStream = [System.IO.FileStream]::new(
    $device, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read,
    [System.IO.FileShare]::ReadWrite)
try {
    $readback = New-Object byte[] $imageFile.Length
    $offset = 0
    while ($offset -lt $readback.Length) {
        $count = $readStream.Read($readback, $offset, $readback.Length - $offset)
        if ($count -le 0) { throw 'Short raw readback.' }
        $offset += $count
    }
}
finally { $readStream.Dispose() }

$readbackHash = [Convert]::ToHexString(
    [Security.Cryptography.SHA256]::Create().ComputeHash($readback))
if ($readbackHash -ne $imageHash) {
    throw "Readback mismatch: expected $imageHash, got $readbackHash."
}

Write-Host "RAW WRITE VERIFIED: $readbackHash"
