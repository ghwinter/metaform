<#
.SYNOPSIS
    Prove the four gates are alive: create failures on purpose and confirm they really turn red.

.DESCRIPTION
    "The check passed" has two possible meanings: there really is no problem, or nothing is being
    checked at all. This script runs one negative test per gate and requires each to fail as expected:

      lean    create a file with an unused variable -> expect exit code 2 (warning)
      rust    set the expected pass count to 999    -> expect exit code 1
      claims  change the expected value of gain to 2.5 -> expect exit code 1
      ops     change probe.early_correct to true    -> expect exit code 1

    Only when all four fail as expected are the gates "alive".

.PARAMETER Python
    Python interpreter path.
#>
[CmdletBinding()]
param(
    [string]$Python = 'C:\Users\xzxxn\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\python\python.exe'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
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
    Write-Host ("[selftest-gates] {0} 套门没有按预期失败" -f $bad)
    exit 1
}
Write-Host ("[selftest-gates] {0} 套门都能拦住错误" -f $results.Count)
exit 0

