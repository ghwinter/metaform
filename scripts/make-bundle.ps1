<#
.SYNOPSIS
    打一个可提交的归档：论文（双语）、形式化、引擎、实验、检查脚本。

.DESCRIPTION
    投稿或交接时不想再从仓库里挑文件。本脚本把必要文件复制到 dist/staging，
    保持目录结构，压成 dist/submission-<日期>.zip，并写一份 MANIFEST.txt。

    不包含：.library（离线语料，太大）、.lake（构建缓存）、target（Rust 构建产物）。

    退出码：0 成功；1 有必需文件缺失。
#>
[CmdletBinding()]
param(
    [string]$Root = (Split-Path $PSScriptRoot -Parent),
    [string]$Stamp = (Get-Date -Format 'yyyyMMdd'),
    [switch]$Tamper   # 自检用：故意从包里删掉一个条目，看核对能不能抓到
)

$ErrorActionPreference = 'Stop'
$dist = Join-Path $Root 'dist'
$stage = Join-Path $dist 'staging'
$zip = Join-Path $dist ("submission-{0}.zip" -f $Stamp)

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

# 相对路径清单
$files = @(
    'README.md',
    'skills/metaform/SKILL.md',
    'research/00-thesis.md',
    'research/01-formal-core.md',
    'research/02-paper-outline.md',
    'research/03-synthesis.md',
    'research/04-engine-design.md',
    'research/05-borrowables.md',
    'research/06-ai-native.md',
    'research/07-jev-decision-layer.md',
    'research/08-claims-audit.md',
    'research/09-essay-cammarata.md',
    'research/10-tool-architecture.md',
    'research/lean/QueryDriven.lean',
    'research/lean/lakefile.toml',
    'research/lean/lean-toolchain',
    'research/lean/last-check.log',
    'research/exp/RESULTS.md',
    'research/exp/responses_domain_c.json',
    'research/exp/probe_ledger.json',
    'research/exp/ledger.py',
    'research/exp/engine.py',
    'research/exp/domain_a.py',
    'research/exp/domain_b.py',
    'research/exp/domain_c.py',
    'research/exp/lean_oracle/check.lean',
    'research/exp/domain_cost.py',
    'research/exp/domain_prior.py',
    'research/exp/domain_more_data.py',
    'research/exp/greedy_cover.py',
    'research/exp/noisy_oracle.py',
    'research/exp/noisy_loop.py',
    'research/exp/adaptive_theta.py',
    'research/exp/selfcalibrated_theta.py',
    'research/exp/posterior_gate.py',
    'research/exp/proposer.py',
    'research/exp/proposer_quality.py',
    'research/exp/multi_seed.py',
    'research/exp/verify_claims.py',
    'research/exp/theory_audit.py',
    'engine/Cargo.toml',
    'engine/Cargo.lock',
    'engine/src/lib.rs',
    'engine/src/main.rs',
    'engine/src/ledger.rs',
    'engine/src/state.rs',
    'engine/src/mcp.rs',
    'engine/src/plan.rs',
    'engine/src/report.rs',
    'engine/src/score.rs',
    'engine/last-test.log',
    'engine/scenarios/setcover.demo',
    'scripts/check-all.ps1',
    'scripts/check-lean.ps1',
    'scripts/check-rust.ps1',
    'scripts/selftest-gates.ps1',
    'scripts/align-papers.py',
    'scripts/migrate-ledger-to-tsv.py',
    'scripts/release.ps1',
    'scripts/extract-pdf-text.ps1',
    'paper/en/main.tex',
    'paper/en/main.pdf',
    'paper/zh/main.tex',
    'paper/zh/main.pdf'
)

$missing = @()
$copied = @()
foreach ($rel in $files) {
    $src = Join-Path $Root $rel
    if (-not (Test-Path $src)) { $missing += $rel; continue }
    $dst = Join-Path $stage $rel
    New-Item -ItemType Directory -Force -Path (Split-Path $dst -Parent) | Out-Null
    Copy-Item $src $dst -Force
    $copied += $rel
}

$manifest = @()
$manifest += "submission bundle  $Stamp"
$manifest += "root: $Root"
$manifest += "files: $($copied.Count)  missing: $($missing.Count)"
$manifest += ''
foreach ($rel in $copied) {
    $size = (Get-Item (Join-Path $stage $rel)).Length
    $manifest += ("{0,9}  {1}" -f $size, $rel)
}
$manifest += ''
$manifest += "double-blind note: this bundle contains no author names."

$manifest | Set-Content -Path (Join-Path $stage 'MANIFEST.txt') -Encoding utf8

# 打包前记录每个文件的大小，打包后回读核对
$sizes = @{}
foreach ($rel in $copied) {
    $sizes[$rel] = (Get-Item (Join-Path $stage $rel)).Length
}

if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$zipSize = [math]::Round((Get-Item $zip).Length / 1KB, 1)

Write-Host ("[make-bundle] {0}  files={1}  missing={2}  size={3} KB" -f $zip, $copied.Count, $missing.Count, $zipSize)
if ($missing.Count -gt 0) {
    Write-Host "[make-bundle] 缺失："
    $missing | ForEach-Object { Write-Host ("  - {0}" -f $_) }
    exit 1
}

Add-Type -AssemblyName System.IO.Compression.FileSystem

if ($Tamper) {
    $u = [System.IO.Compression.ZipFile]::Open($zip, 'Update')
    $victim = $u.Entries | Select-Object -First 1
    Write-Host ("[make-bundle] TAMPER: removing {0}" -f $victim.FullName)
    $victim.Delete()
    $u.Dispose()
}

# --- 回读核对：包里的每个条目都在，且大小与源文件一致 ---
$archive = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
    $entries = @{}
    foreach ($e in $archive.Entries) {
        $name = $e.FullName -replace '\\', '/'
        $entries[$name] = $e.Length
    }

    $bad = @()
    foreach ($rel in $copied) {
        $key = $rel -replace '\\', '/'
        if (-not $entries.ContainsKey($key)) {
            $bad += "missing in zip: $key"
        }
        elseif ($entries[$key] -ne $sizes[$rel]) {
            $bad += ("size mismatch: {0}  zip={1} src={2}" -f $key, $entries[$key], $sizes[$rel])
        }
    }
    if (-not $entries.ContainsKey('MANIFEST.txt')) { $bad += 'missing in zip: MANIFEST.txt' }

    if ($bad.Count -gt 0) {
        Write-Host "[make-bundle] 包不完整："
        $bad | ForEach-Object { Write-Host ("  - {0}" -f $_) }
        exit 1
    }
    Write-Host ("[make-bundle] verified: {0} entries, all sizes match" -f $entries.Count)
}
finally {
    $archive.Dispose()
    Remove-Item $stage -Recurse -Force
}
exit 0











