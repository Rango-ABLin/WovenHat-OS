# Phase 1 Task 1.2: Acceptance Gate Testing
# Tests Stage 10.8, 10.9, and 11.x with all 12 improvements enabled
# Script: RUN-TASK-1.2-GATES.ps1

param(
    [ValidateSet("10.8-1cpu", "10.8-2cpu", "10.8-4cpu", "10.9-1cpu", "10.9-2cpu", "11.1", "11.2", "11.3", "11.4", "11.5", "all")]
    [string]$Stage = "all"
)

$ErrorActionPreference = "Stop"
$StartTime = Get-Date

function Write-Header {
    param([string]$Message)
    Write-Host ""
    Write-Host ("=" * 80) -ForegroundColor Cyan
    Write-Host $Message -ForegroundColor Cyan
    Write-Host ("=" * 80) -ForegroundColor Cyan
    Write-Host ""
}

function Run-Gate {
    param(
        [string]$StageName,
        [string]$Feature,
        [int]$CPUs = 1
    )
    
    Write-Header "Running $StageName (CPUs: $CPUs)"
    
    $LogFile = "logs/gate-$StageName-$CPUs-cpu.log"
    mkdir -Force logs | Out-Null
    
    $QemuArgs = "-smp $CPUs"
    if ($CPUs -gt 1) {
        $QemuArgs += " -m 2G"
    }
    
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Starting $StageName with feature=$Feature, CPUs=$CPUs"
    Write-Host "Command: cargo test -p wovenhat-kernel --target x86_64-unknown-none --features $Feature --release -- --nocapture"
    Write-Host "QEMU Args: $QemuArgs"
    Write-Host ""
    
    $env:QEMU_ARGS = $QemuArgs
    
    try {
        $output = cargo test -p wovenhat-kernel --target x86_64-unknown-none --features $Feature --release -- --nocapture 2>&1
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "[PASS] $StageName completed successfully" -ForegroundColor Green
            $output | Tee-Object -FilePath $LogFile | Select-Object -Last 20
            return $true
        } else {
            Write-Host "[FAIL] $StageName failed with exit code $LASTEXITCODE" -ForegroundColor Red
            $output | Tee-Object -FilePath $LogFile | Select-Object -Last 30
            return $false
        }
    } catch {
        Write-Host "[ERROR] Exception in $StageName : $_" -ForegroundColor Red
        return $false
    }
}

Write-Header "Phase 1 Task 1.2: Acceptance Gate Testing"
Write-Host "Test Matrix:"
Write-Host "  - Stage 10.8: Completion Ports (1/2/4 CPUs)"
Write-Host "  - Stage 10.9: Extensions (1/2 CPUs)"
Write-Host "  - Stage 11.x: Scheduler (11-1 through 11-5)"
Write-Host "  - All with 12 improvements enabled"
Write-Host ""
Write-Host "Execution: $Stage"
Write-Host ""

$Results = @()

# Stage 10.8 Tests
if ($Stage -in @("10.8-1cpu", "all")) {
    $Results += @{ Stage = "Stage 10.8 (1 CPU)"; Pass = (Run-Gate "10.8-1cpu" "stage10-8-test" 1) }
}

if ($Stage -in @("10.8-2cpu", "all")) {
    $Results += @{ Stage = "Stage 10.8 (2 CPU)"; Pass = (Run-Gate "10.8-2cpu" "stage10-8-test" 2) }
}

if ($Stage -in @("10.8-4cpu", "all")) {
    $Results += @{ Stage = "Stage 10.8 (4 CPU)"; Pass = (Run-Gate "10.8-4cpu" "stage10-8-test" 4) }
}

# Stage 10.9 Tests
if ($Stage -in @("10.9-1cpu", "all")) {
    $Results += @{ Stage = "Stage 10.9 (1 CPU)"; Pass = (Run-Gate "10.9-1cpu" "stage10-9-test" 1) }
}

if ($Stage -in @("10.9-2cpu", "all")) {
    $Results += @{ Stage = "Stage 10.9 (2 CPU)"; Pass = (Run-Gate "10.9-2cpu" "stage10-9-test" 2) }
}

# Stage 11.x Tests
if ($Stage -in @("11.1", "all")) {
    $Results += @{ Stage = "Stage 11.1"; Pass = (Run-Gate "11.1" "stage11-1-test" 1) }
}

if ($Stage -in @("11.2", "all")) {
    $Results += @{ Stage = "Stage 11.2"; Pass = (Run-Gate "11.2" "stage11-2-test" 1) }
}

if ($Stage -in @("11.3", "all")) {
    $Results += @{ Stage = "Stage 11.3"; Pass = (Run-Gate "11.3" "stage11-3-test" 1) }
}

if ($Stage -in @("11.4", "all")) {
    $Results += @{ Stage = "Stage 11.4"; Pass = (Run-Gate "11.4" "stage11-4-test" 1) }
}

if ($Stage -in @("11.5", "all")) {
    $Results += @{ Stage = "Stage 11.5"; Pass = (Run-Gate "11.5" "stage11-5-test" 1) }
}

# Summary
Write-Header "Phase 1 Task 1.2: Results Summary"

$PassCount = ($Results | Where-Object { $_.Pass -eq $true }).Count
$FailCount = ($Results | Where-Object { $_.Pass -eq $false }).Count
$TotalTests = $Results.Count

Write-Host "Total Tests: $TotalTests"
Write-Host "Passed: $PassCount" -ForegroundColor Green
Write-Host "Failed: $FailCount" -ForegroundColor $(if ($FailCount -eq 0) { "Green" } else { "Red" })
Write-Host ""

foreach ($Result in $Results) {
    $Status = if ($Result.Pass) { "PASS" } else { "FAIL" }
    $Color = if ($Result.Pass) { "Green" } else { "Red" }
    Write-Host "  [$Status] $($Result.Stage)" -ForegroundColor $Color
}

$Duration = (Get-Date) - $StartTime
Write-Host ""
Write-Host "Total Duration: $($Duration.Hours)h $($Duration.Minutes)m $($Duration.Seconds)s"
Write-Host ""

if ($FailCount -eq 0) {
    Write-Host "STATUS: ALL TESTS PASSED" -ForegroundColor Green
    exit 0
} else {
    Write-Host "STATUS: SOME TESTS FAILED" -ForegroundColor Red
    exit 1
}
