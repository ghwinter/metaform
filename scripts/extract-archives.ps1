#Requires -Version 7.0
<#
.SYNOPSIS
    Extract the ZIP archives listed in scripts/library-manifest.json into
    .library/10_code_tools/extracted/ (one directory per archive), idempotently.

.DESCRIPTION
    - Reads items with type = zip from the manifest.
    - Detects each archive's single top-level folder and extracts there, so the
      result is .library/10_code_tools/extracted/<archive-top-folder>/.
    - Skips archives that are already extracted unless -Force is given.
    - Verifies the expected number of files after extraction.
    - Writes 10_code_tools/EXTRACTED.md and appends to _index/extract-log.tsv.

.EXAMPLE
    pwsh -File scripts/extract-archives.ps1
    pwsh -File scripts/extract-archives.ps1 -Force
#>
[CmdletBinding()]
param(
    [string]$Root,
    [string]$Manifest,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $Root) { $Root = Join-Path (Split-Path $PSScriptRoot -Parent) '.library' }
if (-not $Manifest) { $Manifest = Join-Path $PSScriptRoot 'library-manifest.json' }
if (-not (Test-Path -LiteralPath $Manifest)) { Write-Error "Manifest not found: $Manifest"; exit 2 }

Add-Type -AssemblyName System.IO.Compression.FileSystem
$data = Get-Content -LiteralPath $Manifest -Raw -Encoding utf8 | ConvertFrom-Json
$zips = @($data.items | Where-Object { $_.type -eq 'zip' })

$destRoot = Join-Path $Root '10_code_tools\extracted'
$indexDir = Join-Path $Root '_index'
New-Item -ItemType Directory -Force -Path $destRoot | Out-Null
New-Item -ItemType Directory -Force -Path $indexDir | Out-Null
$logPath = Join-Path $indexDir 'extract-log.tsv'
if (-not (Test-Path -LiteralPath $logPath)) {
    Set-Content -LiteralPath $logPath -Value "time`tid`tstatus`tarchive`ttarget`tfiles`tsize" -Encoding utf8
}

function Format-HumanSize {
    param([long]$Bytes)
    if ($Bytes -ge 1MB) { return ('{0:N1} MB' -f ($Bytes / 1MB)) }
    if ($Bytes -ge 1KB) { return ('{0:N0} KB' -f ($Bytes / 1KB)) }
    return "$Bytes B"
}

$rows = New-Object System.Collections.Generic.List[object]
$failed = 0

foreach ($item in $zips) {
    $zipPath = Join-Path $Root ($item.target -replace '/', '\')
    $zipName = [System.IO.Path]::GetFileNameWithoutExtension($zipPath)
    if (-not (Test-Path -LiteralPath $zipPath)) {
        Write-Host ("  ? {0,-5} archive missing: {1}" -f $item.id, $item.target) -ForegroundColor Yellow
        $rows.Add([pscustomobject]@{ Id = $item.id; Archive = $item.target; Folder = ''; Files = 0; Status = 'archive missing' })
        $failed++
        continue
    }

    $z = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    $entryCount = $z.Entries.Count
    $topNames = @($z.Entries | ForEach-Object { ($_.FullName -split '/')[0] } | Sort-Object -Unique)
    $z.Dispose()

    if ($topNames.Count -eq 1 -and $topNames[0]) {
        $folderName = $topNames[0]
        $extractTo = $destRoot
    }
    else {
        $folderName = $zipName
        $extractTo = Join-Path $destRoot $folderName
        New-Item -ItemType Directory -Force -Path $extractTo | Out-Null
    }

    $folderPath = Join-Path $destRoot $folderName
    $existingFiles = 0
    if (Test-Path -LiteralPath $folderPath) {
        $existingFiles = @(Get-ChildItem -LiteralPath $folderPath -Recurse -File).Count
    }

    if ($existingFiles -gt 0 -and -not $Force) {
        Write-Host ("  = {0,-5} already extracted ({1} files): {2}" -f $item.id, $existingFiles, $folderName) -ForegroundColor DarkGray
        $rows.Add([pscustomobject]@{ Id = $item.id; Archive = $item.target; Folder = $folderName; Files = $existingFiles; Status = 'exists' })
        continue
    }

    try {
        Expand-Archive -LiteralPath $zipPath -DestinationPath $extractTo -Force
        $files = @(Get-ChildItem -LiteralPath $folderPath -Recurse -File)
        $size = ($files | Measure-Object -Property Length -Sum).Sum
        Write-Host ("  + {0,-5} extracted  {1,5} files  {2,10}  -> {3}" -f $item.id, $files.Count, (Format-HumanSize $size), $folderName) -ForegroundColor Green
        $rows.Add([pscustomobject]@{ Id = $item.id; Archive = $item.target; Folder = $folderName; Files = $files.Count; Status = 'extracted' })
    }
    catch {
        Write-Host ("  x {0,-5} extract failed: {1}" -f $item.id, $_.Exception.Message) -ForegroundColor Red
        $rows.Add([pscustomobject]@{ Id = $item.id; Archive = $item.target; Folder = $folderName; Files = 0; Status = "failed: $($_.Exception.Message)" })
        $failed++
    }
}

# log
$lines = New-Object System.Collections.Generic.List[string]
foreach ($r in $rows) {
    $lines.Add(@((Get-Date).ToString('s'), $r.Id, $r.Status, $r.Archive, $r.Folder, $r.Files, $r.Status) -join "`t")
}
Add-Content -LiteralPath $logPath -Value $lines -Encoding utf8

# EXTRACTED.md
$sb = New-Object System.Text.StringBuilder
[void]$sb.AppendLine('# Extracted archives')
[void]$sb.AppendLine('')
[void]$sb.AppendLine("Generated: $(Get-Date -Format 'yyyy-MM-dd HH:mm')  |  Source: type=zip items in ``scripts/library-manifest.json``")
[void]$sb.AppendLine('')
[void]$sb.AppendLine('| id | Archive | Extracted to | Files | Status |')
[void]$sb.AppendLine('|---|---|---|---|---|')
foreach ($r in $rows) {
    $dir = if ($r.Folder) { "``$($r.Folder)/``" } else { '-' }
    [void]$sb.AppendLine("| $($r.Id) | ``$($r.Archive)`` | $dir | $($r.Files) | $($r.Status) |")
}
Set-Content -LiteralPath (Join-Path $Root '10_code_tools\EXTRACTED.md') -Value $sb.ToString() -Encoding utf8

$rows | Format-Table -AutoSize Id, Status, Files, Folder | Out-String -Width 200 | Write-Host
Write-Host ("Extracted root: {0}" -f $destRoot)
if ($failed -gt 0) { exit 1 }
exit 0
