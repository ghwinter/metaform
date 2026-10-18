<#
.SYNOPSIS
    Build a publishable tool bundle: exe + skill + README + VERSION.

.DESCRIPTION
    Unlike make-bundle (the paper/research archive), this bundle is meant "for other projects":
    unzip it and you get an executable plus its usage docs.

    After packing, it reopens the zip and checks every entry's size; a mismatch fails.
#>
[CmdletBinding()]
param(
    [string]$Root = (Split-Path $PSScriptRoot -Parent),
    [string]$OutDir = (Join-Path (Split-Path $PSScriptRoot -Parent) 'dist')
)

$ErrorActionPreference = 'Stop'

$cargo = Get-Content (Join-Path $Root 'engine\Cargo.toml') -Raw
$version = ([regex]::Match($cargo, 'version\s*=\s*"([^"]+)"')).Groups[1].Value
if (-not $version) { throw 'cannot read version' }

Push-Location (Join-Path $Root 'engine')
& cargo build --release --quiet
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { throw 'cargo build --release failed' }

$stage = Join-Path $OutDir 'release-stage'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage | Out-Null

$files = @{
    'metaform.exe'                 = 'engine\target\release\metaform.exe'
    'SKILL.md'                     = 'skills\metaform\SKILL.md'
    'README.md'                    = 'README.md'
}
foreach ($dst in $files.Keys) {
    $src = Join-Path $Root $files[$dst]
    if (-not (Test-Path $src)) { throw "missing file: $($files[$dst])" }
    Copy-Item $src (Join-Path $stage $dst) -Force
}

@(
    "metaform $version"
    "platform: windows-x64"
    "built: $(Get-Date -Format 'yyyy-MM-dd HH:mm')"
    "usage: metaform --help | metaform mcp"
    "docs: SKILL.md"
) | Set-Content (Join-Path $stage 'VERSION.txt') -Encoding utf8

$sizes = @{}
foreach ($f in Get-ChildItem $stage -File) { $sizes[$f.Name] = $f.Length }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$zip = Join-Path $OutDir ("metaform-{0}-win64.zip" -f $version)
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
    $bad = @()
    foreach ($e in $archive.Entries) {
        $name = $e.FullName -replace '\\', '/'
        if (-not $sizes.ContainsKey($name) -or $sizes[$name] -ne $e.Length) {
            $bad += $name
        }
    }
    if ($bad.Count -gt 0) { throw "bundle contents do not match source: $($bad -join ', ')" }
    Write-Host ("[release] {0}  entries={1}  size={2} KB" -f $zip, $archive.Entries.Count, [math]::Round((Get-Item $zip).Length / 1KB, 1))
}
finally {
    $archive.Dispose()
    Remove-Item $stage -Recurse -Force
}
exit 0
