#Requires -Version 7.0
<#
.SYNOPSIS
    Download the open-access literature and code listed in scripts/library-manifest.json
    into .library/, verify file signatures, log every attempt, and regenerate the index.

.DESCRIPTION
    - Reads scripts/library-manifest.json (items + unavailable list).
    - Downloads with curl.exe (present on this machine), browser UA, retries and timeouts.
    - Writes to "<target>.part" first, checks the file signature (%PDF / <html / PK),
      and only then moves it into place. This prevents saving an HTML error page as a PDF.
    - Skips files that already exist, are > 1 KB, and pass the signature check, unless -Force.
    - Appends one TSV line per attempt to .library/_index/download-log.tsv.
    - Regenerates .library/_index/INDEX.md from the manifest + latest known status.

.PARAMETER Root
    Library root. Default: <repo>/.library

.PARAMETER Manifest
    Manifest path. Default: <script dir>/library-manifest.json

.PARAMETER Force
    Re-download even if a valid file already exists.

.PARAMETER Only
    One or more substrings / regexes matched against item id, section, target or title.
    Example: -Only 08_   or   -Only 'FCA','VSA'

.PARAMETER Retries
    Attempts per URL. Default 3.

.PARAMETER TimeoutSec
    curl --max-time per attempt. Default 180.

.EXAMPLE
    pwsh -File scripts/download-library.ps1
    pwsh -File scripts/download-library.ps1 -Force
    pwsh -File scripts/download-library.ps1 -Only 05_,08_
#>
[CmdletBinding()]
param(
    [string]$Root,
    [string]$Manifest,
    [switch]$Force,
    [string[]]$Only,
    [int]$Retries = 3,
    [int]$TimeoutSec = 180
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if (-not $Root) { $Root = Join-Path (Split-Path $PSScriptRoot -Parent) '.library' }
if (-not $Manifest) { $Manifest = Join-Path $PSScriptRoot 'library-manifest.json' }

if (-not (Test-Path -LiteralPath $Manifest)) {
    Write-Error "Manifest not found: $Manifest"
    exit 2
}

$manifestData = Get-Content -LiteralPath $Manifest -Raw -Encoding utf8 | ConvertFrom-Json
$items = @($manifestData.items)
$unavailable = @($manifestData.unavailable)
$UA = $manifestData.user_agent
if (-not $UA) { $UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36' }

$indexDir = Join-Path $Root '_index'
$logPath = Join-Path $indexDir 'download-log.tsv'
New-Item -ItemType Directory -Force -Path $indexDir | Out-Null

function Format-HumanSize {
    param([long]$Bytes)
    if ($Bytes -ge 1MB) { return ('{0:N1} MB' -f ($Bytes / 1MB)) }
    if ($Bytes -ge 1KB) { return ('{0:N0} KB' -f ($Bytes / 1KB)) }
    return "$Bytes B"
}

function Test-FileSignature {
    param([string]$Path, [string]$Kind)
    try {
        $fs = [System.IO.File]::OpenRead($Path)
        $buf = New-Object byte[] 2048
        $n = $fs.Read($buf, 0, $buf.Length)
        $fs.Dispose()
    }
    catch { return $false }
    if ($n -le 0) { return $false }
    $head = [System.Text.Encoding]::ASCII.GetString($buf, 0, $n)
    switch ($Kind) {
        'pdf' { return $head.StartsWith('%PDF-') }
        'zip' { return ($buf[0] -eq 0x50 -and $buf[1] -eq 0x4B) }
        'html' { return ($head -match '(?i)<!doctype\s+html|<html[\s>]|<\?xml') }
        default { return $true }
    }
}

function Test-SuspiciousHtml {
    param([string]$Path)
    try {
        $fs = [System.IO.File]::OpenRead($Path)
        $buf = New-Object byte[] 8192
        $n = $fs.Read($buf, 0, $buf.Length)
        $fs.Dispose()
    }
    catch { return $false }
    if ($n -le 0) { return $false }
    $head = [System.Text.Encoding]::UTF8.GetString($buf, 0, $n)
    foreach ($marker in @('<title>Lock</title>', 'Just a moment...', 'cf-browser-verification', 'Request unsuccessful. Incapsula', 'Enable JavaScript and cookies to continue')) {
        if ($head.Contains($marker)) { return $true }
    }
    return $false
}

function Get-ShortHash {
    param([string]$Path)
    try { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.Substring(0, 12) }
    catch { return '' }
}

function Test-ItemFilter {
    param($Item)
    if (-not $Only -or $Only.Count -eq 0) { return $true }
    $hay = "$($Item.id) $($Item.section) $($Item.target) $($Item.title)"
    foreach ($pattern in $Only) {
        if ($hay -match $pattern) { return $true }
    }
    return $false
}

# ---------------------------------------------------------------- previous log
$statusMap = @{}
if (Test-Path -LiteralPath $logPath) {
    foreach ($line in (Get-Content -LiteralPath $logPath -Encoding utf8)) {
        if (-not $line -or $line.StartsWith('time')) { continue }
        $f = $line -split "`t"
        if ($f.Count -lt 7) { continue }
        $statusMap[$f[1]] = [pscustomobject]@{
            Time = $f[0]; Id = $f[1]; Status = $f[2]; Http = $f[3]; Bytes = [long]$f[4]
            Sha12 = $f[5]; Type = $f[6]; Url = $(if ($f.Count -gt 7) { $f[7] } else { '' })
            Target = $(if ($f.Count -gt 8) { $f[8] } else { '' })
        }
    }
}

# ---------------------------------------------------------------- download loop
$results = New-Object System.Collections.Generic.List[object]
$logLines = New-Object System.Collections.Generic.List[string]

if (-not (Test-Path -LiteralPath $logPath)) {
    $logLines.Add("time`tid`tstatus`thttp`tbytes`tsha12`ttype`turl`ttarget`ttitle")
}

foreach ($item in $items) {
    $dest = Join-Path $Root ($item.target -replace '/', '\')
    $dir = Split-Path -Parent $dest
    if ($dir -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }

    if (-not (Test-ItemFilter -Item $item)) {
        $results.Add([pscustomobject]@{ Item = $item; Status = 'unfiltered'; Http = ''; Bytes = 0; Sha12 = ''; Url = ''; Msg = '' })
        continue
    }

    $existing = $null
    if (Test-Path -LiteralPath $dest) { $existing = Get-Item -LiteralPath $dest }

    if ($existing -and -not $Force -and $existing.Length -gt 1024 -and (Test-FileSignature -Path $dest -Kind $item.type) -and -not ($item.type -eq 'html' -and (Test-SuspiciousHtml -Path $dest))) {
        Write-Host ("  = {0,-5} exists   {1,10}  {2}" -f $item.id, (Format-HumanSize $existing.Length), $item.target) -ForegroundColor DarkGray
        $results.Add([pscustomobject]@{ Item = $item; Status = 'exists'; Http = ''; Bytes = $existing.Length; Sha12 = (Get-ShortHash $dest); Url = ''; Msg = '' })
        continue
    }

    $done = $false
    $lastHttp = ''
    $lastMsg = ''
    foreach ($url in @($item.urls)) {
        if ($done) { break }
        for ($attempt = 1; $attempt -le $Retries -and -not $done; $attempt++) {
            $tmp = "$dest.part"
            if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force }
            $out = & curl.exe --fail --location --silent --show-error `
                --connect-timeout 20 --max-time $TimeoutSec `
                -A $UA -o $tmp -w "%{http_code}|%{content_type}|%{size_download}" $url 2>&1
            $curlExit = $LASTEXITCODE
            $text = ($out | Out-String).Trim()
            $lastHttp = ($text -split '\|')[0]
            if (-not $lastHttp) { $lastHttp = "exit$curlExit" }

            $size = 0
            if (Test-Path -LiteralPath $tmp) { $size = (Get-Item -LiteralPath $tmp).Length }

            if ($curlExit -eq 0 -and $size -gt 1024 -and (Test-FileSignature -Path $tmp -Kind $item.type) -and -not ($item.type -eq 'html' -and (Test-SuspiciousHtml -Path $tmp))) {
                Move-Item -LiteralPath $tmp -Destination $dest -Force
                $done = $true
                $hash = Get-ShortHash $dest
                Write-Host ("  + {0,-5} download {1,10}  {2}" -f $item.id, (Format-HumanSize $size), $item.target) -ForegroundColor Green
                $results.Add([pscustomobject]@{ Item = $item; Status = 'downloaded'; Http = $lastHttp; Bytes = $size; Sha12 = $hash; Url = $url; Msg = '' })
            }
            else {
                if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force }
                $lastMsg = $text
                Write-Host ("  ! {0,-5} attempt {1}/{2} failed (http={3}, bytes={4})  {5}" -f $item.id, $attempt, $Retries, $lastHttp, $size, $url) -ForegroundColor Yellow
                if ($attempt -lt $Retries) { Start-Sleep -Seconds (2 * $attempt) }
            }
        }
    }

    if (-not $done) {
        Write-Host ("  x {0,-5} FAILED   {1}" -f $item.id, $item.target) -ForegroundColor Red
        $results.Add([pscustomobject]@{ Item = $item; Status = 'failed'; Http = $lastHttp; Bytes = 0; Sha12 = ''; Url = @($item.urls)[0]; Msg = $lastMsg })
    }
}

# ---------------------------------------------------------------- write log
foreach ($r in $results) {
    if ($r.Status -eq 'unfiltered') { continue }
    $line = @(
        (Get-Date).ToString('s'),
        $r.Item.id,
        $r.Status,
        $r.Http,
        $r.Bytes,
        $r.Sha12,
        $r.Item.type,
        ($r.Url -replace "`t", ' '),
        ($r.Item.target -replace "`t", ' '),
        ($r.Item.title -replace "`t", ' ')
    ) -join "`t"
    $logLines.Add($line)
    $statusMap[$r.Item.id] = [pscustomobject]@{
        Time = (Get-Date).ToString('s'); Id = $r.Item.id; Status = $r.Status; Http = $r.Http
        Bytes = $r.Bytes; Sha12 = $r.Sha12; Type = $r.Item.type; Url = $r.Url; Target = $r.Item.target
    }
}
Add-Content -LiteralPath $logPath -Value $logLines -Encoding utf8

# copy manifest snapshot for provenance
Copy-Item -LiteralPath $Manifest -Destination (Join-Path $indexDir 'manifest.snapshot.json') -Force

# ---------------------------------------------------------------- index
$sb = New-Object System.Text.StringBuilder
$now = Get-Date -Format 'yyyy-MM-dd HH:mm'
[void]$sb.AppendLine('# Library Index')
[void]$sb.AppendLine('')
[void]$sb.AppendLine("Generated: $now  |  Source manifest: ``reasoning_engine_research_map.md``  |  Machine facts: ``.local/machine-facts.md``")
[void]$sb.AppendLine('')
[void]$sb.AppendLine('Manifest: `scripts/library-manifest.json`; download log: `_index/download-log.tsv`.')
[void]$sb.AppendLine('Re-download: `pwsh -File scripts/download-library.ps1 -Force`; verify: `pwsh -File scripts/verify-library.ps1`.')
if (Test-Path -LiteralPath (Join-Path $Root '10_code_tools\extracted')) {
    [void]$sb.AppendLine('Extracted sources: `10_code_tools/extracted/` (manifest: `10_code_tools/EXTRACTED.md`).')
}
[void]$sb.AppendLine('')

$sectionTitles = [ordered]@{
    '02_problem_finding'           = '02 Problem finding and cognitive modeling'
    '03a_fca'                      = '03a Formal concept analysis (FCA)'
    '03b_structure_mapping'        = '03b Structure mapping (SME)'
    '03c_vsa_hdc'                  = '03c VSA / HDC'
    '04_rule_program_induction'    = '04 Rules, programs, and reusable abstractions'
    '05_probability_model_selection' = '05 Probability, information theory, and model selection'
    '06_intent_pragmatics'         = '06 Intent inference and pragmatics'
    '07_causal_neuroscience'       = '07 Causal representation learning'
    '08_counterexample_llm'        = '08 Counterexample-driven validation and LLM self-correction'
    '09_scientific_discovery'      = '09 Scientific discovery and theory revision'
    '10_code_tools'                = '10 Code and tools'
}

$orderedSections = @($items | ForEach-Object { $_.section } | Select-Object -Unique)
$totalBytes = 0
$downloaded = 0
$failed = 0

foreach ($section in $orderedSections) {
    $heading = $section
    if ($sectionTitles.Contains($section)) { $heading = $sectionTitles[$section] }
    [void]$sb.AppendLine("## $heading")
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('| id | Title | File | Size | Status |')
    [void]$sb.AppendLine('|---|---|---|---|---|')
    foreach ($item in @($items | Where-Object { $_.section -eq $section })) {
        $rel = $item.target
        $full = Join-Path $Root ($item.target -replace '/', '\')
        $st = 'missing'
        $sizeTxt = '-'
        if (Test-Path -LiteralPath $full) {
            $len = (Get-Item -LiteralPath $full).Length
            $sizeTxt = Format-HumanSize $len
            $totalBytes += $len
            $st = 'ok'
            if ($statusMap.ContainsKey($item.id)) {
                $s = $statusMap[$item.id].Status
                if ($s -eq 'failed') { $st = 'stale (last attempt failed)' }
            }
        }
        elseif ($statusMap.ContainsKey($item.id) -and $statusMap[$item.id].Status -eq 'failed') {
            $st = 'FAILED'
        }
        if ($st -eq 'ok') { $downloaded++ }
        if ($st -eq 'FAILED') { $failed++ }
        $title = ($item.title -replace '\|', '/')
        [void]$sb.AppendLine("| $($item.id) | $title | ``$rel`` | $sizeTxt | $st |")
    }
    [void]$sb.AppendLine('')
}

[void]$sb.AppendLine('## Unavailable (paywall / anti-bot / dead link)')
[void]$sb.AppendLine('')
[void]$sb.AppendLine('| id | Title | Source URL | Reason |')
[void]$sb.AppendLine('|---|---|---|---|')
foreach ($u in $unavailable) {
    [void]$sb.AppendLine("| $($u.id) | $($u.title) | $($u.source) | $($u.reason) |")
}
[void]$sb.AppendLine('')
[void]$sb.AppendLine('## Summary')
[void]$sb.AppendLine('')
[void]$sb.AppendLine("- Manifest items: $($items.Count) (available) + $($unavailable.Count) (unavailable)")
[void]$sb.AppendLine("- On disk: $downloaded  failed: $failed  total size: $(Format-HumanSize $totalBytes)")

$indexPath = Join-Path $indexDir 'INDEX.md'
Set-Content -LiteralPath $indexPath -Value $sb.ToString() -Encoding utf8

# ---------------------------------------------------------------- summary
Write-Host ''
Write-Host ("Library root : {0}" -f $Root)
Write-Host ("Index        : {0}" -f $indexPath)
Write-Host ("Log          : {0}" -f $logPath)
$failedItems = @($results | Where-Object { $_.Status -eq 'failed' })
$newCount = @($results | Where-Object { $_.Status -eq 'downloaded' }).Count
$existCount = @($results | Where-Object { $_.Status -eq 'exists' }).Count
Write-Host ("Downloaded   : {0}   Already present: {1}   Failed: {2}   Total: {3}" -f $newCount, $existCount, $failedItems.Count, (Format-HumanSize $totalBytes))
if ($failedItems.Count -gt 0) {
    Write-Host 'Failed items:'
    foreach ($f in $failedItems) { Write-Host ("  - {0}  {1}  ({2})" -f $f.Item.id, $f.Item.target, $f.Http) }
    exit 1
}
exit 0
