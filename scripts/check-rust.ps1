<#
.SYNOPSIS
    Run the Rust tests and assert "passed == expected, 0 failed, 0 ignored".

.DESCRIPTION
    `cargo test` returns non-zero only when a test fails. If a test is #[ignore]d, or an entire
    test module is cfg'd out, the output still looks green. So this pins the numbers: passed must
    equal -Expected, and ignored must be 0.

    The log is written to engine/last-test.log.

    Exit codes:
      0  the pass count matches the expectation, 0 failed, 0 ignored
      1  not satisfied

.PARAMETER Expected
    Expected pass count (default 115: 70 kernel + 28 CLI/ledger/scenario/MCP/plan/propose/report/score +
    1 propose integration test + 3 state integration tests + 9 suggest integration tests +
    2 new_atom_loop integration tests + 2 ask_loop integration tests).

.PARAMETER Manifest
    Cargo.toml path.
#>
[CmdletBinding()]
param(
    [int]$Expected = 115,
    [string]$Manifest = (Join-Path $PSScriptRoot '..\engine\Cargo.toml')
)

$ErrorActionPreference = 'Stop'
$manifestPath = (Resolve-Path $Manifest).Path
$logPath = Join-Path (Split-Path $manifestPath -Parent) 'last-test.log'

$sw = [Diagnostics.Stopwatch]::StartNew()
$out = & cargo test --manifest-path $manifestPath 2>&1 | Out-String
$code = $LASTEXITCODE
$secs = [math]::Round($sw.Elapsed.TotalSeconds, 1)

$header = "# check-rust run: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')  exit=$code  secs=$secs`n"
($header + $out) | Set-Content -Path $logPath -Encoding utf8

$passed = 0; $failed = 0; $ignored = 0; $suites = 0
foreach ($m in [regex]::Matches($out, 'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored')) {
    $suites++
    $passed += [int]$m.Groups[1].Value
    $failed += [int]$m.Groups[2].Value
    $ignored += [int]$m.Groups[3].Value
}

Write-Host ("[check-rust] exit={0}  suites={1}  passed={2}  failed={3}  ignored={4}  expected={5}  {6}s" -f
    $code, $suites, $passed, $failed, $ignored, $Expected, $secs)
Write-Host ("[check-rust] log: {0}" -f $logPath)

if ($code -ne 0) { exit 1 }
if ($suites -eq 0) {
    Write-Host "[check-rust] no test result lines parsed"
    exit 1
}
if ($failed -gt 0 -or $ignored -gt 0) { exit 1 }
if ($passed -ne $Expected) {
    Write-Host ("[check-rust] passed {0} != expected {1}" -f $passed, $Expected)
    exit 1
}
exit 0

















