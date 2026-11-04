$ErrorActionPreference = "Stop"

# `python3` is a real interpreter on most dev machines, but on some Windows
# installs it resolves to the Microsoft Store app-execution-alias stub
# (exits immediately, does nothing) instead. Resolve a real interpreter once
# and use it for every Python invocation below, rather than silently
# skipping host tests when `python3` doesn't work (the previous behavior)
# or hard-failing the whole gate when only the alias name differs.
function Resolve-PythonCommand {
    foreach ($candidate in @("python3", "python")) {
        $command = Get-Command $candidate -ErrorAction SilentlyContinue
        if (-not $command) { continue }
        # The Windows Store app-execution-alias stub (what `python3`
        # resolves to when no real interpreter is installed under that
        # name) writes to stderr and exits non-zero; under this script's
        # own $ErrorActionPreference = "Stop", a native command's stderr
        # output becomes a terminating exception, not just a non-zero
        # $LASTEXITCODE, so this must be caught, not just checked.
        try {
            $previous = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            & $candidate --version *> $null
            $ErrorActionPreference = $previous
            if ($LASTEXITCODE -eq 0) { return $candidate }
        } catch {
            $ErrorActionPreference = $previous
        }
    }
    throw "No working Python interpreter found (tried python3, python)"
}
$python = Resolve-PythonCommand
Write-Host "Using Python interpreter: $python"

Write-Host "=== WovenHat Stage 10.7 Userspace Async TCP Acceptance ==="
Write-Host ""
Write-Host "=== Host acceptance-harness regressions ==="
& $python -m unittest discover -s tests -p 'test_*.py'
if ($LASTEXITCODE -ne 0) { throw 'Host acceptance-harness regressions failed' }

Write-Host "=== Host lint ==="
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Host lint failed' }

Write-Host "=== 1/2 Preserve complete validated Stage 10.6 baseline ==="
& "$PSScriptRoot\run-stage10-6-acceptance.ps1"
if ($LASTEXITCODE -ne 0) { throw "Stage 10.6 preservation acceptance failed with exit code $LASTEXITCODE" }

Write-Host ""
Write-Host "=== 2/2 Isolated Stage 10.7 Ring-3 async TCP boots: 1/2/4 CPUs ==="
foreach ($cpu in 1,2,4) {
    & $python .\scripts\test-stage10-7-tcp.py --cpus $cpu
    if ($LASTEXITCODE -ne 0) { throw "Stage 10.7 isolated TCP boot failed on $cpu CPU(s)" }
    Write-Host "Stage 10.7 userspace async TCP marker: PASS ($cpu CPU)"
}
Write-Host ""
Write-Host "=== STAGE 10.7 ACCEPTANCE: PASS ==="
Write-Host "Stage 10.6 preserved; ordinary Ring-3 applications now use asynchronous TCP connect/send/receive with generation-pinned sockets, readiness-driven retries, remote EOF completion, deferred close, cancellation, retry-safe copyout, and deterministic teardown on 1/2/4 CPUs."
