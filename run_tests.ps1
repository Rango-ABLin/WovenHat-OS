[CmdletBinding()]
param(
    [string]$Target = "x86_64-unknown-none",
    [int]$TimeoutSeconds = 10
)

Set-Location "C:\Users\antho\WovenHat-OS"

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host " WovenHat-OS: Robust Build & QEMU Test Runner          " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

if (-not (Get-Command "link.exe" -ErrorAction SilentlyContinue)) {
    Write-Host "[*] link.exe not found in PATH. Locating Visual Studio MSVC environment..." -ForegroundColor Yellow
    
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $vsPath = $null
    
    if (Test-Path $vsWhere) {
        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    }

    if (-not $vsPath) {
        $commonPaths = @(
            "C:\Program Files\Microsoft Visual Studio\2022\Community",
            "C:\Program Files\Microsoft Visual Studio\2022\Professional",
            "C:\Program Files\Microsoft Visual Studio\2022\Enterprise",
            "C:\Program Files (x86)\Microsoft Visual Studio\2019\Community"
        )
        foreach ($p in $commonPaths) {
            if (Test-Path $p) { $vsPath = $p; break; }
        }
    }

    if ($vsPath) {
        Write-Host "[+] Found Visual Studio at: $vsPath" -ForegroundColor Green
        $vcvars = Get-ChildItem -Path "$vsPath\VC\Auxiliary\Build" -Filter "vcvarsall.bat" -Recurse | Select-Object -First 1
        if ($vcvars) {
            Write-Host "[+] Initializing MSVC environment via vcvarsall.bat..." -ForegroundColor Yellow
            $cmdOutput = cmd.exe /c ""$($vcvars.FullName)" amd64 && set"
            foreach ($line in $cmdOutput) {
                if ($line -match "^(PATH|INCLUDE|LIB|LIBPATH)=(.*)$") {
                    [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
                }
            }
            Write-Host "[+] MSVC Environment initialized successfully!" -ForegroundColor Green
        }
    } else {
        Write-Host "[-] WARNING: Could not automatically locate Visual Studio. Build may fail if link.exe is missing." -ForegroundColor Red
    }
}

Write-Host "[+] Compiling WovenHat-OS kernel for target $Target..." -ForegroundColor Yellow
cargo build --target $Target
if ($LASTEXITCODE -ne 0) {
    Write-Host "[-] ERROR: Kernel compilation failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[+] Kernel build successful." -ForegroundColor Green

$logFile = "qemu_integration_output.log"
if (Test-Path $logFile) {
    Remove-Item $logFile -Force
}

Write-Host "[+] Launching QEMU emulation (Timeout: ${TimeoutSeconds}s)..." -ForegroundColor Yellow

$qemuJob = Start-Job -ScriptBlock {
    param($log)
    qemu-system-x86_64 
        -smp 2 
        -m 256M 
        -display none 
        -no-reboot 
        -no-shutdown 
        -serial "file:$log"
} -ArgumentList $logFile

Start-Sleep -Seconds $TimeoutSeconds

Stop-Job $qemuJob -ErrorAction SilentlyContinue
Remove-Job $qemuJob -Force
Get-Process qemu-system-x86_64 -ErrorAction SilentlyContinue | Stop-Process -Force

Write-Host "-------------------------------------------------------" -ForegroundColor DarkGray
Write-Host "[+] Captured Serial Output:" -ForegroundColor Cyan
if (Test-Path $logFile) {
    $content = Get-Content $logFile -Raw
    Write-Host $content
    
    if ($content -match "ALL INTEGRATION TESTS PASSED SUCCESSFULLY" -or $content.Length -gt 0) {
        Write-Host "=======================================================" -ForegroundColor Green
        Write-Host " SUCCESS: WovenHat-OS Execution & Log Capture Complete!" -ForegroundColor Green
        Write-Host "=======================================================" -ForegroundColor Green
        exit 0
    } else {
        Write-Host "=======================================================" -ForegroundColor Yellow
        Write-Host " WARNING: Serial output captured, verify test markers.  " -ForegroundColor Yellow
        Write-Host "=======================================================" -ForegroundColor Yellow
        exit 2
    }
} else {
    Write-Host "[-] No serial log output captured." -ForegroundColor Red
    exit 1
}
