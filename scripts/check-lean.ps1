<#
.SYNOPSIS
    编译 research/lean 里的 Lean 形式化，并把输出留证。

.DESCRIPTION
    本机 mathlib 只有一份（D:\GitHub\lean\mathlib），但能够真正加载它的工程是
    D:\GitHub\workspace\ReasearchLean —— 那里 .lake\packages 里的 batteries/aesop/Qq
    等依赖已经编译好。在 metaform 里新建工程会触发 Lake 重新 materialize 这批依赖，
    而它们没有预编译产物（mathlib 的 cache 钩子在本机失效）。

    所以：源码留在 research/lean（进论文仓库），编译借 ReasearchLean 的环境。

    本脚本把 stdout+stderr 全量写进 `research/lean/last-check.log`，并统计 warning。
    论文附录声称"no sorry and no warnings"，这条现在可复现：
    **有任何 warning 就返回非零**，不能靠记忆。

    退出码：
      0  编译通过且零 warning
      1  Lean 编译失败（或源码缺失）
      2  编译通过但有 warning

.PARAMETER File
    源文件名（research/lean 下）。

.PARAMETER Project
    借用其环境的 Lake 工程。
#>
[CmdletBinding()]
param(
    [string]$File = 'QueryDriven.lean',
    [string]$SourceDir = (Join-Path $PSScriptRoot '..\research\lean'),
    [string]$Project = 'D:\GitHub\workspace\ReasearchLean',
    [string]$LogPath
)

$ErrorActionPreference = 'Stop'
$env:MATHLIB_NO_CACHE_ON_UPDATE = '1'
$env:MATHLIB_CACHE_DIR = 'D:\GitHub\lean\.cache'

$src = Join-Path $SourceDir $File
if (-not (Test-Path $src)) {
    Write-Host "[check-lean] source not found: $src"
    exit 1
}
if (-not $LogPath) { $LogPath = Join-Path $SourceDir 'last-check.log' }

$dstDir = Join-Path $Project 'Research'
New-Item -ItemType Directory -Force -Path $dstDir | Out-Null
Copy-Item $src (Join-Path $dstDir $File) -Force

Push-Location $Project
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $out = & lake env lean (Join-Path 'Research' $File) 2>&1 | Out-String
    $code = $LASTEXITCODE
    $secs = [math]::Round($sw.Elapsed.TotalSeconds, 1)

    $header = "# check-lean run: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')  file=$File  exit=$code  secs=$secs`n"
    ($header + $out) | Set-Content -Path $LogPath -Encoding utf8

    $warnings = ([regex]::Matches($out, '(?m)^.*\bwarning\b.*$')).Count
    $sorries = ([regex]::Matches($out, '(?m)^.*\bsorry\b.*$')).Count

    Write-Host ("[check-lean] {0}  exit={1}  warnings={2}  sorry-lines={3}  {4}s" -f $File, $code, $warnings, $sorries, $secs)
    Write-Host ("[check-lean] log: {0}" -f $LogPath)

    if ($code -ne 0) { exit 1 }
    if ($warnings -gt 0) { exit 2 }
    exit 0
}
finally { Pop-Location }
