<#
.SYNOPSIS
    Build a submittable archive: paper (bilingual), formalization, engine, experiments, and check scripts.

.DESCRIPTION
    When submitting or handing off, you do not want to pick files out of the repository again.
    This script copies the necessary files to dist/staging, keeps the directory structure,
    compresses them into dist/submission-<date>.zip, and writes a MANIFEST.txt.

    Excluded: the offline corpus (too large), build caches, and Rust build artifacts.

    Exit codes: 0 success; 1 a required file is missing.
#>
[CmdletBinding()]
param(
    [string]$Root = (Split-Path $PSScriptRoot -Parent),
    [string]$Stamp = (Get-Date -Format 'yyyyMMdd'),
    [switch]$Tamper   # self-test: deliberately drop one entry from the bundle to see whether verification catches it
)

$ErrorActionPreference = 'Stop'
$dist = Join-Path $Root 'dist'
$stage = Join-Path $dist 'staging'
$zip = Join-Path $dist ("submission-{0}.zip" -f $Stamp)

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

# Relative path list
$files = @(
    'README.md',
    'CHARTER.md',
    'CHARTER.zh.md',
    '.dsh/skills/metaform/SKILL.md',
    'proofs/QueryDriven.lean',
    'proofs/lakefile.toml',
    'proofs/lean-toolchain',
    'proofs/last-check.log',
    'experiments/RESULTS.md',
    'experiments/responses_domain_c.json',
    'experiments/probe_ledger.json',
    'experiments/ledger.py',
    'experiments/engine.py',
    'experiments/domain_a.py',
    'experiments/domain_b.py',
    'experiments/domain_c.py',
    'experiments/lean_oracle/check.lean',
    'experiments/domain_cost.py',
    'experiments/domain_prior.py',
    'experiments/domain_more_data.py',
    'experiments/greedy_cover.py',
    'experiments/noisy_oracle.py',
    'experiments/noisy_loop.py',
    'experiments/adaptive_theta.py',
    'experiments/selfcalibrated_theta.py',
    'experiments/posterior_gate.py',
    'experiments/proposer.py',
    'experiments/proposer_quality.py',
    'experiments/multi_seed.py',
    'experiments/verify_claims.py',
    'experiments/theory_audit.py',
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

# Record each file's size before packing; read back and verify after packing
$sizes = @{}
foreach ($rel in $copied) {
    $sizes[$rel] = (Get-Item (Join-Path $stage $rel)).Length
}

if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$zipSize = [math]::Round((Get-Item $zip).Length / 1KB, 1)

Write-Host ("[make-bundle] {0}  files={1}  missing={2}  size={3} KB" -f $zip, $copied.Count, $missing.Count, $zipSize)
if ($missing.Count -gt 0) {
    Write-Host "[make-bundle] missing:"
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

# --- Read-back check: every entry is in the bundle, and its size matches the source file ---
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
        Write-Host "[make-bundle] bundle incomplete:"
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











