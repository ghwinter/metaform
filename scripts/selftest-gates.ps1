<#
.SYNOPSIS
    Prove the gates are alive: create failures on purpose and confirm they really turn red.

.DESCRIPTION
    "The check passed" has two possible meanings: there really is no problem, or nothing is being
    checked at all. This script runs one negative test per gate and requires each to fail as expected:

      lean    create a file with an unused variable -> expect exit code 2 (warning)
      rust    set the expected pass count to 999    -> expect exit code 1
      refs    point a file at a name git does not track -> expect exit code 1
      claims  change the expected value of gain to 2.5 -> expect exit code 1
      cost    break one measured cost point        -> expect exit code 1
      ops no. change one expected ops number        -> expect exit code 1
      probe   change probe.early_correct to true    -> expect exit code 1

    Only when every gate fails as expected are the gates "alive".

.PARAMETER Python
    Python interpreter path (defaults to $env:METAFORM_PYTHON).
#>
[CmdletBinding()]
param(
    [string]$Python = $env:METAFORM_PYTHON
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$local = Join-Path $PSScriptRoot 'local.ps1'
if (Test-Path $local) { . $local }
if (-not $Python) { $Python = $env:METAFORM_PYTHON }
if (-not $Python) {
    $cmd = Get-Command python -ErrorAction SilentlyContinue
    if ($cmd) { $Python = $cmd.Source }
}
$ok = $false
if ($Python) {
    try { & $Python -c "import sys" *> $null; $ok = ($LASTEXITCODE -eq 0) } catch { $ok = $false }
}
if (-not $ok) {
    Write-Host '[selftest-gates] no usable Python: set METAFORM_PYTHON (see scripts/local.example.ps1)'
    exit 2
}
$results = @()

# --- 1. Lean: force a warning ---
$warnFile = Join-Path $root 'research\lean\_warncheck.lean'
'def unused (x : Nat) : Nat := 1' | Set-Content -Path $warnFile -Encoding utf8
try {
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-lean.ps1') -File '_warncheck.lean' | Out-Null
    $code = $LASTEXITCODE
}
finally {
    Remove-Item $warnFile -Force -ErrorAction SilentlyContinue
}
$results += [pscustomobject]@{ Gate = 'lean / warning file'; Got = $code; Want = 2 }

# --- 2. Rust: wrong expected count ---
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-rust.ps1') -Expected 999 | Out-Null
$results += [pscustomobject]@{ Gate = 'rust / wrong expected'; Got = $LASTEXITCODE; Want = 1 }

# --- 2b. References: a file that points at a name git does not track ---
# The marker is assembled at run time so that this script does not contain a
# dangling file name of its own.
$refFile = Join-Path $root '_selftest_ref.md'
$marker = '_selftest' + '_missing.md'
('see `' + $marker + '` for the argument') | Set-Content -Path $refFile -Encoding utf8
try {
    & $Python (Join-Path $PSScriptRoot 'check-references.py') --file '_selftest_ref.md' | Out-Null
    $code = $LASTEXITCODE
}
finally {
    Remove-Item $refFile -Force -ErrorAction SilentlyContinue
}
$results += [pscustomobject]@{ Gate = 'refs / untracked file'; Got = $code; Want = 1 }

# --- 3. Number regression: tamper with an expected value ---
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python verify_claims.py --tamper gain=2.5 | Out-Null
    $code = $LASTEXITCODE
}
finally { Pop-Location }
$results += [pscustomobject]@{ Gate = 'claims / tampered gain'; Got = $code; Want = 1 }

# --- 3b. Cost law: deliberately break one measured point ---
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python verify_claims.py --tamper-cost | Out-Null
    $code = $LASTEXITCODE
}
finally { Pop-Location }
$results += [pscustomobject]@{ Gate = 'claims / tampered cost law'; Got = $code; Want = 1 }

# --- 3c. ops numbers: deliberately use a wrong expected value ---
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python verify_claims.py --tamper-ops | Out-Null
    $code = $LASTEXITCODE
}
finally { Pop-Location }
$results += [pscustomobject]@{ Gate = 'claims / tampered ops number'; Got = $code; Want = 1 }

# --- 4. Probe demo: tamper with early_correct (no collision != correct) ---
$fake = Join-Path $env:TEMP "fake_ops_$PID.cmd"
@'
@echo off
echo {"claim":"x","audit":{"trials":200,"violations":0},"probe":{"early_phi":1,"early_queries":1,"early_correct":true,"verified_queries":5,"verified_cost":14,"verified_correct":true},"scenarios":[]}
'@ | Set-Content -Path $fake -Encoding ASCII
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python -c "import theory_audit, sys; sys.exit(0 if theory_audit.ops_audit(r'$fake') == 0 else 1)" | Out-Null
    $code = $LASTEXITCODE
}
finally {
    Pop-Location
    Remove-Item $fake -Force -ErrorAction SilentlyContinue
}
$results += [pscustomobject]@{ Gate = 'ops / tampered probe'; Got = $code; Want = 1 }

Write-Host ''
Write-Host '=== gate self-test ==='
$bad = 0
foreach ($r in $results) {
    $ok = ($r.Got -eq $r.Want)
    if (-not $ok) { $bad++ }
    Write-Host ("  {0,-24} got={1} want={2}  {3}" -f $r.Gate, $r.Got, $r.Want, $(if ($ok) { 'PASS' } else { 'FAIL' }))
}
if ($bad -gt 0) {
    Write-Host ("[selftest-gates] {0} gate(s) did not fail as expected" -f $bad)
    exit 1
}
Write-Host ("[selftest-gates] {0} gate(s) blocked the injected errors" -f $results.Count)
exit 0

