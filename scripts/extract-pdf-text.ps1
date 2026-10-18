<#
.SYNOPSIS
    Extract PDFs under .library into plain text for reading and synthesis.

.DESCRIPTION
    Uses pdftotext (from MiKTeX or poppler) found on PATH. PDFs emit many font warnings, so all of
    them are swallowed; only the exit code and the output volume are kept, written to text-log.tsv.

    The output layout mirrors .library:
        .library/02_problem_finding/foo.pdf
            -> .library/_text/02_problem_finding/foo.txt

.PARAMETER Library
    Library root, default .library at the repository root.

.PARAMETER Only
    Only process files whose path contains this substring (for example 03a_).

.PARAMETER Force
    Re-extract even when a non-empty txt already exists.
#>
[CmdletBinding()]
param(
    [string]$Library = (Join-Path $PSScriptRoot '..\.library'),
    [string]$Only,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$Library = (Resolve-Path $Library).Path
$OutRoot = Join-Path $Library '_text'
$LogPath = Join-Path $OutRoot '_index\text-log.tsv'

$pdftotext = (Get-Command pdftotext -ErrorAction SilentlyContinue).Source
if (-not $pdftotext) {
    throw 'pdftotext.exe not found: put MiKTeX (or poppler) on PATH'
}

New-Item -ItemType Directory -Force -Path (Join-Path $OutRoot '_index') | Out-Null
if (Test-Path $LogPath) { Remove-Item $LogPath -Force }
Add-Content -Path $LogPath -Value ("relpath`tchars`tlines`tstatus") -Encoding utf8

$pdfs = Get-ChildItem $Library -Recurse -Filter *.pdf -File |
    Where-Object { $_.FullName -notlike "*\_text\*" }
if ($Only) { $pdfs = $pdfs | Where-Object { $_.FullName -like "*$Only*" } }

$rows = @()
$ok = 0; $skip = 0; $fail = 0
$sw = [Diagnostics.Stopwatch]::StartNew()

foreach ($pdf in $pdfs) {
    $rel = [System.IO.Path]::GetRelativePath($Library, $pdf.FullName)
    $relTxt = [System.IO.Path]::ChangeExtension($rel, '.txt')
    $dst = Join-Path $OutRoot $relTxt
    $dstDir = Split-Path $dst -Parent
    if (-not (Test-Path $dstDir)) { New-Item -ItemType Directory -Force -Path $dstDir | Out-Null }

    if ((Test-Path $dst) -and -not $Force -and (Get-Item $dst).Length -gt 500) {
        $t = Get-Content $dst -Raw
        $rows += [pscustomobject]@{ rel = $rel; chars = $t.Length; lines = ($t -split "`n").Count; status = 'exists' }
        $skip++
        continue
    }

    & $pdftotext -enc UTF-8 -layout $pdf.FullName $dst 2>$null
    $code = $LASTEXITCODE
    if ($code -eq 0 -and (Test-Path $dst) -and (Get-Item $dst).Length -gt 200) {
        $t = Get-Content $dst -Raw
        $rows += [pscustomobject]@{ rel = $rel; chars = $t.Length; lines = ($t -split "`n").Count; status = 'ok' }
        $ok++
    }
    else {
        if (Test-Path $dst) { Remove-Item $dst -Force }
        $rows += [pscustomobject]@{ rel = $rel; chars = 0; lines = 0; status = "fail($code)" }
        $fail++
    }
}

foreach ($r in $rows) {
    Add-Content -Path $LogPath -Value ("{0}`t{1}`t{2}`t{3}" -f $r.rel, $r.chars, $r.lines, $r.status) -Encoding utf8
}

$secs = [math]::Round($sw.Elapsed.TotalSeconds, 1)
Write-Host ("pdf={0}  ok={1}  exists={2}  fail={3}  {4}s" -f $pdfs.Count, $ok, $skip, $fail, $secs)
$rows | Sort-Object chars -Descending | Select-Object -First 10 |
    Format-Table @{ n = 'rel'; e = { $_.rel } }, chars, status -AutoSize | Out-String | Write-Host

if ($fail -gt 0) { exit 1 }
