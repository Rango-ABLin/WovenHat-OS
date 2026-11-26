# Wrapper script for write-live-usb.ps1 with correct parameters for VendorCo USB
$ErrorActionPreference = 'Stop'

# Verify elevation
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this from an Administrator PowerShell window; raw physical disk access is privileged.'
}

$imagePath = "C:\Users\banda\Documents\GitHub\WovenHat-OS\target\debug\build\wovenhat-os-52f6d8f94cd5736e\out\wovenhat-os-uefi.img"
$image = Get-Item $imagePath
$hash = (Get-FileHash -Algorithm SHA256 $imagePath).Hash.ToLowerInvariant()
$length = $image.Length
Write-Host "Image: $imagePath ($length bytes, SHA-256 $hash)"

# Dot-source the write-live-usb.ps1 logic with our parameters
# But write-live-usb.ps1 uses param() so we need to call it with proper quoting
& "C:\Users\banda\Documents\GitHub\WovenHat-OS\scripts\write-live-usb.ps1" `
    -ExpectedModel 'VendorCo ProductCode USB Device' `
    -ExpectedSize 15726735360 `
    -ExpectedSerial '4045141272666606378' `
    -Image $imagePath `
    -ExpectedImageSha256 $hash `
    -Log "C:\Users\banda\Documents\GitHub\WovenHat-OS\audit-artifacts\z3-write-usb-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
