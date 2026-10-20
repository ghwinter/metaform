# Install the MCP server to a stable prefix, prove the wire, and register it in the host profiles.
#
# The install path is deliberately outside the repository: target\debug is a build artifact that
# cargo clean removes, and a host that starts a missing binary fails at session start, not here.
# Run from the repository root. The last step edits each profile's cordis.patch.yml in place, so
# no entry has to be copied by hand; -NoRegister stops after the probe, -DryRun prints the plan.
#
# Why the registration is nested under insert: a profile patch file is a top-level YAML array of
# loader patches. An insert appends entries; any other patch is matched against the ids already
# in the tree, so a bare "- id: mcp-metaform" targets a row that does not exist yet and is
# warned and skipped. See config-schema/document.ts and the include patch applier in the host.
[CmdletBinding()]
param(
    [switch]$NoRegister,
    [switch]$DryRun,
    [string]$Profile,
    [string]$DshHome
)

$ErrorActionPreference = 'Stop'

$prefix = Join-Path $env:LOCALAPPDATA 'metaform'
cargo install --path engine --root $prefix --locked --force
if ($LASTEXITCODE -ne 0) { throw "cargo install failed with $LASTEXITCODE" }

$exe = Join-Path $prefix 'bin\metaform.exe'
if (-not (Test-Path $exe)) { throw "no binary at $exe" }

$version = & $exe --version
if ($LASTEXITCODE -ne 0) { throw "metaform --version failed with $LASTEXITCODE" }
Write-Host "installed: $version at $exe"

# The transcript a host sends: initialize, then ask for the tool list. A server that fails either
# step is a server that starts and does nothing, which is worse than one that refuses to start.
$transcript = @(
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"install-check","version":"1"}}}',
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
)
$replies = $transcript | & $exe mcp
if ($LASTEXITCODE -ne 0) { throw "the server exited with $LASTEXITCODE" }

$init = $replies[0] | ConvertFrom-Json
$tools = $replies[1] | ConvertFrom-Json
if ($init.result.serverInfo.name -ne 'metaform') { throw "unexpected serverInfo: $($replies[0])" }
$count = $tools.result.tools.Count
if ($count -ne 10) { throw "expected 10 tools, got $count" }
Write-Host "handshake ok: serverInfo=$($init.result.protocolVersion) tools=$count"

# ---------------------------------------------------------------------------
# Registration into the host profile patch layers.
#
# The edit is deliberately line-oriented: it must touch one insert block and leave every other
# byte of the file alone, including comments, key order and quoting the user chose. A full YAML
# round-trip would reformat the whole file.
# ---------------------------------------------------------------------------

$entryId = 'mcp-metaform'
$pluginName = '@deepseek-ai/dsh-mcp-client'
$patchFileName = 'cordis.patch.yml'
$backupSuffix = '.bak-mcp'
$script:BackedUp = [System.Collections.Generic.HashSet[string]]::new()

# An entry line this script can reason about: "- id: mcp-metaform" (quoted or not, no other keys).
$script:EntryIdPattern = [regex]'^(?<pad> *)- +id: *(?<q>[''"]?)mcp-metaform\k<q> *(?:#.*)?$'
$script:EntryIdMention = [regex]'mcp-metaform'
$script:InsertPattern = [regex]'^- +insert: *(?:#.*)?$'

function ConvertTo-YamlString {
    param([string]$Value)
    # Single quotes are literal in YAML; a quote inside is doubled. Windows paths need no escaping.
    return "'" + $Value.Replace("'", "''") + "'"
}

function Get-ConfigLines {
    param([int]$Indent, [string]$Command, [string]$Cwd)
    $pad = ' ' * $Indent
    return @(
        $pad + 'config:'
        $pad + '  serverName: metaform'
        $pad + '  transport: stdio'
        $pad + '  command: ' + (ConvertTo-YamlString $Command)
        $pad + '  args:'
        $pad + '    - mcp'
        $pad + '  cwd: ' + (ConvertTo-YamlString $Cwd)
        $pad + '  toolCallTimeoutMs: 60000'
        $pad + '  failOnStartupError: true'
    )
}

function Get-RegistrationBlock {
    param([string]$Command, [string]$Cwd)
    return @(
        '- insert:'
        '    - id: ' + $entryId
        '      name: "' + $pluginName + '"'
    ) + (Get-ConfigLines -Indent 6 -Command $Command -Cwd $Cwd)
}

function Read-PatchFile {
    param([string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -ge 2 -and (($bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE) -or ($bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF))) {
        throw "$Path is UTF-16; the host reads patch files as UTF-8"
    }
    $hasBom = $bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF
    $text = [System.Text.Encoding]::UTF8.GetString($bytes)
    if ($hasBom) { $text = $text.Substring(1) }
    return [pscustomobject]@{ Text = $text; HasBom = $hasBom }
}

function Write-PatchFile {
    param([string]$Path, [string]$Text, [bool]$HasBom)
    $encoding = [System.Text.UTF8Encoding]::new($HasBom)
    [System.IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Split-PatchText {
    param([string]$Text)
    if ($Text.Length -eq 0) {
        return [pscustomobject]@{ Lines = [string[]]@(); Newline = "`n"; Trailing = $true }
    }
    $newline = if ($Text.Contains("`r`n")) { "`r`n" } else { "`n" }
    $trailing = $Text.EndsWith("`n")
    $body = if ($trailing) { $Text.Substring(0, $Text.Length - 1) } else { $Text }
    $lines = $body.Split("`n")
    if ($newline -eq "`r`n") {
        for ($i = 0; $i -lt $lines.Length; $i++) {
            if ($lines[$i].EndsWith("`r")) { $lines[$i] = $lines[$i].Substring(0, $lines[$i].Length - 1) }
        }
    }
    return [pscustomobject]@{ Lines = $lines; Newline = $newline; Trailing = $trailing }
}

function Join-PatchText {
    param($Lines, [string]$Newline, [bool]$Trailing)
    $text = [string]::Join($Newline, [string[]]$Lines)
    if ($Trailing -and $Lines.Count -gt 0) { $text += $Newline }
    return $text
}

function Get-Indent {
    param([string]$Line)
    return ([regex]::Match($Line, '^ *')).Length
}

function Test-KeyLine {
    param([string]$Line, [int]$Indent, [string]$Key)
    return ($Line.Trim().Length -gt 0) -and ((Get-Indent $Line) -eq $Indent) -and $Line.TrimStart().StartsWith($Key + ':')
}

function Get-BlockEnd {
    # The last line of an entry that starts at $Start and is indented deeper than $DashIndent.
    param($Lines, [int]$Start, [int]$DashIndent)
    $end = $Start
    for ($i = $Start + 1; $i -lt $Lines.Count; $i++) {
        if ($Lines[$i].Trim().Length -eq 0) { continue }
        if ((Get-Indent $Lines[$i]) -le $DashIndent) { break }
        $end = $i
    }
    return $end
}

function Find-EntryLine {
    param($Lines)
    $found = @()
    for ($i = 0; $i -lt $Lines.Count; $i++) {
        if ($Lines[$i].TrimStart().StartsWith('#')) { continue }
        if ($script:EntryIdPattern.IsMatch($Lines[$i])) { $found += $i }
    }
    return $found
}

function Test-EntryMention {
    param($Lines)
    for ($i = 0; $i -lt $Lines.Count; $i++) {
        if ($Lines[$i].TrimStart().StartsWith('#')) { continue }
        if ($script:EntryIdMention.IsMatch($Lines[$i])) { return $true }
    }
    return $false
}

function Get-EntryBlockText {
    # The insert block that now carries the entry, for the run report.
    param($Lines)
    $ids = @(Find-EntryLine -Lines $Lines)
    if ($ids.Count -ne 1) { return @() }
    $start = $ids[0]
    $dashIndent = Get-Indent $Lines[$start]
    $blockStart = $start
    for ($i = $start - 1; $i -ge 0; $i--) {
        if ($Lines[$i].Trim().Length -eq 0) { continue }
        if ((Get-Indent $Lines[$i]) -lt $dashIndent) { $blockStart = $i; break }
    }
    $end = Get-BlockEnd -Lines $Lines -Start $start -DashIndent $dashIndent
    $out = @()
    for ($i = $blockStart; $i -le $end; $i++) { $out += $Lines[$i] }
    return $out
}

function Get-PatchEdit {
    param([string]$Path, [string]$Command, [string]$Cwd)

    $file = Read-PatchFile -Path $Path
    $doc = Split-PatchText -Text $file.Text
    $lines = [System.Collections.Generic.List[string]]::new()
    foreach ($line in $doc.Lines) { $lines.Add($line) }
    $newline = $doc.Newline
    $trailing = $doc.Trailing
    $result = {
        param([string]$Action, [string]$Note = '')
        [pscustomobject]@{
            Action = $Action
            Note = $Note
            Text = (Join-PatchText -Lines $lines -Newline $newline -Trailing $trailing)
            HasBom = $file.HasBom
            Entry = @(Get-EntryBlockText -Lines $lines)
        }
    }

    # What the file already contains, ignoring blank lines and comments.
    $meaningful = @()
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $trimmed = $lines[$i].Trim()
        if ($trimmed.Length -eq 0 -or $trimmed.StartsWith('#')) { continue }
        $meaningful += [pscustomobject]@{ Index = $i; Text = $trimmed }
    }

    if ($meaningful.Count -eq 0) {
        # Empty patch layer: write the block as the whole array.
        $lines.Clear()
        foreach ($line in (Get-RegistrationBlock -Command $Command -Cwd $Cwd)) { $lines.Add($line) }
        $trailing = $true
        return (& $result 'inserted')
    }

    if ($meaningful.Count -eq 1 -and $meaningful[0].Text -match '^\[\] *(?:#.*)?$') {
        # The template's "[]": replace the empty flow array with the block.
        $block = @(Get-RegistrationBlock -Command $Command -Cwd $Cwd)
        $lines.RemoveAt($meaningful[0].Index)
        $lines.InsertRange($meaningful[0].Index, [string[]]$block)
        return (& $result 'inserted')
    }

    if (-not $meaningful[0].Text.StartsWith('-')) {
        return (& $result 'skipped' 'the file is not a top-level YAML array of patch entries')
    }

    $ids = @(Find-EntryLine -Lines $lines)

    if ($ids.Count -gt 1) {
        return (& $result 'skipped' "found $($ids.Count) entries with id $entryId; edit the file by hand so exactly one remains")
    }

    if ($ids.Count -eq 0) {
        if (Test-EntryMention -Lines $lines) {
            return (& $result 'skipped' "the file mentions $entryId in a form this script cannot edit; edit it by hand")
        }
        # Append one top-level insert block, separated from the previous item by a blank line.
        if ($lines.Count -gt 0 -and $lines[$lines.Count - 1].Trim().Length -ne 0) { $lines.Add('') }
        foreach ($line in (Get-RegistrationBlock -Command $Command -Cwd $Cwd)) { $lines.Add($line) }
        $trailing = $true
        return (& $result 'inserted')
    }

    $start = $ids[0]
    $dashIndent = Get-Indent $lines[$start]
    $note = ''

    if ($dashIndent -eq 0) {
        # The previous attempt's shape: a bare top-level patch. It targets a row that does not
        # exist, so the host warns and skips it. Convert it into an insert, keeping its fields.
        $end = Get-BlockEnd -Lines $lines -Start $start -DashIndent $dashIndent
        $converted = [System.Collections.Generic.List[string]]::new()
        $converted.Add('- insert:')
        for ($i = $start; $i -le $end; $i++) {
            if ($lines[$i].Trim().Length -eq 0) { $converted.Add('') }
            else { $converted.Add('    ' + $lines[$i]) }
        }
        $lines.RemoveRange($start, $end - $start + 1)
        $lines.InsertRange($start, $converted)
        $start = $start + 1
        $dashIndent = 4
        $note = 'converted an inert top-level patch into an insert'
    }

    # The entry must live inside an insert list; anything else repeats the bug above.
    $insertIndex = -1
    for ($i = $start - 1; $i -ge 0; $i--) {
        if ($lines[$i].Trim().Length -eq 0) { continue }
        if ((Get-Indent $lines[$i]) -lt $dashIndent) { $insertIndex = $i; break }
    }
    if ($insertIndex -lt 0 -or -not $script:InsertPattern.IsMatch($lines[$insertIndex].Trim())) {
        return (& $result 'skipped' "the $entryId entry is not inside an insert list")
    }

    $keyIndent = $dashIndent + 2
    $end = Get-BlockEnd -Lines $lines -Start $start -DashIndent $dashIndent

    $nameIndex = -1
    for ($i = $start + 1; $i -le $end; $i++) {
        if (Test-KeyLine -Line $lines[$i] -Indent $keyIndent -Key 'name') { $nameIndex = $i; break }
    }
    if ($nameIndex -ge 0) {
        $value = $lines[$nameIndex].TrimStart().Substring('name:'.Length).Trim().Trim('"').Trim("'")
        if ($value -ne $pluginName) {
            return (& $result 'skipped' "entry $entryId names '$value', not $pluginName")
        }
    }
    else {
        $lines.Insert($start + 1, (' ' * $keyIndent) + 'name: "' + $pluginName + '"')
        $end++
    }

    $configIndex = -1
    for ($i = $start + 1; $i -le $end; $i++) {
        if (Test-KeyLine -Line $lines[$i] -Indent $keyIndent -Key 'config') { $configIndex = $i; break }
    }

    if ($configIndex -lt 0) {
        # No config yet: append the full one after the entry's other keys.
        $block = @(Get-ConfigLines -Indent $keyIndent -Command $Command -Cwd $Cwd)
        $lines.InsertRange($end + 1, [string[]]$block)
    }
    else {
        $configEnd = $configIndex
        $childIndent = -1
        for ($i = $configIndex + 1; $i -le $end; $i++) {
            if ($lines[$i].Trim().Length -eq 0) { continue }
            $ind = Get-Indent $lines[$i]
            if ($ind -le $keyIndent) { break }
            if ($childIndent -lt 0) { $childIndent = $ind }
            $configEnd = $i
        }

        if ($childIndent -lt 0) {
            # "config:" with no block body, or "config: {}": replace it with the full block.
            $block = @(Get-ConfigLines -Indent $keyIndent -Command $Command -Cwd $Cwd)
            $lines.RemoveAt($configIndex)
            $lines.InsertRange($configIndex, [string[]]$block)
        }
        else {
            $commandIndex = -1
            $cwdIndex = -1
            for ($i = $configIndex + 1; $i -le $configEnd; $i++) {
                if ($commandIndex -lt 0 -and (Test-KeyLine -Line $lines[$i] -Indent $childIndent -Key 'command')) { $commandIndex = $i }
                if ($cwdIndex -lt 0 -and (Test-KeyLine -Line $lines[$i] -Indent $childIndent -Key 'cwd')) { $cwdIndex = $i }
            }
            $missing = @()
            if ($commandIndex -ge 0) {
                $lines[$commandIndex] = (' ' * $childIndent) + 'command: ' + (ConvertTo-YamlString $Command)
            }
            else { $missing += (' ' * $childIndent) + 'command: ' + (ConvertTo-YamlString $Command) }
            if ($cwdIndex -ge 0) {
                $lines[$cwdIndex] = (' ' * $childIndent) + 'cwd: ' + (ConvertTo-YamlString $Cwd)
            }
            else { $missing += (' ' * $childIndent) + 'cwd: ' + (ConvertTo-YamlString $Cwd) }
            if ($missing.Count -gt 0) { $lines.InsertRange($configEnd + 1, [string[]]$missing) }
        }
    }

    $newText = Join-PatchText -Lines $lines -Newline $newline -Trailing $trailing
    if ($newText -eq $file.Text) { return (& $result 'unchanged') }
    return (& $result 'updated' $note)
}

# ---------------------------------------------------------------------------
# Run it.
# ---------------------------------------------------------------------------

$command = $exe
$cwd = (Get-Location).Path

if (-not $DshHome) {
    if (-not $env:USERPROFILE) { throw 'USERPROFILE is empty; pass -DshHome' }
    $DshHome = Join-Path $env:USERPROFILE '.dsh'
}
$profilesRoot = Join-Path $DshHome 'profiles'

if ($NoRegister) {
    Write-Host ''
    Write-Host 'Registration skipped (-NoRegister). To register by hand, add this block as a top-level'
    Write-Host 'item of a profile patch array (the file is cordis.patch.yml in the profile directory):'
    Write-Host ''
    foreach ($line in (Get-RegistrationBlock -Command $command -Cwd $cwd)) { Write-Host $line }
    exit 0
}

$patchFiles = @()
if ($Profile) {
    if ($Profile.Contains('\') -or $Profile.Contains('/')) {
        if (-not (Test-Path -LiteralPath $Profile -PathType Container)) { throw "no profile directory at $Profile" }
        $dir = (Resolve-Path -LiteralPath $Profile).Path
    }
    else {
        $dir = Join-Path $profilesRoot $Profile
        if (-not (Test-Path -LiteralPath $dir -PathType Container)) {
            throw "no profile named $Profile under $profilesRoot"
        }
    }
    $file = Join-Path $dir $patchFileName
    if (-not (Test-Path -LiteralPath $file)) { throw "no $patchFileName in $dir" }
    $patchFiles = @($file)
}
else {
    if (-not (Test-Path -LiteralPath $profilesRoot -PathType Container)) {
        Write-Warning "no profiles directory at $profilesRoot; nothing to register"
    }
    else {
        $patchFiles = @(Get-ChildItem -LiteralPath $profilesRoot -Directory | ForEach-Object {
            $candidate = Join-Path $_.FullName $patchFileName
            if (Test-Path -LiteralPath $candidate -PathType Leaf) { $candidate }
        })
        if ($patchFiles.Count -eq 0) {
            Write-Warning "no profile under $profilesRoot has $patchFileName; nothing to register"
        }
    }
}

$inserted = 0
$updated = 0
$unchanged = 0
$skipped = 0
$written = 0

foreach ($file in $patchFiles) {
    $edit = Get-PatchEdit -Path $file -Command $command -Cwd $cwd

    Write-Host ''
    Write-Host "profile: $file"

    if ($edit.Action -eq 'skipped') {
        $skipped++
        Write-Warning "not registered: $($edit.Note)"
        continue
    }

    Write-Host "action: $($edit.Action)"
    if ($edit.Note) { Write-Host "note: $($edit.Note)" }
    Write-Host 'entry:'
    foreach ($line in $edit.Entry) { Write-Host "  $line" }

    if ($edit.Action -eq 'unchanged') { $unchanged++; continue }
    if ($edit.Action -eq 'inserted') { $inserted++ }
    else { $updated++ }

    if ($DryRun) {
        Write-Host 'write: skipped (dry run)'
        continue
    }

    # One backup per file per run, and never over an existing backup: an older .bak-mcp keeps
    # the state before the first registration, which is the copy worth restoring.
    $backup = $file + $backupSuffix
    if ($script:BackedUp.Contains($file)) {
        $backup = $null
    }
    elseif (Test-Path -LiteralPath $backup) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $backup = $file + $backupSuffix + '-' + $stamp
        $suffix = 1
        while (Test-Path -LiteralPath $backup) {
            $backup = $file + $backupSuffix + '-' + $stamp + '-' + $suffix
            $suffix++
        }
    }
    if ($backup) {
        [System.IO.File]::Copy($file, $backup, $false)
        $script:BackedUp.Add($file) | Out-Null
        Write-Host "backup: $backup"
    }
    Write-PatchFile -Path $file -Text $edit.Text -HasBom $edit.HasBom
    $written++
}

Write-Host ''
if ($patchFiles.Count -eq 0) {
    Write-Host 'No profile was registered. Add this block as a top-level item of a profile patch array:'
    Write-Host ''
    foreach ($line in (Get-RegistrationBlock -Command $command -Cwd $cwd)) { Write-Host $line }
    exit 0
}
if ($DryRun) {
    Write-Host "dry run: $($patchFiles.Count) profile(s) inspected, nothing written"
    Write-Host "plan: $inserted inserted, $updated updated, $unchanged unchanged, $skipped skipped"
}
elseif ($written -eq 0) {
    Write-Host "profiles: $($patchFiles.Count) inspected, $unchanged unchanged, $skipped skipped; nothing written"
}
else {
    Write-Host "profiles: $($patchFiles.Count) inspected, $written written ($inserted inserted, $updated updated), $unchanged unchanged, $skipped skipped"
    Write-Host 'Restart the host to load the registration.'
}
if ($skipped -gt 0) { exit 1 }
exit 0
