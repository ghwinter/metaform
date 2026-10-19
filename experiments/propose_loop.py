"""
One proposal round through the real `metaform.propose/1` boundary, end to end, in a temp directory.

Flow (the script never edits state directly; every fact comes back from the engine binary):
  1. build the engine when the binary is missing, then locate it (`--engine` or the default path)
  2. write a scenario that mirrors `engine/scenarios/verify.demo` up to INSUFFICIENT / PROPOSE
  3. write the protocol request describing that state; `llm_mock.py --case loop` fabricates the response
  4. `metaform propose --request ... --response ...`: 1 accepted (`net`, splits > 0), 2 rejected
  5. append `add net 0 2`, `vote 5 3 0`, `need`: the last step must be IDENTIFIED / STOP
  6. `explain` on the same scenario: only the partition induced by slow, doc and net is left, so the
     rejected candidates (`slow`, `ghost`) left no trace
  7. print one summary line; any failed assertion exits non-zero

The request's negative evidence includes object 4. `verify.demo` probes 4 with `vote 4 0 3`, so 4 is
negative and collides with positive 0; without it the boundary has no colliding pair to split and
`net` would be accepted with `splits == 0`.

Deterministic: fixed ids, no timestamps, and order-insensitive facts are sorted (`explain` prints its
equivalence classes in hash order, so classes are compared as sets).

Run: python experiments/propose_loop.py [--engine ...] [--no-build]
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
DEFAULT_ENGINE = ROOT / "target" / "debug" / "metaform.exe"
MANIFEST = ROOT / "engine" / "Cargo.toml"
MOCK = HERE / "llm_mock.py"

REQUEST_ID = "propose-loop-1"

# `engine/scenarios/verify.demo` up to INSUFFICIENT: votes 0..3 as in the demo, then the probe of
# object 4 (negative), which collides with positive 0.
SCENARIO_HEAD = """\
objects 6
add slow 0 1 4
add doc 0 2 4 5
vote 0 3 0
vote 1 0 3
vote 2 3 0
vote 3 0 3
vote 4 0 3
need
"""

# Votes recorded by SCENARIO_HEAD: 0 and 2 are positive; 1, 3, 4 are negative.
EVIDENCE_POS = [0, 2]
EVIDENCE_NEG = [1, 3, 4]

REQUEST = {
    "protocol": "metaform.propose/1",
    "request_id": REQUEST_ID,
    "objects": 6,
    "vocabulary": [
        {"name": "slow", "ids": [0, 1, 4]},
        {"name": "doc", "ids": [0, 2, 4, 5]},
    ],
    "evidence": {"pos": EVIDENCE_POS, "neg": EVIDENCE_NEG},
    "limit": 8,
    "ask": "propose a predicate that splits the colliding pair (0,4)",
}

CLASS_LINE = re.compile(r"^\s*\[\s*\d+\]\s+(.*)$")
MEMBER = re.compile(r"^([+\-]?)(\d+)$")


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


def run_scenario(engine: Path, path: Path) -> tuple[list[dict], str]:
    """Run a scenario script; return its JSON rows and its full stdout."""
    r = subprocess.run(
        [str(engine), str(path)], capture_output=True, text=True, encoding="utf-8"
    )
    if r.returncode != 0:
        raise SystemExit(f"engine scenario exited with {r.returncode}:\n{r.stderr[:400]}")
    rows = [json.loads(line) for line in r.stdout.splitlines() if line.strip().startswith("{")]
    return rows, r.stdout


def cli(engine: Path, *args: str) -> str:
    r = subprocess.run(
        [str(engine), *args], capture_output=True, text=True, encoding="utf-8"
    )
    if r.returncode != 0:
        raise SystemExit(f"`{' '.join(args)}` exited with {r.returncode}:\n{r.stderr[:400]}")
    return r.stdout


def run_mock(request: Path, response: Path, case: str) -> None:
    """The mock is the only place a response is fabricated; the engine only validates it."""
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


def parse_explain(text: str) -> list[dict[int, str]]:
    """`explain` output as a list of {object id: '+'/'-'/' '} classes (engine order is hashed)."""
    classes: list[dict[int, str]] = []
    for line in text.splitlines():
        m = CLASS_LINE.match(line)
        if not m:
            continue
        members: dict[int, str] = {}
        for token in m.group(1).split():
            t = MEMBER.match(token)
            if t is None:
                raise SystemExit(f"unexpected `explain` member: {token!r}")
            members[int(t.group(2))] = t.group(1) or " "
        classes.append(members)
    return classes


def expected_classes(
    n: int, preds: list[tuple[str, set[int]]]
) -> set[frozenset[int]]:
    """The equivalence classes a given vocabulary induces, independent of print order."""
    groups: dict[tuple[bool, ...], set[int]] = {}
    for x in range(n):
        sig = tuple(x in ext for _, ext in preds)
        groups.setdefault(sig, set()).add(x)
    return {frozenset(members) for members in groups.values()}


def expect(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def main() -> int:
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

    d = Path(tempfile.mkdtemp(prefix="mf-propose-loop-"))
    try:
        scenario = d / "scenario.demo"
        scenario.write_text(SCENARIO_HEAD, encoding="utf-8")

        # 2. The state must be proven insufficient, and the recommended action must be PROPOSE.
        rows, _ = run_scenario(engine, scenario)
        expect(len(rows) == 1, f"expected one `need` row, got {len(rows)}")
        expect(
            rows[0]["status"] == "INSUFFICIENT" and rows[0]["action"] == "PROPOSE",
            f"expected INSUFFICIENT/PROPOSE, got {rows[0]}",
        )
        expect(
            rows[0]["colliding_pairs"] == [[0, 4]],
            f"the probe of object 4 must collide with object 0: {rows[0]}",
        )

        # 3. Request written by us, response fabricated by the mock.
        request = d / "request.json"
        request.write_text(json.dumps(REQUEST, indent=2) + "\n", encoding="utf-8")
        response = d / "response.json"
        run_mock(request, response, "loop")
        request_before = request.read_bytes()
        response_before = response.read_bytes()

        # 4. One pass through the boundary.
        out_text = cli(
            engine, "propose", "--request", str(request), "--response", str(response)
        )
        lines = [line for line in out_text.splitlines() if line.strip()]
        expect(len(lines) == 1, f"propose must print exactly one line, got:\n{out_text}")
        out = json.loads(lines[0])

        expect(out["protocol"] == "metaform.propose/1", f"wrong protocol echoed: {out}")
        expect(out["request_id"] == REQUEST_ID, f"wrong request id echoed: {out}")
        expect(out["refused"] is None, f"the response must not be a refusal: {out}")

        accepted = out["accepted"]
        rejected = out["rejected"]
        expect(len(accepted) == 1, f"expected exactly 1 accepted candidate: {out}")
        expect(
            accepted[0]["name"] == "net" and accepted[0]["ids"] == [0, 2],
            f"the accepted candidate must be net=[0,2]: {out}",
        )
        expect(accepted[0]["splits"] > 0, f"`net` must split evidence pairs: {out}")
        expect(len(rejected) == 2, f"expected exactly 2 rejected candidates: {out}")
        reasons = sorted(entry["reason"] for entry in rejected)
        expect(
            reasons == ["duplicate_name", "out_of_range"],
            f"unexpected rejection reasons: {reasons}",
        )
        rejected_names = sorted(entry["name"] for entry in rejected)
        expect(rejected_names == ["ghost", "slow"], f"unexpected rejected names: {rejected_names}")
        by_name = {entry["name"]: entry["reason"] for entry in rejected}
        expect(
            by_name.get("slow") == "duplicate_name"
            and by_name.get("ghost") == "out_of_range",
            f"each candidate must fail its own rule: {by_name}",
        )
        expect(request.read_bytes() == request_before, "the boundary modified the request file")
        expect(
            response.read_bytes() == response_before, "the boundary modified the response file"
        )

        # 5. Adopt exactly the accepted candidate, answer the last open class, and stop.
        name = accepted[0]["name"]
        ids = accepted[0]["ids"]
        adopted = f"add {name} {' '.join(str(i) for i in ids)}"
        scenario.write_text(SCENARIO_HEAD + adopted + "\nvote 5 3 0\nneed\n", encoding="utf-8")

        rows, stdout = run_scenario(engine, scenario)
        expect(len(rows) == 2, f"expected two `need` rows, got {len(rows)}")
        final = rows[-1]
        expect(
            final["status"] == "IDENTIFIED" and final["action"] == "STOP",
            f"expected IDENTIFIED/STOP after the proposal round, got {final}",
        )
        expect(final["unverified"] == [], f"no inherited label may remain at STOP: {final}")

        sizes = [int(m) for m in re.findall(r"vocabulary has (\d+) entries", stdout)]
        expect(
            sizes == [1, 2, 3],
            f"expected exactly the three predicates slow, doc, net; vocabulary sizes: {sizes}",
        )

        # 6. `explain` on the same scenario: the rejected candidates left no trace.
        #    `explain` lists equivalence classes (ids), not names, so "only slow, doc, net remain"
        #    is checked as the class partition those three predicates induce, plus a vocabulary of
        #    exactly three entries (a duplicate `slow` would show up as a fourth).
        scenario.write_text(
            SCENARIO_HEAD + adopted + "\nvote 5 3 0\nneed\nexplain\n", encoding="utf-8"
        )
        _, explained = run_scenario(engine, scenario)

        classes = parse_explain(explained)
        expect(len(classes) == 6, f"expected 6 classes at STOP:\n{explained}")
        observed = {frozenset(members) for members in classes}
        expected = expected_classes(
            6, [("slow", {0, 1, 4}), ("doc", {0, 2, 4, 5}), (name, set(ids))]
        )
        expect(
            observed == expected,
            f"explain classes {sorted(sorted(c) for c in observed)} differ from slow/doc/net "
            f"{sorted(sorted(c) for c in expected)}",
        )
        # The proposal was timed against EVIDENCE_POS/NEG; `vote 5 3 0` then labels object 5 positive.
        labels = {**{x: "+" for x in EVIDENCE_POS}, **{x: "-" for x in EVIDENCE_NEG}, 5: "+"}
        for members in classes:
            for obj, tag in members.items():
                expect(
                    tag == labels[obj],
                    f"object {obj} is labeled {tag!r}, expected {labels[obj]!r}",
                )
        expect(
            "colliding pairs: none" in explained,
            f"the final state must have no colliding pair:\n{explained}",
        )
        expect("ghost" not in explained, "the rejected `ghost` candidate left a trace")

        text = scenario.read_text(encoding="utf-8")
        adds = [line.strip() for line in text.splitlines() if line.strip().startswith("add ")]
        expect(
            adds == ["add slow 0 1 4", "add doc 0 2 4 5", adopted],
            f"the scenario adopted unexpected predicates: {adds}",
        )
        expect("ghost" not in text, "the rejected `ghost` candidate was written into the scenario")

        rounds = 1
        queries = 1
        print(
            f"OK: {rounds} proposal round, {len(accepted)} accepted, {len(rejected)} rejected, "
            f"{queries} query, final {final['action']}; rejected candidates left no trace"
        )
    finally:
        shutil.rmtree(d, ignore_errors=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
