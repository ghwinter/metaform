<#
.SYNOPSIS
    Build and test the proposal components, and run the three-arm proposer experiment.

.DESCRIPTION
    The exact kernel is checked elsewhere (check-rust). This gate covers the components that sit
    behind the operator protocol:

      crates/vsa      vector retrieval          (cargo test, then the offline measurement)
      crates/sme      structure mapping         (cargo test, then the demo)
      crates/runtime  protocol and validation   (cargo test, then the smoke run)

    It then runs research/exp/proposer_arms.py for the k2, k3 and noisy families: the same state to three proposers --
    random, vector retrieval, and the proposals in research/exp/llm_proposals.json -- and has the
    kernel judge all of them. The experiment is deterministic, so a regression in the protocol or in
    a component turns this gate red.

    Exit codes: 0 all components and the experiment agree with their expectations; 1 otherwise.

.PARAMETER Python
    Python interpreter (defaults to $env:METAFORM_PYTHON).

.PARAMETER LlmProposals
    Proposals for the model arm; the self-test replaces this with a file that must fail.

.PARAMETER SkipBuild
    Skip the per-crate `cargo test` steps.
#>
[CmdletBinding()]
param(
    [string]$Python = $env:METAFORM_PYTHON,
    [string]$LlmProposals = '',
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$local = Join-Path $PSScriptRoot 'local.ps1'
if (Test-Path $local) { . $local }
if (-not $Python) { $Python = $env:METAFORM_PYTHON }
if (-not $LlmProposals) { $LlmProposals = Join-Path $root 'research\exp\llm_proposals.json' }

$failures = @()

function Check([string]$name, [scriptblock]$body) {
    Write-Host "[components] $name"
    try {
        & $body
    }
    catch {
        Write-Host "  ! $($_.Exception.Message)"
        $script:failures += $name
    }
}

function Require([bool]$condition, [string]$message) {
    if (-not $condition) { throw $message }
}

foreach ($crate in 'vsa', 'sme', 'ilp', 'runtime') {
    if ($SkipBuild) { continue }
    Check "cargo test $crate" {
        $manifest = Join-Path $root "crates\$crate\Cargo.toml"
        cargo test --quiet --manifest-path $manifest 2>&1 | Out-Host
        Require ($LASTEXITCODE -eq 0) "cargo test $crate exited $LASTEXITCODE"
    }
}

Check 'retrieval measurement' {
    $out = cargo run --quiet --release --manifest-path (Join-Path $root 'crates\vsa\Cargo.toml') --bin vsa_retrieval -- --trials 200 2>&1
    $out | Out-Host
    $vsa = ($out | Select-String 'vsa top-1 splits the pair: ([\d.]+)').Matches.Groups[1].Value
    $rand = ($out | Select-String 'random candidate splits:\s+([\d.]+)').Matches.Groups[1].Value
    Require ($null -ne $vsa -and $null -ne $rand) 'cannot read the measurement'
    Require ([double]$vsa -gt [double]$rand) "retrieval $vsa is not above random $rand"
}

Check 'structure mapping demo' {
    $out = cargo run --quiet --manifest-path (Join-Path $root 'crates\sme\Cargo.toml') --bin sme_demo 2>&1
    $out | Out-Host
    Require ([bool]($out | Select-String 'matched=3 systematicity=5 components=1')) 'unexpected mapping'
    Require ([bool]($out | Select-String 'candidate inferences: 1')) 'expected one inference'
}

Check 'runtime smoke' {
    $out = cargo run --quiet --manifest-path (Join-Path $root 'crates\runtime\Cargo.toml') --bin runtime_smoke 2>&1
    $out | Out-Host
    Require ([bool]($out | Select-String 'status: INSUFFICIENT')) 'the state did not start insufficient'
    Require ([bool]($out | Select-String 'final: .*IDENTIFIED')) 'the run did not end identified'
}

Check 'the CLI path: component proposes, engine validates' {
    $fixtures = Join-Path $root 'crates\runtime\fixtures'
    $runtime = Join-Path $root 'crates\runtime\target\debug\metaform-runtime.exe'
    $engine = Join-Path $root 'engine\target\debug\metaform.exe'
    Require (Test-Path $engine) 'the engine binary is missing (run check-rust first)'
    foreach ($case in @(@('retrieve', 'a&b'), @('analogize', 'liquid'))) {
        $response = Join-Path $env:TEMP "component_$($case[0])_$PID.json"
        $extra = if ($case[0] -eq 'analogize') { @('--systems', (Join-Path $fixtures 'systems.txt')) } else { @('--limit', '3') }
        & $runtime $case[0] --state (Join-Path $fixtures 'state.txt') --out $response @extra | Out-Host
        Require ($LASTEXITCODE -eq 0) "$($case[0]) exited $LASTEXITCODE"
        $verdict = & $engine propose --request (Join-Path $fixtures 'request.json') --response $response
        $verdict | Out-Host
        Require ([bool]($verdict | Select-String ([regex]::Escape('"name":"' + $case[1])))) "$($case[0]) did not lead to $($case[1]) being accepted"
        Remove-Item $response -Force -ErrorAction SilentlyContinue
    }
}

Check 'the controller: adopt the best proposal, then stop' {
    $fixtures = Join-Path $root 'crates\runtime\fixtures'
    $runtime = Join-Path $root 'crates\runtime\target\debug\metaform-runtime.exe'
    $out = & $runtime run --state (Join-Path $fixtures 'state.txt') 2>&1
    $out | Out-Host
    Require ([bool]($out | Select-String 'INSUFFICIENT -> IDENTIFIED')) 'no adopted step'
    Require ([bool]($out | Select-String 'final: 2 predicates, status IDENTIFIED')) 'the run did not settle'
    $out = & $runtime run --state (Join-Path $fixtures 'decided_state.txt') 2>&1
    $out | Out-Host
    Require ([bool]($out | Select-String 'final: 2 predicates, status IDENTIFIED')) 'a decided state changed'
    Require (-not ($out | Select-String 'step 0')) 'a decided state should take no step'
}
Check 'the ILP component: a learned clause goes through the kernel' {
    $fixtures = Join-Path $root 'crates\ilp\fixtures'
    $engine = Join-Path $root 'engine\target\debug\metaform.exe'
    Require (Test-Path $engine) 'the engine binary is missing (run check-rust first)'
    $response = Join-Path $env:TEMP "ilp_response_$PID.json"
    $out = cargo run --quiet --manifest-path (Join-Path $root 'crates\ilp\Cargo.toml') --bin ilp_demo -- --out $response 2>&1
    $out | Out-Host
    Require ([bool]($out | Select-String 'grandparent\(V0,V1\) :- parent\(V0,V2\), parent\(V2,V1\)')) 'unexpected clause'
    $verdict = & $engine propose --request (Join-Path $fixtures 'request.json') --response $response
    $verdict | Out-Host
    Require ([bool]($verdict | Select-String '"name":"grandparent_pair"')) 'the learned clause was not accepted'
    Require ([bool]($verdict | Select-String '"splits":2')) 'the learned clause did not separate both colliding pairs'
    Remove-Item $response -Force -ErrorAction SilentlyContinue
}
foreach ($family in @(@('k2', 10, 0.9), @('k3', 5, 0.8), @('noisy', 5, 0.9))) {
    Check "three proposers, one validator ($($family[0]))" {
        Push-Location (Join-Path $root 'research\exp')
        try {
            $out = & $Python proposer_arms.py --family $family[0] --instances $family[1] --llm $LlmProposals 2>&1
            $out | Out-Host
            $llm = ($out | Select-String '^llm\s+([\d.]+)').Matches.Groups[1].Value
            $vsa = ($out | Select-String '^vsa\s+([\d.]+)').Matches.Groups[1].Value
            Require ($null -ne $llm) 'no model arm in the output'
            Require ([double]$llm -ge $family[2]) "the model arm scored $llm, below $($family[2])"
            Require ([double]$llm -gt [double]$vsa) "the model arm $llm does not beat retrieval $vsa"
        }
        finally { Pop-Location }
    }
}

Write-Host ''
if ($failures.Count -gt 0) {
    Write-Host ("[components] {0} step(s) failed: {1}" -f $failures.Count, ($failures -join ', '))
    exit 1
}
Write-Host '[components] all green'
exit 0
