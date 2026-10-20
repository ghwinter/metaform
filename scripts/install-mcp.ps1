# Install the MCP server to a stable prefix and prove the wire before a host is pointed at it.
#
# The install path is deliberately outside the repository: target\debug is a build artifact that
# cargo clean removes, and a host that starts a missing binary fails at session start, not here.
# Run from the repository root; register the printed path in the host's profile afterwards.
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

Write-Host ''
Write-Host 'Register this entry in the host profile (cordis.patch.yml), then restart the host:'
Write-Host ''
Write-Host '- id: mcp-metaform'
Write-Host '  name: "@deepseek-ai/dsh-mcp-client"'
Write-Host '  config:'
Write-Host '    serverName: metaform'
Write-Host '    transport: stdio'
Write-Host "    command: '$exe'"
Write-Host '    args:'
Write-Host '      - mcp'
Write-Host "    cwd: '$((Get-Location).Path)'"
Write-Host '    toolCallTimeoutMs: 60000'
Write-Host '    failOnStartupError: true'
