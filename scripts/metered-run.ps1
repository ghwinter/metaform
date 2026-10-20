<#
.SYNOPSIS
    Run one headless dsh task and write a single metrics JSON line.

.DESCRIPTION
    Resolve the API key, run `dsh --profile <Profile> --json "<task>"` in -Workdir, parse the
    newline-delimited event stream, and write only counts:

      {"task_id":"...","profile":"headless","exit":0,"elapsed_s":1.2,
       "steps":3,"tokens_in":123,"tokens_out":45,"complete":true}

    Key handling, in order: the environment variable named by -KeyEnv, then the file named by
    -KeyFile. A key file that resolves inside this repository is refused before it is read. A key
    read from a file is placed only in the child process environment and removed immediately after
    the child starts; the parent environment is never touched. The key is never printed, logged,
    or written to an error message, including under -Verbose.

    The parser reads the `--json` projection: token usage is attached to each committed assistant
    message. The current projection emits one `status` event with `phase:"step_end"` per step and
    carries StepUsage there, so that shape is accepted too. `assistant/message` events win when a
    stream carries them; otherwise `status/step_end` events are the steps. Usage field names may be
    camelCase or snake_case, and input/output may be named inputTokens/prompt_tokens/completionTokens
    and their snake_case forms.

    Exit codes: 0 the run succeeded and metrics were written; 1 the run failed, timed out, or could
    not start (metrics are still written); 2 usage, key resolution, or output-path error.

.PARAMETER Task
    The task text (mutually exclusive with -TaskFile).

.PARAMETER TaskFile
    A UTF-8 file whose text is the task (mutually exclusive with -Task).

.PARAMETER Workdir
    Working directory for dsh. Defaults to the current directory.

.PARAMETER Out
    Metrics JSON path. Defaults to tmp/metrics/<timestamp>.json under the repository root.

.PARAMETER Profile
    dsh profile. Defaults to headless.

.PARAMETER KeyEnv
    Environment variable that supplies the key. Defaults to DEEPSEEK_API_KEY.

.PARAMETER KeyFile
    Optional key file outside this repository. Used only when the environment variable is unset.

.PARAMETER TimeoutSec
    Seconds before the child is killed. Defaults to 600.

.PARAMETER Raw
    Optional path under the repository's tmp/ directory that keeps the raw JSONL stdout.

.PARAMETER JsonFixture
    Internal parser-test path: skip dsh and parse this JSONL file instead. The task text is still
    required so task_id can be computed. No key is resolved in this mode.
#>
[CmdletBinding()]
param(
    [string]$Task,
    [string]$TaskFile,
    [string]$Workdir = (Get-Location).Path,
    [string]$Out,
    [string]$Profile = 'headless',
    [string]$KeyEnv = 'DEEPSEEK_API_KEY',
    [string]$KeyFile,
    [int]$TimeoutSec = 600,
    [string]$Raw,
    [Parameter(DontShow = $true)]
    [string]$JsonFixture
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$invocationDir = (Get-Location).Path
$utf8 = [System.Text.UTF8Encoding]::new($false)

function Fail([string]$Message, [int]$Code = 2) {
    [Console]::Error.WriteLine("[metered-run] $Message")
    exit $Code
}

function Resolve-UserPath([string]$Path) {
    return $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
}

function Test-PathInside([string]$Path, [string]$Root) {
    $p = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $r = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    if ($p -eq $r) { return $true }
    return $p.StartsWith($r + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Get-PropertyValue($Object, [string[]]$Names) {
    if ($null -eq $Object) { return $null }
    foreach ($name in $Names) {
        $property = $Object.PSObject.Properties[$name]
        if ($null -ne $property) { return $property.Value }
    }
    return $null
}

function Get-UsageSample($Usage) {
    if ($null -eq $Usage) { return $null }
    $inNames = @('inputTokens', 'input_tokens', 'promptTokens', 'prompt_tokens', 'tokens_in', 'tokensIn', 'input')
    $outNames = @('outputTokens', 'output_tokens', 'completionTokens', 'completion_tokens', 'tokens_out', 'tokensOut', 'output')
    $tokensIn = Get-PropertyValue $Usage $inNames
    $tokensOut = Get-PropertyValue $Usage $outNames
    if ($null -eq $tokensIn -or $null -eq $tokensOut) { return $null }
    try {
        return [pscustomobject]@{ In = [long]$tokensIn; Out = [long]$tokensOut }
    }
    catch {
        return $null
    }
}

function Get-StreamUsage($Stream) {
    $last = $null
    foreach ($item in @($Stream)) {
        if ($null -eq $item) { continue }
        $kind = [string](Get-PropertyValue $item @('type'))
        if ($kind -eq 'chunk') {
            $chunk = Get-PropertyValue $item @('chunk')
            if ((Get-PropertyValue $chunk @('type')) -eq 'usage') {
                $last = Get-PropertyValue $chunk @('usage')
            }
        }
        elseif ($kind -eq 'usage') {
            $last = Get-PropertyValue $item @('usage')
        }
    }
    return $last
}

function Get-EventUsage($Event) {
    $data = Get-PropertyValue $Event @('data')
    $sample = Get-UsageSample (Get-PropertyValue $data @('usage'))
    if ($null -eq $sample) {
        $stream = Get-PropertyValue $data @('stream')
        if ($null -eq $stream) { $stream = Get-PropertyValue $Event @('stream') }
        $sample = Get-UsageSample (Get-StreamUsage $stream)
    }
    if ($null -eq $sample) {
        $sample = Get-UsageSample (Get-PropertyValue $Event @('usage'))
    }
    return $sample
}

function Read-JsonLines([string]$Path) {
    $assistant = [System.Collections.Generic.List[object]]::new()
    $stepEnds = [System.Collections.Generic.List[object]]::new()
    $malformed = 0
    foreach ($line in [IO.File]::ReadLines($Path)) {
        $text = $line.Trim()
        if ($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) { $text = $text.Substring(1) }
        if (-not $text) { continue }
        try {
            $event = $text | ConvertFrom-Json -ErrorAction Stop
        }
        catch {
            $malformed++
            continue
        }
        $kind = [string](Get-PropertyValue $event @('type'))
        if ($kind -eq 'assistant/message' -or $kind -eq 'assistant_message') {
            $assistant.Add((Get-EventUsage $event))
        }
        elseif ($kind -eq 'status' -and [string](Get-PropertyValue $event @('phase')) -eq 'step_end') {
            $stepEnds.Add((Get-EventUsage $event))
        }
    }
    return [pscustomobject]@{
        Assistant = $assistant
        StepEnds  = $stepEnds
        Malformed = $malformed
    }
}

function Measure-JsonLines([string]$Path) {
    $events = Read-JsonLines $Path
    # A raw Session stream carries assistant/message events; the current --json projection
    # carries the same step usage on status/step_end. Count exactly one vocabulary.
    $steps = if ($events.Assistant.Count -gt 0) { $events.Assistant } else { $events.StepEnds }
    $tokensIn = [long]0
    $tokensOut = [long]0
    $complete = ($events.Malformed -eq 0)
    foreach ($sample in $steps) {
        if ($null -eq $sample) {
            $complete = $false
            continue
        }
        $tokensIn += $sample.In
        $tokensOut += $sample.Out
    }
    return [pscustomobject]@{
        Steps     = $steps.Count
        TokensIn  = $tokensIn
        TokensOut = $tokensOut
        Complete  = $complete
    }
}

function Get-TaskId([string]$Text) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text))
    }
    finally {
        $sha.Dispose()
    }
    return -join ($hash[0..5] | ForEach-Object { $_.ToString('x2') })
}

if ([string]::IsNullOrWhiteSpace($KeyEnv)) { Fail '-KeyEnv must name an environment variable' }
if ($TimeoutSec -le 0) { Fail '-TimeoutSec must be positive' }

if ($Task -and $TaskFile) { Fail 'pass -Task or -TaskFile, not both' }
if (-not $Task -and -not $TaskFile) { Fail 'pass -Task or -TaskFile' }
if ($TaskFile) {
    $taskPath = Resolve-UserPath $TaskFile
    if (-not (Test-Path -LiteralPath $taskPath -PathType Leaf)) { Fail "task file not found: $taskPath" }
    $Task = [IO.File]::ReadAllText($taskPath, [Text.Encoding]::UTF8)
    if ($Task.Length -gt 0 -and $Task[0] -eq [char]0xFEFF) { $Task = $Task.Substring(1) }
}
if ([string]::IsNullOrWhiteSpace($Task)) { Fail 'the task text is empty' }

$workdirPath = Resolve-UserPath $Workdir
if (-not (Test-Path -LiteralPath $workdirPath -PathType Container)) { Fail "work directory not found: $workdirPath" }
$workdirPath = (Resolve-Path -LiteralPath $workdirPath).ProviderPath

if (-not $Out) {
    $Out = Join-Path $repoRoot ('tmp/metrics/' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.json')
}
else {
    $Out = Resolve-UserPath $Out
}
$outDir = Split-Path -Parent $Out
if ($outDir) { New-Item -ItemType Directory -Force -Path $outDir | Out-Null }

$rawPath = $null
if ($Raw) {
    $rawPath = Resolve-UserPath $Raw
    $tmpRoot = Join-Path $repoRoot 'tmp'
    if (-not (Test-PathInside $rawPath $tmpRoot)) {
        Fail '-Raw must be under the repository tmp/ directory'
    }
    if ([IO.Path]::GetFullPath($rawPath).TrimEnd('\', '/') -eq [IO.Path]::GetFullPath($tmpRoot).TrimEnd('\', '/')) {
        Fail '-Raw must name a file, not the tmp/ directory'
    }
    $rawDir = Split-Path -Parent $rawPath
    if ($rawDir) { New-Item -ItemType Directory -Force -Path $rawDir | Out-Null }
}

$taskId = Get-TaskId $Task
$exitCode = 0
$elapsed = [double]0
$steps = 0
$tokensIn = [long]0
$tokensOut = [long]0
$complete = $true
$runFailed = $false

$stdoutTemp = $null
$stderrTemp = $null
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()

if ($JsonFixture) {
    $fixturePath = Resolve-UserPath $JsonFixture
    if (-not (Test-Path -LiteralPath $fixturePath -PathType Leaf)) { Fail "JSON fixture not found: $fixturePath" }
    try {
        $measure = Measure-JsonLines $fixturePath
        $steps = [int]$measure.Steps
        $tokensIn = [long]$measure.TokensIn
        $tokensOut = [long]$measure.TokensOut
        $complete = [bool]$measure.Complete
        if ($rawPath) { Copy-Item -LiteralPath $fixturePath -Destination $rawPath -Force }
    }
    catch {
        Fail 'cannot parse the JSON fixture'
    }
    $stopwatch.Stop()
    $elapsed = [math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
}
else {
    # Resolve the key. A key file inside the repository is refused even when the environment
    # variable is set, so a repository path can never become the resolved key source.
    $keyFromFile = $false
    $key = $null
    if ($KeyFile) {
        $keyPath = Resolve-UserPath $KeyFile
        if (Test-PathInside $keyPath $repoRoot) {
            Fail 'refusing a key file inside the repository; keep the key outside the repository'
        }
        if (-not (Test-Path -LiteralPath $keyPath -PathType Leaf)) { Fail "key file not found: $keyPath" }
    }
    $envKey = [Environment]::GetEnvironmentVariable($KeyEnv, 'Process')
    if (-not [string]::IsNullOrWhiteSpace($envKey)) {
        $key = $envKey.Trim()
        if ($key.Length -gt 0 -and $key[0] -eq [char]0xFEFF) { $key = $key.Substring(1).Trim() }
        Write-Verbose "using the key from environment variable $KeyEnv"
    }
    elseif ($KeyFile) {
        $keyPath = Resolve-UserPath $KeyFile
        try {
            $key = ([IO.File]::ReadAllText($keyPath, [Text.Encoding]::UTF8)).Trim()
        }
        catch {
            Fail "cannot read the key file for $KeyEnv"
        }
        if ($key.Length -gt 0 -and $key[0] -eq [char]0xFEFF) { $key = $key.Substring(1).Trim() }
        if ([string]::IsNullOrWhiteSpace($key)) {
            Fail "no API key: $KeyEnv is unset and the key file is empty"
        }
        $keyFromFile = $true
        Write-Verbose "using the key from the external key file"
    }
    else {
        Fail "no API key: set environment variable $KeyEnv or pass -KeyFile"
    }

    $dshCommand = Get-Command dsh -ErrorAction SilentlyContinue | Select-Object -First 1
    $dshPath = if ($dshCommand -and $dshCommand.Source) { $dshCommand.Source } else { 'dsh' }

    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $psi.WorkingDirectory = $workdirPath
    $psi.StandardOutputEncoding = $utf8
    $psi.StandardErrorEncoding = $utf8
    $psi.FileName = $dshPath

    if ([IO.Path]::GetExtension($dshPath) -ieq '.ps1') {
        $pwshPath = (Get-Command pwsh -ErrorAction SilentlyContinue | Select-Object -First 1).Source
        if (-not $pwshPath) { $pwshPath = 'pwsh' }
        $psi.FileName = $pwshPath
        [void]$psi.ArgumentList.Add('-NoProfile')
        [void]$psi.ArgumentList.Add('-File')
        [void]$psi.ArgumentList.Add($dshPath)
    }
    [void]$psi.ArgumentList.Add('--profile')
    [void]$psi.ArgumentList.Add($Profile)
    [void]$psi.ArgumentList.Add('--json')
    [void]$psi.ArgumentList.Add($Task)

    $tmpRoot = Join-Path $repoRoot 'tmp'
    New-Item -ItemType Directory -Force -Path $tmpRoot | Out-Null
    $runId = [guid]::NewGuid().ToString('N')
    $stdoutTemp = Join-Path $tmpRoot "metered-$runId.out.jsonl"
    $stderrTemp = Join-Path $tmpRoot "metered-$runId.err.txt"
    $null = New-Item -ItemType File -Force -Path $stdoutTemp, $stderrTemp

    try {
        if ($keyFromFile) {
            # ProcessStartInfo.Environment is the child's block. The parent $env: block is never
            # assigned, and the entry is removed from the start info after the child is running.
            $psi.Environment[$KeyEnv] = $key
        }
        $process = $null
        $startFailed = $false
        try {
            $process = [System.Diagnostics.Process]::Start($psi)
        }
        catch {
            $startFailed = $true
            $runFailed = $true
            $exitCode = 127
            $complete = $false
        }
        if ($keyFromFile) {
            [void]$psi.Environment.Remove($KeyEnv)
        }
        $key = $null

        $stdoutText = ''
        $stderrText = ''
        if (-not $startFailed) {
            $stdoutTask = $process.StandardOutput.ReadToEndAsync()
            $stderrTask = $process.StandardError.ReadToEndAsync()
            $finished = $process.WaitForExit($TimeoutSec * 1000)
            if (-not $finished) {
                $runFailed = $true
                $exitCode = 124
                $complete = $false
                try { $process.Kill($true) } catch { }
                try { [void]$process.WaitForExit(5000) } catch { }
            }
            else {
                $exitCode = [int]$process.ExitCode
                if ($exitCode -ne 0) {
                    $runFailed = $true
                    $complete = $false
                }
            }
            try { $stdoutText = $stdoutTask.GetAwaiter().GetResult() } catch { $stdoutText = '' }
            try { $stderrText = $stderrTask.GetAwaiter().GetResult() } catch { $stderrText = '' }
        }

        [IO.File]::WriteAllText($stdoutTemp, $stdoutText, $utf8)
        [IO.File]::WriteAllText($stderrTemp, $stderrText, $utf8)
        if ($rawPath) { Copy-Item -LiteralPath $stdoutTemp -Destination $rawPath -Force }

        try {
            $measure = Measure-JsonLines $stdoutTemp
            $steps = [int]$measure.Steps
            $tokensIn = [long]$measure.TokensIn
            $tokensOut = [long]$measure.TokensOut
            if (-not $measure.Complete) { $complete = $false }
        }
        catch {
            $runFailed = $true
            $complete = $false
        }
    }
    finally {
        if ($stdoutTemp -and (Test-Path -LiteralPath $stdoutTemp)) {
            Remove-Item -LiteralPath $stdoutTemp -Force -ErrorAction SilentlyContinue
        }
        if ($stderrTemp -and (Test-Path -LiteralPath $stderrTemp)) {
            Remove-Item -LiteralPath $stderrTemp -Force -ErrorAction SilentlyContinue
        }
        $key = $null
    }
    $stopwatch.Stop()
    $elapsed = [math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
}

$metrics = [ordered]@{
    task_id   = $taskId
    profile   = $Profile
    exit      = [int]$exitCode
    elapsed_s = [double]$elapsed
    steps     = [int]$steps
    tokens_in = [long]$tokensIn
    tokens_out = [long]$tokensOut
    complete  = [bool]$complete
}
$json = ConvertTo-Json -InputObject $metrics -Compress
try {
    [IO.File]::WriteAllText($Out, $json, $utf8)
}
catch {
    Fail "cannot write metrics to $Out"
}
[Console]::Out.WriteLine($json)
if ($runFailed) { exit 1 }
exit 0
