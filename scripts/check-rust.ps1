<#
.SYNOPSIS
    跑 Rust 测试，并断言"通过数 == 期望数、失败 0、忽略 0"。

.DESCRIPTION
    `cargo test` 只有在测试失败时才返回非零。若某个测试被 #[ignore] 掉、或整个测试模块
    被 cfg 掉，输出仍是绿的。所以这里把数字固定住：通过数必须等于 -Expected，
    且 ignored 必须为 0。

    日志写到 engine/last-test.log。

    退出码：
      0  通过数符合期望、0 失败、0 忽略
      1  不满足

.PARAMETER Expected
    期望通过数（默认 63：41 个内核 + 22 个 CLI/台账/场景/MCP/plan/report/score 测试）。

.PARAMETER Manifest
    Cargo.toml 路径。
#>
[CmdletBinding()]
param(
    [int]$Expected = 63,
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
    Write-Host "[check-rust] 没有解析到任何 test result 行"
    exit 1
}
if ($failed -gt 0 -or $ignored -gt 0) { exit 1 }
if ($passed -ne $Expected) {
    Write-Host ("[check-rust] 通过数 {0} != 期望 {1}" -f $passed, $Expected)
    exit 1
}
exit 0

















