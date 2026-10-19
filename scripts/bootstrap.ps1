<#
.SYNOPSIS
    Check this machine against the repository's environment declaration, or write the local file the
    gates source.

.DESCRIPTION
    environment.json declares what the gates need: which capability, which environment variable, which
    minimum version. It does not say where those live on this disk, because a path is a fact about one
    machine and a version is a fact about the results.

    Default mode reports what was found and what is missing. -Write writes scripts/local.ps1 from the
    values it observed and leaves unresolved requirements out rather than guessing. -Print prints the
    same information as instructions, for an agent reading the repository.

    Exit codes:
      0  every required capability is present
      1  something required is missing
      2  the declaration could not be read

.PARAMETER Write
    Write scripts/local.ps1 from the observed values.

.PARAMETER Print
    Print instructions instead of a status table.
#>
[CmdletBinding()]
param(
    [switch]$Write,
    [switch]$Print
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$declarationPath = Join-Path $root 'environment.json'
$local = Join-Path $PSScriptRoot 'local.ps1'
if (Test-Path $local) { . $local }

if (-not (Test-Path $declarationPath)) {
    Write-Host "[bootstrap] declaration not found: environment.json"
    exit 2
}
try {
    $declaration = Get-Content -Raw -Encoding UTF8 $declarationPath | ConvertFrom-Json
}
catch {
    Write-Host "[bootstrap] environment.json is not valid JSON: $($_.Exception.Message)"
    exit 2
}

function Test-VersionAtLeast([string]$Have, [string]$Need) {
    $h = @($Have -split '\.' | ForEach-Object { ($_ -replace '\D', '') } | Where-Object { $_ -ne '' } | ForEach-Object { [int]$_ })
    $n = @($Need -split '\.' | ForEach-Object { ($_ -replace '\D', '') } | Where-Object { $_ -ne '' } | ForEach-Object { [int]$_ })
    for ($i = 0; $i -lt [Math]::Max($h.Count, $n.Count); $i++) {
        $a = if ($i -lt $h.Count) { $h[$i] } else { 0 }
        $b = if ($i -lt $n.Count) { $n[$i] } else { 0 }
        if ($a -gt $b) { return $true }
        if ($a -lt $b) { return $false }
    }
    return $true
}

$rows = @()
foreach ($name in $declaration.requirements.PSObject.Properties.Name) {
    $req = $declaration.requirements.$name
    $value = [Environment]::GetEnvironmentVariable($req.env)
    $location = $null
    $detail = ''
    $note = ''

    switch ($name) {
        'python' {
            if (-not $value) {
                $cmd = Get-Command python -ErrorAction SilentlyContinue
                if ($cmd) { $value = $cmd.Source }
            }
            if ($value -and (Test-Path $value)) {
                $location = $value
                try { $detail = (& $value @($req.versionArgs) 2>$null | Select-Object -First 1) }
                catch { $detail = '' }
                if ($req.minVersion -and $detail -and -not (Test-VersionAtLeast $detail $req.minVersion)) {
                    $note = "below $($req.minVersion)"
                    $location = $null
                }
            }
        }
        default {
            if ($value -and (Test-Path $value)) {
                $location = $value
                $detail = $value
                if ($req.versionExe) {
                    Push-Location $value
                    try {
                        $line = (& $req.versionExe @($req.versionArgs) 2>$null | Select-Object -First 1)
                    }
                    catch { $line = $null }
                    finally { Pop-Location }
                    $m = [regex]::Match([string]$line, '\d+\.\d+(?:\.\d+)?')
                    if ($m.Success) {
                        $detail = "$value (Lean $($m.Value))"
                        if ($req.minVersion -and -not (Test-VersionAtLeast $m.Value $req.minVersion)) {
                            $note = "below $($req.minVersion)"
                            $location = $null
                        }
                    }
                    else {
                        $note = "cannot read the version"
                    }
                }
            }
        }
    }

    $rows += [pscustomobject]@{
        Requirement = $name
        Env         = $req.env
        Required    = [bool]$req.required
        Status      = if ($location) { 'ok' } else { 'missing' }
        Value       = if ($detail) { $detail } else { '' }
        Path        = if ($location) { $location } else { '' }
        Note        = $note
    }
}

if ($Print) {
    Write-Host "# environment for this repository"
    Write-Host "# declaration: environment.json"
    Write-Host ""
    foreach ($r in $rows) {
        $req = $declaration.requirements.($r.Requirement)
        Write-Host ("[{0}] {1} -> {2}" -f $r.Status, $r.Requirement, $r.Env)
        Write-Host ("      needs: {0}" -f $req.use)
        if ($r.Status -eq 'ok') { Write-Host ("      have:  {0}" -f $r.Value) }
        else { Write-Host ("      set:   {0}" -f $req.hint) }
    }
    Write-Host ""
    Write-Host "Then write the local file the gates source:"
    Write-Host "  pwsh -File scripts/bootstrap.ps1 -Write"
    exit 0
}

if ($Write) {
    $lines = @(
        '# Generated by scripts/bootstrap.ps1 from environment.json.',
        '# Not tracked: these are facts about one machine, not about the repository.',
        ''
    )
    foreach ($r in $rows) {
        if ($r.Status -eq 'ok') {
            $lines += ("`$env:{0} = '{1}'" -f $r.Env, $r.Path)
        }
        else {
            $lines += ("# unmet: {0} = '<{1}>'" -f $r.Env, ($declaration.requirements.($r.Requirement).hint))
        }
    }
    Set-Content -Path $local -Value ($lines -join "`n") -Encoding UTF8
    Write-Host "[bootstrap] wrote $($declaration.generated)"
}

Write-Host ''
Write-Host '=== environment ==='
foreach ($r in $rows) {
    $flag = if ($r.Required) { 'required' } else { 'optional' }
    Write-Host ("  {0,-16} {1,-10} {2,-8} {3}" -f $r.Requirement, $flag, $r.Status, $r.Value)
    if ($r.Note) { Write-Host ("  {0,-16} {1}" -f '', $r.Note) }
}

$missing = $rows | Where-Object { $_.Required -and $_.Status -ne 'ok' }
if ($missing) {
    Write-Host ''
    foreach ($m in $missing) {
        $req = $declaration.requirements.($m.Requirement)
        Write-Host ("[bootstrap] {0}: {1}" -f $m.Env, $req.hint)
    }
    Write-Host "[bootstrap] then run: pwsh -File scripts/bootstrap.ps1 -Write"
    exit 1
}
Write-Host ''
Write-Host '[bootstrap] every required capability is present'
exit 0
