# Re-runs the measurement in RESULTS.md §28: one round, then three arms, then the statistics.
#
#   pwsh -File experiments/proposer-arms/reproduce.ps1 -Model phi
#   pwsh -File experiments/proposer-arms/reproduce.ps1 -Model qwen2b -N 12
#
# Requires a llama-server on :8080 holding the model, and the workspace binary built. The numbers in
# this directory were produced this way; sampling is at temperature 0.7 with no fixed server seed, so
# a rerun gives different counts and q_stats.py is what says whether it disagreed.
#
#   llama-server -m tmp\models\Phi-4-mini-instruct-Q4_K_M.gguf -ngl 99 -c 4096 --jinja --port 8080
#
# The `pairs` arm needs the two colliding objects, which `metaform propose` reports only as a count.
# `witness_pair` in local_proposer.py recomputes them from the request -- identical signature,
# opposite labels -- because the protocol has no field for the witness. That is the gap §28 names.

param(
    [ValidateSet("phi", "qwen2b")] [string]$Model = "phi",
    [int]$N = 60,
    [int]$Seed = 11,
    [string]$Out = "tmp/repro",
    [int]$Port = 8080
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $root
try {
    $py = "python"
    $lp = "experiments/local_proposer.py"
    $ep = "http://127.0.0.1:$Port"

    & $py $lp instances --n $N --seed $Seed --out $Out | Out-Host
    & $py $lp prompt --instances "$Out/instances.json" --round 1 --out "$Out/p1.json" | Out-Host
    & $py $lp answer --prompts "$Out/p1.json" --instances "$Out/instances.json" `
        --endpoint $ep --out "$Out/a1.json" | Out-Host

    foreach ($arm in @("placebo", "diagnosis", "pairs")) {
        & $py $lp prompt --instances "$Out/instances.json" --round 2 `
            --first "$Out/a1.json" --feedback $arm --out "$Out/p2_$arm.json" | Out-Host
        & $py $lp answer --prompts "$Out/p2_$arm.json" --instances "$Out/instances.json" `
            --endpoint $ep --out "$Out/a2_$arm.json" | Out-Host
        & $py $lp score --instances "$Out/instances.json" --first "$Out/a1.json" `
            --second "$Out/a2_$arm.json" --workdir "$Out/w_$arm" --tsv "$Out/rows_$arm.tsv" | Out-Host
    }

    & $py experiments/q_stats.py "$Out/rows_placebo.tsv" "$Out/rows_pairs.tsv" | Out-Host
    & $py experiments/q_stats.py "$Out/rows_diagnosis.tsv" "$Out/rows_pairs.tsv" | Out-Host
} finally {
    Pop-Location
}
