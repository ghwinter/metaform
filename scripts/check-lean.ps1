<#
.SYNOPSIS
    Compile the Lean formalization under research/lean and keep the output as evidence.

.DESCRIPTION
    This machine has only one copy of mathlib (D:\GitHub\lean\mathlib), but the project that can
    actually load it is D:\GitHub\workspace\ReasearchLean -- the batteries/aesop/Qq and other
    dependencies under its .lake\packages are already compiled. Creating a new project inside
    metaform makes Lake re-materialize those dependencies, and they have no prebuilt artifacts
    (the mathlib cache hook does not work on this machine).

    So: the sources stay in research/lean (part of the paper repository), and compilation borrows
    the ReasearchLean environment.

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
    Lake project whose environment is borrowed.
#>
[CmdletBinding()]
param(
    [string]$File = 'QueryDriven.lean',
    [string]$SourceDir = (Join-Path $PSScriptRoot '..\research\lean'),
    [string]$Project = 'D:\GitHub\workspace\ReasearchLean',
    [string]$LogPath
)

$ErrorActionPreference = 'Stop'
$env:MATHLIB_NO_CACHE_ON_UPDATE = '1'
$env:MATHLIB_CACHE_DIR = 'D:\GitHub\lean\.cache'

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
