"""
End-to-end check for the `metaform.propose/1` boundary: mock model -> engine -> stdout JSON.

Three cases:
  good    one valid candidate is accepted with splits > 0; out-of-range and duplicate-name are rejected
  bad     the wrong protocol rejects the whole response before any candidate is read
  refuse  the model declines; `refused` is echoed and nothing is accepted

The engine is read-only by contract, so the request and response files must be byte-identical after
every run.

Run: python research/exp/llm_e2e.py [--engine ...] [--no-build]
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEFAULT_ENGINE = ROOT / "target" / "debug" / "metaform.exe"
MANIFEST = ROOT / "engine" / "Cargo.toml"
MOCK = HERE / "llm_mock.py"

REQUEST = {
    "protocol": "metaform.propose/1",
    "request_id": "e2e-1",
    "objects": 6,
    "vocabulary": [{"name": "p0", "ids": [0, 1]}],
    "evidence": {"pos": [0, 2], "neg": [1, 5]},
    "limit": 8,
    "ask": "propose predicates that split the colliding pairs",
}


def build() -> None:
    """Same build pattern as e2e_verify.py."""
    r = subprocess.run(
        ["cargo", "build", "--quiet", "--manifest-path", str(MANIFEST)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit(f"cargo build failed:\n{r.stderr[:600]}")


def run_mock(request: Path, response: Path, case: str) -> None:
    r = subprocess.run(
        [
            sys.executable,
            str(MOCK),
            "--request",
            str(request),
            "--response",
            str(response),
            "--case",
            case,
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit(f"llm_mock failed for case {case}:\n{r.stderr[:400]}")


def run_engine(engine: Path, request: Path, response: Path) -> dict:
    r = subprocess.run(
        [str(engine), "propose", "--request", str(request), "--response", str(response)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit(f"engine propose exited with {r.returncode}:\n{r.stderr[:400]}")
    lines = [line for line in r.stdout.splitlines() if line.strip()]
    if len(lines) != 1:
        raise SystemExit(f"engine propose must print exactly one line, got {len(lines)}:\n{r.stdout[:400]}")
    return json.loads(lines[0])


def expect(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--engine", default=str(DEFAULT_ENGINE))
    ap.add_argument("--no-build", action="store_true")
    args = ap.parse_args()

    engine = Path(args.engine)
    if not engine.exists():
        if args.no_build:
            raise SystemExit(f"engine binary not found: {engine}")
        build()
    if not engine.exists():
        raise SystemExit(f"engine binary not found after build: {engine}")

    d = Path(tempfile.mkdtemp(prefix="mf-llm-e2e-"))
    try:
        request = d / "request.json"
        request.write_text(json.dumps(REQUEST, indent=2) + "\n", encoding="utf-8")
        request_before = request.read_bytes()

        # good: one usable candidate, one out-of-range, one duplicate name.
        response = d / "response-good.json"
        run_mock(request, response, "good")
        response_before = response.read_bytes()
        out = run_engine(engine, request, response)
        expect(out["protocol"] == "metaform.propose/1", f"wrong protocol echoed: {out}")
        expect(out["request_id"] == "e2e-1", f"wrong request id echoed: {out}")
        expect(out["refused"] is None, f"good response must not be refused: {out}")
        expect(len(out["accepted"]) == 1, f"expected 1 accepted candidate: {out}")
        expect(out["accepted"][0]["splits"] > 0, f"accepted candidate must split evidence pairs: {out}")
        reasons = [r["reason"] for r in out["rejected"]]
        expect(reasons == ["out_of_range", "duplicate_name"], f"unexpected rejected reasons: {reasons}")
        expect(request.read_bytes() == request_before, "the engine modified the request file")
        expect(response.read_bytes() == response_before, "the engine modified the response file")
        print("OK: good -> 1 accepted with splits>0; out_of_range and duplicate_name rejected")

        # bad: the protocol check fires before any candidate is considered.
        response_bad = d / "response-bad.json"
        run_mock(request, response_bad, "bad")
        out = run_engine(engine, request, response_bad)
        expect(out["accepted"] == [], f"wrong protocol must accept nothing: {out}")
        expect(out["refused"] is None, f"wrong protocol is not a refusal: {out}")
        expect(
            len(out["rejected"]) == 1 and out["rejected"][0]["reason"] == "wrong_protocol",
            f"expected exactly one wrong_protocol rejection: {out}",
        )
        print("OK: bad -> whole response rejected with wrong_protocol")

        # refuse: the model is allowed to decline.
        response_refuse = d / "response-refuse.json"
        run_mock(request, response_refuse, "refuse")
        out = run_engine(engine, request, response_refuse)
        expect(out["accepted"] == [], f"a refusal must accept nothing: {out}")
        expect(out["rejected"] == [], f"a refusal must not reject candidates: {out}")
        expect(
            isinstance(out["refused"], str) and out["refused"],
            f"the refusal string must be echoed: {out}",
        )
        print("OK: refuse -> refused set, 0 accepted")
    finally:
        shutil.rmtree(d, ignore_errors=True)


if __name__ == "__main__":
    main()
