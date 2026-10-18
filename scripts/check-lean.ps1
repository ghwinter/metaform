<#
.SYNOPSIS
    Compile the Lean formalization under research/lean and keep the output as evidence.

.DESCRIPTION
    The Lean sources stay in research/lean (part of the paper repository) and are compiled by
    borrowing the environment of an existing Lake project that can already load Mathlib. Point
    METAFORM_LEAN_PROJECT (or the -Project parameter) at such a project; a fresh project inside
    this repository would have to re-materialize Mathlib and its dependencies.

    This script writes all of stdout+stderr to `research/lean/last-check.log` and counts warnings.
    The paper appendix claims "no sorry and no warnings"; that is now reproducible:
    **any warning returns non-zero**. Do not rely on memory.

    Exit codes:
      0  compilation passes with zero warnings
      1  Lean compilation fails (or the source is missing)
      2  compilation passes with warnings

.PARAMETER File
    Source file name (under research/lean).

.PARAMETER Project
    Lake project whose environment is borrowed (defaults to $env:METAFORM_LEAN_PROJECT).
#>
[CmdletBinding()]
param(
    [string]$File = 'QueryDriven.lean',
    [string]$SourceDir = (Join-Path $PSScriptRoot '..\research\lean'),
    [string]$Project = $env:METAFORM_LEAN_PROJECT,
    [string]$LogPath
)

$ErrorActionPreference = 'Stop'
$local = Join-Path $PSScriptRoot 'local.ps1'
if (Test-Path $local) { . $local }
if (-not $Project) { $Project = $env:METAFORM_LEAN_PROJECT }
if (-not $Project -or -not (Test-Path $Project)) {
    Write-Host '[check-lean] no Lake project: set METAFORM_LEAN_PROJECT (see scripts/local.example.ps1)'
    exit 1
}
$env:MATHLIB_NO_CACHE_ON_UPDATE = '1'

$src = Join-Path $SourceDir $File
if (-not (Test-Path $src)) {
    Write-Host "[check-lean] source not found: $src"
    exit 1
}
if (-not $LogPath) { $LogPath = Join-Path $SourceDir 'last-check.log' }

$dstDir = Join-Path $Project 'Research'
New-Item -ItemType Directory -Force -Path $dstDir | Out-Null
Copy-Item $src (Join-Path $dstDir $File) -Force

Push-Location $Project
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $out = & lake env lean (Join-Path 'Research' $File) 2>&1 | Out-String
    $code = $LASTEXITCODE
    $secs = [math]::Round($sw.Elapsed.TotalSeconds, 1)

    $header = "# check-lean run: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')  file=$File  exit=$code  secs=$secs`n"
    ($header + $out) | Set-Content -Path $LogPath -Encoding utf8

    $warnings = ([regex]::Matches($out, '(?m)^.*\bwarning\b.*$')).Count
    $sorries = ([regex]::Matches($out, '(?m)^.*\bsorry\b.*$')).Count

    Write-Host ("[check-lean] {0}  exit={1}  warnings={2}  sorry-lines={3}  {4}s" -f $File, $code, $warnings, $sorries, $secs)
    Write-Host ("[check-lean] log: {0}" -f $LogPath)

    if ($code -ne 0) { exit 1 }
    if ($warnings -gt 0) { exit 2 }
    exit 0
}
finally { Pop-Location }
