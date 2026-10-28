param(
    [string]$Image,
    [ValidateSet(1, 2, 4)][int]$Cpus = 2,
    [string]$Memory = '5G',
    [ValidateSet('whpx', 'tcg')][string]$Accelerator = 'whpx',
    [string]$Qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe'
)
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    $runDir = Join-Path $PSScriptRoot ('target/desktop-interactive-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
    New-Item -ItemType Directory -Path $runDir | Out-Null
    if (-not $Image) {
        $env:WOVENHAT_DESKTOP_OUTPUT = $runDir
        $build = @'
import importlib.util, os
from pathlib import Path
spec = importlib.util.spec_from_file_location('desktop', 'scripts/test-desktop-qualification.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
out = Path(os.environ['WOVENHAT_DESKTOP_OUTPUT'])
image = desktop.build_hardware_demo(out)
(out / 'image-path.txt').write_text(str(image))
'@
        & python -c $build
        if ($LASTEXITCODE -ne 0) { throw "Desktop build failed; logs: $runDir" }
        $Image = (Get-Content (Join-Path $runDir 'image-path.txt') -Raw).Trim()
    }
    $sourceImage = (Resolve-Path -LiteralPath $Image).Path
    # Cargo may regenerate its output while this VM runs. Boot a private,
    # read-only snapshot so a later build cannot change the VM's boot disk.
    $Image = Join-Path $runDir 'wovenhat-os-uefi.img'
    Copy-Item -LiteralPath $sourceImage -Destination $Image
    $firmware = Join-Path (Split-Path $Qemu) 'share/edk2-x86_64-code.fd'
    if (-not (Test-Path -LiteralPath $firmware)) { throw "Missing UEFI firmware: $firmware" }
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    $accelOption = if ($Accelerator -eq 'whpx') { 'whpx' } else { 'tcg,thread=multi,tb-size=256' }
    $qemuArgs = @(
        '-accel', $accelOption, '-machine', 'pc',
        '-m', $Memory, '-smp', "$Cpus", '-name', 'WovenHat Desktop',
        '-serial', "file:$runDir/serial.log", '-no-reboot', '-vga', 'std',
        '-qmp', "tcp:127.0.0.1:$port,server=on,wait=off",
        '-drive', "if=pflash,format=raw,readonly=on,file=$firmware",
        '-drive', "if=none,id=boot,format=raw,readonly=on,file=$Image",
        '-device', 'virtio-blk-pci,drive=boot,bootindex=1',
        '-device', 'qemu-xhci,id=xhci', '-device', 'usb-tablet,bus=xhci.0'
    )
    $qemuArgs | ConvertTo-Json | Set-Content (Join-Path $runDir 'qemu-arguments.json')
    $quotedArgs = $qemuArgs | ForEach-Object { '"' + $_ + '"' }
    # A visible window is intentional: this launcher is for interactive use.
    $process = Start-Process -FilePath $Qemu -ArgumentList $quotedArgs -PassThru `
        -RedirectStandardOutput (Join-Path $runDir 'qemu-stdout.log') `
        -RedirectStandardError (Join-Path $runDir 'qemu-stderr.log')
    @{ pid = $process.Id; accelerator = $Accelerator; qmp_port = $port; image = $Image; source_image = $sourceImage; logs = $runDir } |
        ConvertTo-Json | Set-Content (Join-Path $runDir 'session.json')
    Write-Host "Desktop QEMU PID: $($process.Id)"
    Write-Host 'Demo sign-in: Ada / PIN 1234. USB tablet aligns host and guest pointer positions.'
    Write-Host "Logs: $runDir"
} finally { Pop-Location }
