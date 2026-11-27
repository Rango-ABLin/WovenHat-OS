param(
    [ValidateSet(1, 2, 4, 8, 16)][int]$Cpus = 4,
    [string]$Memory = "4G",
    [string]$Qemu = "C:\Program Files\qemu\qemu-system-x86_64.exe",
    [switch]$SkipAcceptance
)
$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot
try {
    $releaseFirmware = Join-Path (Split-Path $Qemu) "share/edk2-x86_64-code.fd"
    if (-not $SkipAcceptance) {
        & python (Join-Path $PSScriptRoot "scripts/test-release.py") --qemu $Qemu --firmware $releaseFirmware
        if ($LASTEXITCODE -ne 0) { throw "Release qualification failed." }
        & python (Join-Path $PSScriptRoot "scripts/package-release.py") --qemu $Qemu --firmware $releaseFirmware
        if ($LASTEXITCODE -ne 0) { throw "Validated release packaging failed." }
    }
    $releaseImage = & cargo run --quiet --release -- --print-image
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }
    $releaseDisk = Join-Path $PSScriptRoot "wovenhat-disk.img"
    & python (Join-Path $PSScriptRoot "scripts/create-fat32.py") $releaseDisk
    if ($LASTEXITCODE -ne 0) { throw "Data disk setup failed." }
    & $Qemu -machine pc -m $Memory -smp $Cpus `
        -drive "if=pflash,format=raw,readonly=on,file=$releaseFirmware" `
        -drive "if=none,id=boot,format=raw,readonly=on,file=$releaseImage" `
        -device virtio-blk-pci,drive=boot,bootindex=1 `
        -drive "if=ide,index=0,format=raw,file=$releaseDisk" `
        -device virtio-net-pci,netdev=net0,disable-modern=on `
        -netdev user,id=net0 -serial stdio -no-reboot
} finally { Pop-Location }
