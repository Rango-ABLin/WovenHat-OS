[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [scriptblock]$Test
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cachePaths = @(
    (Join-Path $repo 'target\debug'),
    (Join-Path $repo 'target\release'),
    (Join-Path $repo 'target\x86_64-unknown-none')
)

# Include only named compiler-output directories in isolated worktrees. Never
# target `apps/`, an entire worktree `target/`, audit artifacts, serial logs,
# screenshots, or UEFI images.
$worktrees = Join-Path $repo 'target\worktrees'
if (Test-Path -LiteralPath $worktrees) {
    Get-ChildItem -LiteralPath $worktrees -Force -Directory | ForEach-Object {
        $cachePaths += Join-Path $_.FullName '.cargo-build'
        $cachePaths += Join-Path $_.FullName 'target\debug'
        $cachePaths += Join-Path $_.FullName 'target\release'
        $cachePaths += Join-Path $_.FullName 'target\x86_64-unknown-none'
    }
}

$testExit = 0
try {
    & $Test
    $testExit = $LASTEXITCODE
    if ($testExit -ne 0) { throw "Test command failed with exit code $testExit." }
}
finally {
    foreach ($candidate in $cachePaths) {
        if (-not (Test-Path -LiteralPath $candidate)) { continue }
        $item = Get-Item -LiteralPath $candidate -Force
        if (-not $item.PSIsContainer -or
            ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            -not $item.FullName.StartsWith($repo + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing unsafe cache target: $candidate"
        }
        Remove-Item -LiteralPath $item.FullName -Recurse -Force
        Write-Host "Removed disposable cache: $($item.FullName)"
    }
}

exit $testExit
