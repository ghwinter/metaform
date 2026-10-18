<#
.SYNOPSIS
    一条命令跑完五套"有断言"的检查：Lean 形式化、Rust 内核测试、定理证伪、双语对齐、论文数字回归。

.DESCRIPTION
    - check-lean：有 warning 就失败
    - check-rust：通过数必须等于期望值、ignored 必须为 0
    - verify_claims.py：论文里的关键数字与实测值的差必须在容差内

    任何一项失败，整体失败。

.PARAMETER SkipLean
    跳过 Lean（冷启动约 25 s）。

.PARAMETER SkipExperiments
    跳过 Python 回归（约 3 s）。

.PARAMETER RustExpected
    Rust 期望通过数（默认 63）。

.PARAMETER Python
    Python 解释器路径（本机没有可用的 `python`，默认用 harness 自带运行时）。
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



















