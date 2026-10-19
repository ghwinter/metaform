"""
End-to-end check, in two parts:

1. Script mode: `engine/scenarios/verify.demo` walks
   VERIFY → PROPOSE → ASK → STOP, and the JSON of each step is checked.
2. Project mode: use `init / state / ledger / report` to build a real project in a temp directory,
   and check that the report really changes its verdict before and after the probe (VERIFY → INSUFFICIENT + collision (0,4)).

This domain is not A/B/C from the paper: 6 test files, deciding "is this an integration test".
What it must show is: **VERIFY comes first, only then PROPOSE** — under the initial vocabulary every equivalence class has a label,
but the labels of 4 and 5 are inherited from their class.

Run: python e2e_verify.py [--engine ...] [--no-build]
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEMO = ROOT / "engine" / "scenarios" / "verify.demo"
DEFAULT_ENGINE = ROOT / "target" / "debug" / "metaform.exe"
MANIFEST = ROOT / "engine" / "Cargo.toml"


def build() -> None:
    r = subprocess.run(
        ["cargo", "build", "--quiet", "--manifest-path", str(MANIFEST)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit(f"cargo build failed:\n{r.stderr[:600]}")


def run(engine: Path) -> list[dict]:
    out = subprocess.run(
        [str(engine), str(DEMO)], capture_output=True, text=True, encoding="utf-8"
    )
    if out.returncode != 0:
        raise SystemExit(f"engine exited with {out.returncode}: {out.stderr[:400]}")
    rows = []
    for line in out.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            rows.append(json.loads(line))
    return rows


def cli(engine: Path, *args: str) -> str:
    r = subprocess.run([str(engine), *args], capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(f"`{' '.join(args)}` exited with {r.returncode}: {r.stderr[:300]}")
    return r.stdout


def last_json(out: str) -> dict:
    for line in reversed(out.splitlines()):
        if line.strip().startswith("{"):
            return json.loads(line)
    raise SystemExit("no JSON in the output")


def project_mode(engine: Path) -> None:
    d = Path(tempfile.mkdtemp(prefix="mf-e2e-"))
    try:
        cli(engine, "init", "--project", str(d))
        cli(engine, "state", "init", "--project", str(d), "--n", "6")
        cli(engine, "state", "add", "--project", str(d), "--name", "slow", "--ids", "0,1,4")
        cli(engine, "state", "add", "--project", str(d), "--name", "doc", "--ids", "0,2,4,5")
        for i, (yes, no) in enumerate([(3, 0), (0, 3), (3, 0), (0, 3)]):
            cli(
                engine, "state", "vote", "--project", str(d),
                "--id", str(i), "--yes", str(yes), "--no", str(no),
            )

        need = last_json(cli(engine, "state", "need", "--project", str(d)))
        assert need["action"] == "VERIFY", need
        assert need["unverified"] == [4, 5], need

        cli(engine, "report", "--project", str(d))
        before = (d / ".metaform" / "out" / "report.md").read_text(encoding="utf-8")
        assert "VERIFY" in before, before
        assert "## To verify" in before, before
        assert "4, 5" in before, before

        cli(
            engine, "state", "vote", "--project", str(d),
            "--id", "4", "--yes", "0", "--no", "3",
            "--record", "--by", "probe", "--reason", "probe 4: collides with 0",
        )
        cli(engine, "report", "--project", str(d))
        after = (d / ".metaform" / "out" / "report.md").read_text(encoding="utf-8")
        assert "INSUFFICIENT" in after, after
        assert "(0,4)" in after, after
        assert "probe 4: collides with 0" in after, "the ledger reason must show up in the report"

        print("project: report VERIFY -> INSUFFICIENT after probe")
    finally:
        shutil.rmtree(d, ignore_errors=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--engine", default=str(DEFAULT_ENGINE))
    ap.add_argument("--no-build", action="store_true")
    args = ap.parse_args()

    engine = Path(args.engine)
    if not args.no_build:
        build()

    rows = run(engine)
    steps = [(r["status"], r["action"]) for r in rows]
    print("steps:", " -> ".join(a for _, a in steps))

    assert len(rows) == 4, f"expected 4 steps, got {len(rows)}: {steps}"
    assert steps[0] == ("IDENTIFIED", "VERIFY"), steps[0]
    assert steps[1] == ("INSUFFICIENT", "PROPOSE"), steps[1]
    assert steps[2] == ("UNDETERMINED", "ASK"), steps[2]
    assert steps[3] == ("IDENTIFIED", "STOP"), steps[3]

    assert rows[0]["unverified"] == [4, 5], rows[0]
    assert rows[1]["colliding_pairs"] == [[0, 4]], rows[1]
    assert rows[1]["unverified"] == [5], "after probing 4, 5 is still unverified"
    assert rows[3]["unverified"] == [], "no inherited label may remain at the final stop"

    project_mode(engine)

    print("OK: script mode VERIFY->PROPOSE->ASK->STOP; in project mode the report changes with the probe")


if __name__ == "__main__":
    main()
