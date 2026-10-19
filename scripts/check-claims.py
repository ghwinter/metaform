#!/usr/bin/env python3
"""Check the repository's claims about itself.

`experiments/claims.tsv` holds sentences the repository asks a reader to believe, each with a
command whose output would refute it. This script runs the commands and reports:

  PASS  the check still supports the claim
  FAIL  the check no longer supports the claim (the prose drifted from the code)
  NEED  the claim has no executable check yet (the row's status is `unverified`)

A FAIL is a gate failure: it means a sentence in the repository is now false. A NEED is not a
failure; it is the list of things we assert without having made them checkable, which is where
hallucination lives.

Usage:
  python check-claims.py            # run every verified claim, list the NEED rows
  python check-claims.py --need     # only the NEED list
  python check-claims.py --json     # machine-readable report
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLAIMS = ROOT / "experiments" / "claims.tsv"


def rows(path: Path = CLAIMS) -> list[dict]:
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < 5:
            continue
        out.append(
            {
                "id": fields[0].strip(),
                "claim": fields[1].strip(),
                "check": fields[2].strip(),
                "expect": fields[3].strip(),
                "status": fields[4].strip(),
            }
        )
    return out


def run(check: str) -> tuple[int, str]:
    result = subprocess.run(
        ["pwsh", "-NoProfile", "-Command", check],
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return result.returncode, (result.stdout + result.stderr).strip()


def holds(code: int, output: str, expect: str) -> bool:
    if expect == "exit0":
        return code == 0
    kind, _, text = expect.partition(":")
    if kind == "contains":
        return text in output
    if kind == "lacks":
        return text not in output
    raise SystemExit(f"unknown expectation: {expect}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--need", action="store_true", help="only print the claims without a check")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--claims", default=str(CLAIMS), help="claim file to check (default: experiments/claims.tsv)")
    args = ap.parse_args()

    report = []
    for row in rows(Path(args.claims)):
        if row["status"] == "unverified":
            report.append({**row, "verdict": "NEED", "output": ""})
            continue
        if args.need:
            continue
        code, output = run(row["check"])
        report.append({**row, "verdict": "PASS" if holds(code, output, row["expect"]) else "FAIL", "output": output})

    if args.json:
        print(json.dumps(report, indent=1))
    else:
        for r in report:
            print(f"{r['verdict']:4s} {r['id']:3s} {r['claim']}")
            if r["verdict"] == "FAIL":
                print(f"     check: {r['check']}")
                print(f"     expect {r['expect']}, got: {r['output'][:200]}")
        need = [r for r in report if r["verdict"] == "NEED"]
        if need and not args.need:
            print(f"\n{len(need)} claim(s) have no executable check: {', '.join(r['id'] for r in need)}")
    bad = [r for r in report if r["verdict"] == "FAIL"]
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
