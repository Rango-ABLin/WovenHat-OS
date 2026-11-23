param(
    [string]$OutputRoot = 'D:\WovenHat-OS-output'
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$driveRoot = [System.IO.Path]::GetPathRoot($OutputRoot)
if ($driveRoot -ne 'D:\' -or -not (Test-Path -LiteralPath $driveRoot)) {
    throw "OutputRoot must be on the available D: drive: $OutputRoot"
}
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputRoot)
if ($resolvedOutput -eq $driveRoot) {
    throw 'Refusing to use the drive root itself as an output directory'
}

New-Item -ItemType Directory -Force -Path $resolvedOutput | Out-Null

function Move-And-Link([string]$LocalPath, [string]$Destination) {
    $localFull = [System.IO.Path]::GetFullPath($LocalPath)
    $destinationFull = [System.IO.Path]::GetFullPath($Destination)
    if (-not $localFull.StartsWith($repository + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Local junction path escaped the repository: $localFull"
    }
    if (-not $destinationFull.StartsWith($resolvedOutput + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Destination escaped the output root: $destinationFull"
    }
    $existing = Get-Item -LiteralPath $localFull -Force -ErrorAction SilentlyContinue
    if ($existing -and $existing.LinkType -eq 'Junction') {
        if ([System.IO.Path]::GetFullPath($existing.Target[0]) -ne $destinationFull) {
            throw "Existing junction has a different target: $localFull"
        }
        return
    }
    New-Item -ItemType Directory -Force -Path (Split-Path $destinationFull) | Out-Null
    if ($existing) {
        if (Test-Path -LiteralPath $destinationFull) {
            if (Get-ChildItem -LiteralPath $destinationFull -Force | Select-Object -First 1) {
                throw "Refusing to merge two nonempty output directories: $destinationFull"
            }
            Remove-Item -LiteralPath $destinationFull
        }
        Move-Item -LiteralPath $localFull -Destination $destinationFull
    } else {
        New-Item -ItemType Directory -Force -Path $destinationFull | Out-Null
    }
    New-Item -ItemType Junction -Path $localFull -Target $destinationFull | Out-Null
}

Move-And-Link (Join-Path $repository 'target') (Join-Path $resolvedOutput 'target')
Move-And-Link (Join-Path $repository '.cargo-build') (Join-Path $resolvedOutput 'cargo-build')
Move-And-Link (Join-Path $repository 'audit-artifacts') (Join-Path $resolvedOutput 'audit-artifacts')

$manifests = Get-ChildItem -LiteralPath $repository -Filter Cargo.toml -Recurse -File |
    Where-Object {
        $_.DirectoryName -ne $repository -and
        -not $_.FullName.StartsWith((Join-Path $repository 'target') + '\') -and
        -not $_.FullName.StartsWith((Join-Path $repository '.cargo-build') + '\') -and
        -not $_.FullName.StartsWith((Join-Path $repository 'audit-artifacts') + '\')
    }
foreach ($manifest in $manifests) {
    $relative = $manifest.DirectoryName.Substring($repository.Length).TrimStart('\')
    $key = $relative -replace '[\\/:*?"<>|]', '_'
    Move-And-Link (Join-Path $manifest.DirectoryName 'target') (Join-Path $resolvedOutput "nested-targets\$key")
}

[Environment]::SetEnvironmentVariable('WOVENHAT_OUTPUT_ROOT', $resolvedOutput, 'User')
[Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', (Join-Path $resolvedOutput 'target'), 'User')
[Environment]::SetEnvironmentVariable('CARGO_BUILD_BUILD_DIR', (Join-Path $resolvedOutput 'cargo-build'), 'User')
$env:WOVENHAT_OUTPUT_ROOT = $resolvedOutput
$env:CARGO_TARGET_DIR = Join-Path $resolvedOutput 'target'
$env:CARGO_BUILD_BUILD_DIR = Join-Path $resolvedOutput 'cargo-build'

Write-Host "WovenHat outputs redirected to $resolvedOutput"
