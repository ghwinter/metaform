"""
Regression check: do the key numbers in the paper still hold when rerun?

The numbers and tolerances all live in EXPECTED; any item outside its tolerance exits non-zero.
The point is to avoid "the paper says 2.01, the code now gives 3.4, and nobody noticed".

Run: python verify_claims.py [--tolerance 0.06]
"""
from __future__ import annotations

import argparse
import json
import re
import statistics
import subprocess
import sys
from pathlib import Path

from multi_seed import run_cover, run_policy

SEEDS = [1, 2, 3, 4, 5]
N = 150

# cost-law consistency check: the paper claims total ≈ r/q + 7.6 (r=20); below are four measured points.
COST_R = 20.0
COST_ROUNDS = 7.6
COST_POINTS = [(1.0, 27.6), (0.5, 49.4), (0.25, 88.9), (0.125, 162.7)]
PAPER_EN = Path(__file__).resolve().parents[2] / "paper" / "en" / "main.tex"
ROOT = Path(__file__).resolve().parents[2]
DEFAULT_ENGINE = ROOT / "engine" / "target" / "debug" / "metaform.exe"

# the three groups of numbers from `metaform ops` (cited by the paper and the README)
OPS_SELECTION = {"selected_cost": 3.11, "random_cost": 3.50, "fixed_cost": 3.60}
OPS_PROBE = {"early_queries": 1, "verified_queries": 5, "verified_cost": 14}

# values in the paper (RESULTS.md §3.18 / §3.5)
EXPECTED = {
    "gain": 2.01,
    "rand_inf": 1.91,
    "random_queries": 6.14,
    "greedy": 2.10,
    "optimal": 1.85,
    "random_cover": 4.28,
}


def agg(fn) -> tuple[float, float]:
    vals = [fn(s) for s in SEEDS]
    return statistics.mean(vals), statistics.stdev(vals)


def ensure_engine(path: Path) -> Path:
    if not path.exists():
        r = subprocess.run(
            ["cargo", "build", "--quiet", "--manifest-path", str(ROOT / "engine" / "Cargo.toml")],
            capture_output=True, text=True, encoding="utf-8",
        )
        if r.returncode != 0:
            print(f"! cargo build failed:\n{r.stderr[:400]}")
            sys.exit(1)
    return path


def check_ops(engine: Path) -> int:
    """Run `metaform ops` and check the selection and probe numbers cited by the paper."""
    out = subprocess.run([str(engine), "ops"], capture_output=True, text=True, encoding="utf-8")
    try:
        doc = json.loads(out.stdout.strip().splitlines()[-1])
    except Exception:
        print("! ops output is not parseable")
        return 1
    bad = 0
    sel = doc.get("selection", {})
    for k, want in OPS_SELECTION.items():
        got = sel.get(k)
        ok = got is not None and abs(got - want) <= 0.011
        bad += 0 if ok else 1
        print(f"ops.selection.{k:14s} want {want:5.2f} got {got!s:>6}  {'OK' if ok else 'DRIFT'}")
    probe = doc.get("probe", {})
    for k, want in OPS_PROBE.items():
        got = probe.get(k)
        ok = got == want
        bad += 0 if ok else 1
        print(f"ops.probe.{k:16s} want {want:>3} got {got!s:>4}  {'OK' if ok else 'DRIFT'}")
    text = PAPER_EN.read_text(encoding="utf-8")
    msel = re.search(
        r"needs \$([\d.]+)\$ queries against \$([\d.]+)\$ for a random order and \$([\d.]+)\$ for a fixed one",
        re.sub(r"\s+", " ", text),
    )
    if not msel:
        bad += 1
        print("! the selection sentence in the paper changed shape")
    else:
        for k, got in zip(
            ("selected_cost", "random_cost", "fixed_cost"),
            (float(x) for x in msel.groups()),
        ):
            if abs(got - sel[k]) > 0.011:
                bad += 1
                print(f"! the paper's {k}={got} disagrees with ops {sel[k]}")

    mcost = re.search(
        r"total cost is \$([\d.]+)\$, \$([\d.]+)\$, \$([\d.]+)\$ and \$([\d.]+)\$",
        re.sub(r"\s+", " ", text),
    )
    if not mcost:
        bad += 1
        print("! the four-cost-points sentence in the paper changed shape")
    else:
        for (q, _), got in zip(COST_POINTS, (float(x) for x in mcost.groups())):
            want = next(p for qq, p in COST_POINTS if qq == q)
            if abs(got - want) > 0.05:
                bad += 1
                print(f"! the paper's measured point q={q} is written {got}, expected {want}")

    # Third source: the three numbers in the README, compared item by item with the ops output,
    # so the paper and the constants cannot drift together.
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    flat = re.sub(r"\s+", " ", readme)
    m = re.search(r"selection takes ([\d.]+) queries, random order ([\d.]+), and fixed order ([\d.]+)", flat)
    if not m:
        bad += 1
        print("! the README selection sentence changed shape (all three numbers must appear)")
    else:
        for k, got in zip(
            ("selected_cost", "random_cost", "fixed_cost"),
            (float(x) for x in m.groups()),
        ):
            if abs(got - sel[k]) > 0.011:
                bad += 1
                print(f"! README {k}={got} disagrees with ops {sel[k]}")
    mp = re.search(r"verified version takes (\d+) queries, costs (\d+)", flat)
    if not mp:
        bad += 1
        print("! the README probe sentence changed shape")
    elif (int(mp.group(1)), int(mp.group(2))) != (
        probe.get("verified_queries"),
        probe.get("verified_cost"),
    ):
        bad += 1
        print(f"! the README probe numbers {mp.groups()} disagree with ops")
    return bad


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tolerance", type=float, default=0.06)
    ap.add_argument("--tamper", action="append", default=[], metavar="KEY=VALUE",
                    help="deliberately corrupt one expected value, to prove the gate is live")
    ap.add_argument("--tamper-cost", action="store_true",
                    help="corrupt the first cost point, to prove the cost-law check is live")
    ap.add_argument("--engine", default=str(DEFAULT_ENGINE),
                    help="metaform executable (used to check the ops numbers)")
    ap.add_argument("--tamper-ops", action="store_true",
                    help="corrupt an ops expected value, to prove that check is live")
    args = ap.parse_args()

    if args.tamper_cost:
        COST_POINTS[0] = (1.0, 99.0)
        print("[tamper] cost point q=1 := 99.0")

    if args.tamper_ops:
        OPS_SELECTION["selected_cost"] = 9.99
        print("[tamper] ops.selection.selected_cost := 9.99")

    for t in args.tamper:
        k, v = t.split("=", 1)
        if k not in EXPECTED:
            print(f"unknown claim: {k}")
            sys.exit(2)
        EXPECTED[k] = float(v)
        print(f"[tamper] {k} := {v}")

    rows = []
    for policy, key in (("gain", "gain"), ("rand_inf", "rand_inf"), ("random", "random_queries")):
        m, sd = agg(lambda s, p=policy: run_policy(s, p, N, 12, 8, 2))
        rows.append((key, EXPECTED[key], m, sd))

    cover = [run_cover(s, N, 10, 12) for s in SEEDS]
    for i, key in ((0, "greedy"), (1, "random_cover"), (2, "optimal")):
        vals = [c[i] for c in cover]
        rows.append((key, EXPECTED[key], statistics.mean(vals), statistics.stdev(vals)))


    print(f"{'claim':16s} {'paper':>7s} {'now':>7s} {'sd':>6s} {'diff':>7s}  verdict")
    bad = 0
    for key, want, got, sd in rows:
        diff = got - want
        ok = abs(diff) <= args.tolerance
        bad += 0 if ok else 1
        print(f"{key:16s} {want:7.2f} {got:7.2f} {sd:6.2f} {diff:+7.2f}  {'OK' if ok else 'REG RESSED'}")

    if bad:
        print(f"\n{bad} items outside the tolerance {args.tolerance}")
        sys.exit(1)

    # --- cost-law consistency: the formula and the four measured points must hold at the same time ---
    print(f"\n{'q':>6s} {'measured':>9s} {'20/q+7.6':>9s} {'rel.err':>8s}")
    law_bad = 0
    for q, measured in COST_POINTS:
        predicted = COST_R / q + COST_ROUNDS
        err = abs(measured - predicted) / predicted
        ok = err <= 0.04
        law_bad += 0 if ok else 1
        print(f"{q:6.3f} {measured:9.2f} {predicted:9.2f} {err:8.1%}  {'OK' if ok else 'BROKEN'}")

    text = PAPER_EN.read_text(encoding="utf-8")
    if "close to $r/q + 7.6$" not in text:
        print("! the paper no longer states close to r/q+7.6")
        law_bad += 1
    if "exactly $r/q" in text:
        print("! the paper says exactly r/q again (which the measurements contradict)")
        law_bad += 1

    if law_bad:
        print(f"\ncost law inconsistent in {law_bad} places")
        sys.exit(1)

    ops_bad = check_ops(ensure_engine(Path(args.engine)))
    if ops_bad:
        print(f"\nops numbers drifted in {ops_bad} places")
        sys.exit(1)
    print("\nall claims within tolerance")


if __name__ == "__main__":
    main()


