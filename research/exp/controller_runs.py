"""Runs the controller over the three families, with the components and the model arm competing.

Every state is written with a label (its family:index), the model's proposals are written into the
file-proposer format under those labels, and the controller is asked to settle each state. All steps
go into one record, which `experience` then aggregates -- the point is to see whether an operator
wins a class consistently, or whether the answer is "no difference".

Run: python controller_runs.py [--record runs.tsv] [--instances 10]
"""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import proposer_arms as pa

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
RUNTIME = ROOT / "target" / "debug" / "metaform-runtime.exe"


def state_with_label(inst, n_obj: int, n_attr: int, label: str) -> str:
    lines = [f"objects {n_obj}", f"request_id {label}"]
    for a in inst["base"]:
        lines.append(f"vocab a{a} " + " ".join(str(i) for i in inst["attrs"][a]))
    for a in range(n_attr):
        lines.append(f"attr a{a} " + " ".join(str(i) for i in inst["attrs"][a]))
    lines.append("pos " + " ".join(str(i) for i in inst["evidence_pos"]))
    lines.append("neg " + " ".join(str(i) for i in inst["evidence_neg"]))
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--record", default=str(HERE / "tmp" / "runs.tsv"))
    ap.add_argument("--instances", type=int, default=10)
    args = ap.parse_args()

    # Build first: the experiment once ran against a stale binary that silently ignored a flag.
    build = subprocess.run(
        ["cargo", "build", "--quiet", "--manifest-path", str(ROOT / "crates" / "runtime" / "Cargo.toml")],
        capture_output=True, text=True, encoding="utf-8",
    )
    if build.returncode != 0:
        raise SystemExit(f"cargo build failed: {build.stderr[:300]}")

    out_dir = HERE / "tmp"
    out_dir.mkdir(exist_ok=True)
    record = Path(args.record)
    record.unlink(missing_ok=True)

    proposals = json.loads((HERE / "llm_proposals.json").read_text(encoding="utf-8"))
    work = out_dir / "controller"
    work.mkdir(exist_ok=True)

    lines = []
    for key, entries in proposals.items():
        if key.startswith("_"):
            continue
        for entry in entries:
            ids = " ".join(str(i) for i in entry["ids"])
            lines.append(f"{key} {entry['name']} {ids}")
    proposals_file = work / "proposals.txt"
    proposals_file.write_text("\n".join(lines) + "\n", encoding="utf-8")

    rows = []
    for family, count in (("k2", args.instances), ("k3", 5), ("noisy", 5)):
        for k in range(count):
            inst = pa.instance(pa.__dict__.get("SEED", 11) + k, family)
            label = f"{family}:{k}"
            state = work / f"{family}_{k}.txt"
            state.write_text(state_with_label(inst, 12, 8, label), encoding="utf-8")
            out = subprocess.run(
                [str(RUNTIME), "run", "--state", str(state), "--proposals", str(proposals_file),
                 "--record", str(record)],
                capture_output=True, text=True, encoding="utf-8",
            )
            if out.returncode != 0:
                raise SystemExit(f"run failed on {label}: {out.stderr[:300]}")
            steps = [l.strip() for l in out.stdout.splitlines() if "step " in l or l.startswith("final:")]
            adopted = [l for l in steps if "proposed" in l]
            operator = adopted[0].split(":")[1].split("proposed")[0].strip() if adopted else "none"
            rows.append((label, len(adopted), operator, steps[-1] if steps else ""))

    print(f"{'state':10s} {'steps':>5s} {'operator':16s} final")
    for label, n, operator, final in rows:
        print(f"{label:10s} {n:5d} {operator:16s} {final}")

    print()
    out = subprocess.run(
        [str(RUNTIME), "experience", "--record", str(record)],
        capture_output=True, text=True, encoding="utf-8",
    )
    print(out.stdout.strip())
    print(f"record: {record}")


if __name__ == "__main__":
    main()
