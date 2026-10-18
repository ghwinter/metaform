#Requires -Version 7.0
<#
.SYNOPSIS
    Verify the files downloaded by download-library.ps1: presence, size, file signature,
    ZIP integrity, duplicate content, and drift from the recorded log.

.EXAMPLE
    pwsh -File scripts/verify-library.ps1
    pwsh -File scripts/verify-library.ps1 -Detailed
#>
[CmdletBinding()]
param(
    [string]$Root,
    [string]$Manifest,
    [switch]$Detailed
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $Root) { $Root = Join-Path (Split-Path $PSScriptRoot -Parent) '.library' }
if (-not $Manifest) { $Manifest = Join-Path $PSScriptRoot 'library-manifest.json' }
if (-not (Test-Path -LiteralPath $Manifest)) { Write-Error "Manifest not found: $Manifest"; exit 2 }

$data = Get-Content -LiteralPath $Manifest -Raw -Encoding utf8 | ConvertFrom-Json
$items = @($data.items)

function Format-HumanSize {
    param([long]$Bytes)
    if ($Bytes -ge 1MB) { return ('{0:N1} MB' -f ($Bytes / 1MB)) }
    if ($Bytes -ge 1KB) { return ('{0:N0} KB' -f ($Bytes / 1KB)) }
    return "$Bytes B"
}

function Get-Head {
    param([string]$Path, [int]$Count = 2048)
    try {
        $fs = [System.IO.File]::OpenRead($Path)
        $buf = New-Object byte[] $Count
        $n = $fs.Read($buf, 0, $Count)
        $fs.Dispose()
        return [pscustomobject]@{ Bytes = $buf; Count = $n; Text = [System.Text.Encoding]::ASCII.GetString($buf, 0, $n) }
    }
    catch { return $null }
}

function Test-Signature {
    param($Head, [string]$Kind)
    if (-not $Head) { return $false }
    switch ($Kind) {
        'pdf' { return $Head.Text.StartsWith('%PDF-') }
        'zip' { return ($Head.Bytes[0] -eq 0x50 -and $Head.Bytes[1] -eq 0x4B) }
        'html' { return ($Head.Text -match '(?i)<!doctype\s+html|<html[\s>]|<\?xml') }
        default { return $true }
    }
}

# load log into map (latest wins)
$logPath = Join-Path $Root '_index/download-log.tsv'
$logMap = @{}
if (Test-Path -LiteralPath $logPath) {
    foreach ($line in (Get-Content -LiteralPath $logPath -Encoding utf8)) {
        if (-not $line -or $line.StartsWith('time')) { continue }
        $f = $line -split "`t"
        if ($f.Count -lt 7) { continue }
        $logMap[$f[1]] = $f
    }
}

$rows = New-Object System.Collections.Generic.List[object]
$hashMap = @{}
$problems = 0
$okCount = 0
$totalBytes = [long]0

foreach ($item in $items) {
    $full = Join-Path $Root ($item.target -replace '/', '\')
    $issues = New-Object System.Collections.Generic.List[string]
    $size = [long]0

    if (-not (Test-Path -LiteralPath $full)) {
        $issues.Add('MISSING')
    }
    else {
        $fi = Get-Item -LiteralPath $full
        $size = $fi.Length
        $totalBytes += $size
        if ($size -lt 1024) { $issues.Add("too small ($size B)") }
        $head = Get-Head -Path $full -Count 8192
        if (-not (Test-Signature -Head $head -Kind $item.type)) { $issues.Add("signature is not $($item.type)") }
        if ($item.type -eq 'html' -and $head) {
            foreach ($marker in @('<title>Lock</title>', 'Just a moment...', 'cf-browser-verification', 'Enable JavaScript and cookies to continue')) {
                if ($head.Text.Contains($marker)) { $issues.Add('HTML is an anti-bot challenge page'); break }
            }
        }

        if ($item.type -eq 'zip' -and $head -and $head.Bytes[0] -eq 0x50) {
            try {
                Add-Type -AssemblyName System.IO.Compression.FileSystem -ErrorAction Stop
                $z = [System.IO.Compression.ZipFile]::OpenRead($full)
                if ($z.Entries.Count -eq 0) { $issues.Add('zip has 0 entries') }
                $z.Dispose()
            }
            catch { $issues.Add("zip unreadable: $($_.Exception.Message)") }
        }

        $h = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash
        if ($hashMap.ContainsKey($h)) { $issues.Add("same content as $($hashMap[$h])") }
        else { $hashMap[$h] = $item.id }

        if ($logMap.ContainsKey($item.id)) {
            $logged = [long]$logMap[$item.id][4]
            if ($logged -gt 0 -and $logged -ne $size) { $issues.Add("size differs from log ($logged -> $size)") }
        }
    }

    if ($issues.Count -gt 0) { $problems++ } else { $okCount++ }
    $rows.Add([pscustomobject]@{
        Id      = $item.id
        Type    = $item.type
        Size    = $size
        SizeTxt = (Format-HumanSize $size)
        File    = $item.target
        Status  = $(if ($issues.Count -eq 0) { 'ok' } else { ($issues -join '; ') })
    })
}

$rows | Format-Table -AutoSize Id, Type, SizeTxt, Status, File | Out-String -Width 220 | Write-Host

if ($Detailed) {
    Write-Host '--- details ---'
    foreach ($r in $rows) { Write-Host ("{0}  {1}  {2}  {3}" -f $r.Id, $r.SizeTxt, $r.Status, $r.File) }
}

Write-Host ''
Write-Host ("Checked : {0}" -f $items.Count)
Write-Host ("OK      : {0}" -f $okCount)
Write-Host ("Problems: {0}" -f $problems)
Write-Host ("Bytes   : {0}" -f (Format-HumanSize $totalBytes))
Write-Host ("Root    : {0}" -f $Root)

if ($problems -gt 0) { exit 1 }
exit 0
