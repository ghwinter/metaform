<#
.SYNOPSIS
    Run all five assertion-bearing checks with one command: Lean formalization, Rust kernel tests, theorem falsification, bilingual alignment, and paper-number regression.

.DESCRIPTION
    - check-lean: fail on any warning
    - check-rust: the pass count must equal the expected value, and ignored must be 0
    - verify_claims.py: key numbers in the paper must match measured values within tolerance

    If any check fails, the whole run fails.

.PARAMETER SkipLean
    Skip Lean (about 25 s from a cold start).

.PARAMETER SkipExperiments
    Skip the Python regression (about 3 s).

.PARAMETER RustExpected
    Expected Rust pass count (default 63).

.PARAMETER Python
    Python interpreter path (this machine has no usable `python`; defaults to the runtime bundled with the harness).
#>
[CmdletBinding()]
param(
    [switch]$SkipLean,
    [switch]$SkipExperiments,
    [switch]$SkipAlign,
    [switch]$SkipTheory,
    [int]$RustExpected = 63,
    [string]$Python = 'C:\Users\xzxxn\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\python\python.exe'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$results = @()

if (-not $SkipLean) {
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-lean.ps1') | Out-Host
    $results += [pscustomobject]@{ Check = 'lean'; Exit = $LASTEXITCODE }
}

& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-rust.ps1') -Expected $RustExpected | Out-Host
$results += [pscustomobject]@{ Check = 'rust'; Exit = $LASTEXITCODE }

if (-not $SkipAlign) {
    & $Python (Join-Path $PSScriptRoot 'align-papers.py') | Out-Host
    $results += [pscustomobject]@{ Check = 'align'; Exit = $LASTEXITCODE }
}

if (-not $SkipTheory) {
    Push-Location (Join-Path $root 'research\exp')
    try {
        $qde = Join-Path $root 'engine\target\debug\metaform.exe'
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
        Push-Location (Join-Path $root 'research\exp')
        try {
            & $Python verify_claims.py | Out-Host
            $claims = $LASTEXITCODE
            & $Python selection_domain_c.py | Out-Host
            $selection = $LASTEXITCODE
            & $Python e2e_verify.py | Out-Host
            $e2e = $LASTEXITCODE
            $results += [pscustomobject]@{ Check = 'experiments'; Exit = [Math]::Max([Math]::Max($claims, $selection), $e2e) }
        }
        finally { Pop-Location }
    }
}

Write-Host ''
Write-Host '=== summary ==='
$results | ForEach-Object { Write-Host ("  {0,-12} exit={1}" -f $_.Check, $_.Exit) }

$bad = $results | Where-Object { $_.Exit -ne 0 }
if ($bad) {
    Write-Host ("[check-all] {0} 项未通过" -f $bad.Count)
    exit 1
}
Write-Host '[check-all] all green'
exit 0



















