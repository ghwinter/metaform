<#
.SYNOPSIS
    Run all seven assertion-bearing checks with one command: Lean formalization, Rust kernel tests, theorem falsification, bilingual alignment, and paper-number regression.

.DESCRIPTION
    - check-lean: fail on any warning
    - check-rust: the pass count must equal the expected value, and ignored must be 0
    - check-rust (VSA manifest): the same, for the hyperdimensional component's own tests
    - check-components.ps1: the proposal components build, and three proposers are judged by one validator
    - check-references.py: no path, file name or link that the repository does not contain
    - check-claims.py: every claim about the repository still has a command that supports it
    - llm_e2e.py: the propose protocol accepts valid candidates and refuses malformed ones
    - verify_claims.py: key numbers in the paper must match measured values within tolerance

    If any check fails, the whole run fails.

.PARAMETER SkipLean
    Skip Lean (about 25 s from a cold start).

.PARAMETER SkipExperiments
    Skip the Python regression (about 3 s).

.PARAMETER RustExpected
    Expected Rust pass count (default 189).

.PARAMETER Python
    Python interpreter path. Defaults to $env:METAFORM_PYTHON, then to `python` on PATH.
#>
[CmdletBinding()]
param(
    [switch]$SkipLean,
    [switch]$SkipExperiments,
    [switch]$SkipAlign,
    [switch]$SkipTheory,
    [int]$RustExpected = 189,
    [int]$VsaExpected = 19,
    [string]$Python = $env:METAFORM_PYTHON
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$local = Join-Path $PSScriptRoot 'local.ps1'
if (Test-Path $local) { . $local }
if (-not $Python) { $Python = $env:METAFORM_PYTHON }
$results = @()

if (-not $Python) {
    $cmd = Get-Command python -ErrorAction SilentlyContinue
    if ($cmd) { $Python = $cmd.Source }
}
$ok = $false
if ($Python) {
    try { & $Python -c "import sys" *> $null; $ok = ($LASTEXITCODE -eq 0) } catch { $ok = $false }
}
if (-not $ok) {
    Write-Host '[check-all] no usable Python: set METAFORM_PYTHON or put python on PATH'
    exit 2
}

# Every gate that runs a binary expects a fresh one in the workspace target.
Push-Location $root
try {
    cargo build --workspace --bins --quiet
    $build = $LASTEXITCODE
} finally { Pop-Location }
if ($build -ne 0) {
    Write-Host '[check-all] cargo build failed'
    exit 1
}

if (-not $SkipLean) {
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-lean.ps1') | Out-Host
    $results += [pscustomobject]@{ Check = 'lean'; Exit = $LASTEXITCODE }
}

& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-rust.ps1') -Expected $RustExpected | Out-Host
$results += [pscustomobject]@{ Check = 'rust'; Exit = $LASTEXITCODE }

& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-rust.ps1') -Manifest (Join-Path $root 'crates\vsa\Cargo.toml') -Expected $VsaExpected | Out-Host
$results += [pscustomobject]@{ Check = 'vsa'; Exit = $LASTEXITCODE }

if (-not $SkipAlign) {
    & $Python (Join-Path $PSScriptRoot 'align-papers.py') | Out-Host
    $results += [pscustomobject]@{ Check = 'align'; Exit = $LASTEXITCODE }
}

if (-not $SkipReferences) {
    & $Python (Join-Path $PSScriptRoot 'check-references.py') | Out-Host
    $results += [pscustomobject]@{ Check = 'refs'; Exit = $LASTEXITCODE }
}

if (-not $SkipComponents) {
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-components.ps1') | Out-Host
    $results += [pscustomobject]@{ Check = 'components'; Exit = $LASTEXITCODE }
}

if (-not $SkipClaims) {
    & $Python (Join-Path $PSScriptRoot 'check-claims.py') | Out-Host
    $results += [pscustomobject]@{ Check = 'claims'; Exit = $LASTEXITCODE }
}

if (-not $SkipTheory) {
    Push-Location (Join-Path $root 'experiments')
    try {
        $qde = Join-Path $root 'target\debug\metaform.exe'
        if (Test-Path $qde) {
            & $Python theory_audit.py --trials 200 --engine $qde | Out-Host
        }
        else {
            & $Python theory_audit.py --trials 200 | Out-Host
        }
        $results += [pscustomobject]@{ Check = 'theory'; Exit = $LASTEXITCODE }
    }
    finally { Pop-Location }
}

if (-not $SkipExperiments) {
    if (-not (Test-Path $Python)) {
        Write-Host "[check-all] python not found: $Python"
        $results += [pscustomobject]@{ Check = 'experiments'; Exit = 1 }
    }
    else {
        Push-Location (Join-Path $root 'experiments')
        try {
            & $Python verify_claims.py | Out-Host
            $claims = $LASTEXITCODE
            & $Python selection_domain_c.py | Out-Host
            $selection = $LASTEXITCODE
            & $Python e2e_verify.py | Out-Host
            $e2e = $LASTEXITCODE
            & $Python llm_e2e.py | Out-Host
            $adapter = $LASTEXITCODE
            & $Python propose_loop.py | Out-Host
            $loop = $LASTEXITCODE
            $results += [pscustomobject]@{ Check = 'experiments'; Exit = [Math]::Max([Math]::Max([Math]::Max($claims, $selection), $e2e), $adapter) }
        }
        finally { Pop-Location }
    }
}

Write-Host ''
Write-Host '=== summary ==='
$results | ForEach-Object { Write-Host ("  {0,-12} exit={1}" -f $_.Check, $_.Exit) }

$bad = $results | Where-Object { $_.Exit -ne 0 }
if ($bad) {
    Write-Host ("[check-all] {0} check(s) failed" -f $bad.Count)
    exit 1
}
Write-Host '[check-all] all green'
exit 0



















