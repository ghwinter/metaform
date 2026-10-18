<#
.SYNOPSIS
    证明四套门是活的：故意制造失败，确认它们真的会红。

.DESCRIPTION
    "检查通过" 有两种可能：真的没问题，或者根本没在检查。
    这个脚本对每套门做一次负向测试，要求它们按预期失败：

      lean    造一个带未使用变量的文件 → 期望退出码 2（有 warning）
      rust    把期望通过数写成 999    → 期望退出码 1
      claims  把 gain 的期望值改成 2.5 → 期望退出码 1
      ops     把 probe.early_correct 改成 true → 期望退出码 1

    四套都按预期失败，才算"门是活的"。

.PARAMETER Python
    Python 解释器路径。
#>
[CmdletBinding()]
param(
    [string]$Python = 'C:\Users\xzxxn\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\python\python.exe'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$results = @()

# --- 1. Lean：制造 warning ---
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

# --- 2. Rust：错误的期望值 ---
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'check-rust.ps1') -Expected 999 | Out-Null
$results += [pscustomobject]@{ Gate = 'rust / wrong expected'; Got = $LASTEXITCODE; Want = 1 }

# --- 3. 数字回归：篡改期望值 ---
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python verify_claims.py --tamper gain=2.5 | Out-Null
    $code = $LASTEXITCODE
}
finally { Pop-Location }
$results += [pscustomobject]@{ Gate = 'claims / tampered gain'; Got = $code; Want = 1 }

# --- 3b. 成本律：故意弄坏一个实测点 ---
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python verify_claims.py --tamper-cost | Out-Null
    $code = $LASTEXITCODE
}
finally { Pop-Location }
$results += [pscustomobject]@{ Gate = 'claims / tampered cost law'; Got = $code; Want = 1 }

# --- 3c. ops 数字：故意改错期望值 ---
Push-Location (Join-Path $root 'research\exp')
try {
    & $Python verify_claims.py --tamper-ops | Out-Null
    $code = $LASTEXITCODE
}
finally { Pop-Location }
$results += [pscustomobject]@{ Gate = 'claims / tampered ops number'; Got = $code; Want = 1 }

# --- 4. 探测演示：篡改 early_correct（无撞车 ≠ 正确） ---
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

