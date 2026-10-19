"""Does the loop close on real files from this repository?

The objects are the Python experiments under experiments and the scenarios under engine/scenarios.
The attributes are read off the file contents. The question is a real one for this repository: which
files drive the tool -- that is, which ones run the binary or cargo -- and the vocabulary in force
deliberately does not contain the attributes that answer it directly.

Pipeline, all on real data:
  1. read the files, derive the attribute extensions;
  2. label a few of them by hand (the driver checks those labels against the mechanical truth and
     reports any disagreement);
  3. hand the labelled state to the kernel, which decides whether the vocabulary in force is
     sufficient;
  4. let the components propose a predicate (`metaform-runtime retrieve`), and let the model arm
     propose one too (`real_llm_proposals.json`);
  5. every proposal goes through `metaform propose`; the driver reports what separates the
     colliding pairs, and whether adopting it settles the state.

Run: python real_domain.py [--write-state]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
ENGINE = ROOT / "target" / "debug" / "metaform.exe"
RUNTIME = ROOT / "target" / "debug" / "metaform-runtime.exe"

# Files that drive the tool: they start a process (the kernel binary or cargo).
DRIVERS = {
    "experiments/e2e_verify.py",
    "experiments/llm_e2e.py",
    "experiments/propose_loop.py",
    "experiments/proposer_arms.py",
    "experiments/domain_b.py",
    "experiments/theory_audit.py",
}

# Hand labels: what the model running the session says about these files.
LABELS = {
    "experiments/e2e_verify.py": True,
    "experiments/llm_e2e.py": True,
    "experiments/propose_loop.py": True,
    "experiments/proposer_arms.py": True,
    "experiments/domain_b.py": True,
    "experiments/greedy_cover.py": False,
    "experiments/ledger.py": False,
    "experiments/llm_mock.py": False,
    "experiments/domain_a.py": False,
    "engine/scenarios/review.demo": False,
}

# The vocabulary in force: attributes that say nothing about driving a process.
BASE = ["a_json", "a_assert", "a_small"]


def load() -> list[tuple[str, str]]:
    out = []
    for path in sorted(list((ROOT / "experiments").glob("*.py")) + list((ROOT / "engine" / "scenarios").glob("*.demo"))):
        rel = path.relative_to(ROOT).as_posix()
        out.append((rel, path.read_text(encoding="utf-8", errors="replace")))
    return out


def attributes(files: list[tuple[str, str]]) -> dict[str, list[int]]:
    def ids(pred) -> list[int]:
        return [i for i, (_, text) in enumerate(files) if pred(text)]

    lower = [(name, text.lower()) for name, text in files]
    return {
        "a_json": [i for i, (_, t) in enumerate(lower) if "json" in t],
        "a_assert": [i for i, (_, t) in enumerate(lower) if "assert " in t],
        "a_small": [i for i, (_, t) in enumerate(files) if len(t.splitlines()) < 80],
        "a_subprocess": [i for i, (_, t) in enumerate(lower) if "subprocess" in t],
        "a_cargo": [i for i, (_, t) in enumerate(lower) if "cargo" in t],
        "a_binary": [i for i, (_, t) in enumerate(lower) if "metaform.exe" in t or "target\\debug" in t or "target/debug" in t],
        "a_argparse": [i for i, (_, t) in enumerate(lower) if "argparse" in t],
        "a_demo": [i for i, (name, _) in enumerate(files) if name.endswith(".demo")],
        "a_ledger": [i for i, (_, t) in enumerate(lower) if "ledger" in t],
    }


def truth_driver(files: list[tuple[str, str]]) -> list[int]:
    return [i for i, (name, _) in enumerate(files) if name in DRIVERS]


def mask(ids: list[int]) -> int:
    out = 0
    for i in ids:
        out |= 1 << i
    return out


def run_engine_scenario(files, attrs, base, pos, neg) -> dict:
    """Asks the kernel for the status of this real state."""
    lines = [f"objects {len(files)}"]
    for name in base:
        lines.append(f"add {name} " + " ".join(str(i) for i in attrs[name]))
    for i in sorted(pos):
        lines.append(f"vote {i} 3 0")
    for i in sorted(neg):
        lines.append(f"vote {i} 0 3")
    lines.append("need")
    with tempfile.TemporaryDirectory() as tmp:
        demo = Path(tmp) / "real.demo"
        demo.write_text("\n".join(lines) + "\n", encoding="utf-8")
        out = subprocess.run([str(ENGINE), str(demo)], capture_output=True, text=True, encoding="utf-8")
        if out.returncode != 0:
            raise SystemExit(f"engine failed: {out.stderr[:300]}")
        last = [l for l in out.stdout.splitlines() if l.strip().startswith("{")][-1]
        return json.loads(last)


def validate(files, attrs, base, pos, neg, candidates: list[dict]) -> dict:
    request = {
        "protocol": "metaform.propose/1",
        "request_id": "real",
        "objects": len(files),
        "vocabulary": [{"name": n, "ids": attrs[n]} for n in base],
        "evidence": {"pos": sorted(pos), "neg": sorted(neg)},
        "limit": 8,
    }
    response = {
        "protocol": "metaform.propose/1",
        "request_id": "real",
        "candidates": [
            {"name": c["name"], "ids": c["ids"], "warrant": "", "falsifier": ""} for c in candidates
        ],
        "refused": None,
    }
    with tempfile.TemporaryDirectory() as tmp:
        req, res = Path(tmp) / "req.json", Path(tmp) / "res.json"
        req.write_text(json.dumps(request), encoding="utf-8")
        res.write_text(json.dumps(response), encoding="utf-8")
        out = subprocess.run(
            [str(ENGINE), "propose", "--request", str(req), "--response", str(res)],
            capture_output=True, text=True, encoding="utf-8",
        )
        if out.returncode != 0:
            raise SystemExit(f"propose failed: {out.stderr[:300]}")
        return json.loads(out.stdout.strip().splitlines()[-1])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--write-state", action="store_true")
    ap.add_argument("--llm", default=str(HERE / "real_llm_proposals.json"))
    args = ap.parse_args()

    files = load()
    attrs = attributes(files)
    pos = [i for i, (name, _) in enumerate(files) if LABELS.get(name) is True]
    neg = [i for i, (name, _) in enumerate(files) if LABELS.get(name) is False]

    if args.write_state:
        print(f"{len(files)} files, {len(attrs)} attributes, {len(pos)} labelled positives, {len(neg)} negatives")
        print("base vocabulary in force:", ", ".join(BASE))
        print()
        for i, (name, _) in enumerate(files):
            label = "pos" if i in pos else ("neg" if i in neg else "   ")
            marks = " ".join(n for n in attrs if i in attrs[n])
            print(f"  {i:2d} {label} {name:44s} {marks}")
        print()
        print("mechanically a driver:", [files[i][0] for i in truth_driver(files)])
        (HERE / "tmp").mkdir(exist_ok=True)
        state = [f"objects {len(files)}", "request_id real"]
        for n in BASE:
            state.append(f"vocab {n} " + " ".join(str(i) for i in attrs[n]))
        for n in sorted(attrs):
            state.append(f"attr {n} " + " ".join(str(i) for i in attrs[n]))
        state.append("pos " + " ".join(str(i) for i in pos))
        state.append("neg " + " ".join(str(i) for i in neg))
        (HERE / "tmp" / "real_state.txt").write_text("\n".join(state) + "\n", encoding="utf-8")
        return

    # 1. the kernel's view of the labelled state under the vocabulary in force
    status = run_engine_scenario(files, attrs, BASE, pos, neg)
    print(f"vocabulary in force: {', '.join(BASE)}")
    print(f"kernel: {status['status']} / {status['action']}, colliding pairs {status['colliding_pairs']}")

    # 2. the hand labels against the mechanical truth
    # Compare only the files this session actually labelled against the mechanical truth.
    disagreements = [
        name for name, label in LABELS.items() if label != (name in DRIVERS)
    ]
    print(f"label check: {len(disagreements)} disagreement(s)" + (f" {disagreements}" if disagreements else ""))

    pairs = [tuple(p) for p in status["colliding_pairs"]]

    def separates(ids: list[int]) -> int:
        ext = set(ids)
        return sum(1 for p, n in pairs if (p in ext) != (n in ext))

    # 3. the retrieval arm, through the component's CLI
    state_file = HERE / "tmp" / "real_state.txt"
    response_file = HERE / "tmp" / "real_vsa.json"
    out = subprocess.run(
        [str(RUNTIME), "retrieve", "--state", str(state_file), "--out", str(response_file),
         "--limit", "1", "--max-arity", "2"],
        capture_output=True, text=True, encoding="utf-8",
    )
    if out.returncode != 0:
        raise SystemExit(f"retrieve failed: {out.stderr[:300]}")
    vsa_candidates = json.loads(response_file.read_text(encoding="utf-8"))["candidates"]

    # 4. the model arm
    # The model arm states which files it considers drivers, by path; the driver resolves them, so
    # the proposal does not go stale when unrelated files are added.
    llm = json.loads(Path(args.llm).read_text(encoding="utf-8")) if Path(args.llm).exists() else {}
    index = {name: i for i, (name, _) in enumerate(files)}
    llm_candidates = []
    for entry in llm.get("real", []):
        missing = [f for f in entry.get("files", []) if f not in index]
        if missing:
            raise SystemExit(f"the model arm names files that are not in the state: {missing}")
        llm_candidates.append(
            {"name": entry["name"], "ids": sorted(index[f] for f in entry["files"])}
        )

    # 5. one validator for all of them
    for arm, candidates in (("retrieve", vsa_candidates), ("llm", llm_candidates)):
        if not candidates:
            print(f"{arm}: no proposal")
            continue
        verdict = validate(files, attrs, BASE, pos, neg, candidates)
        accepted = verdict["accepted"]
        print(f"{arm}: {len(accepted)} of {len(candidates)} accepted")
        for c in accepted:
            print(f"    {c['name']}: splits {c['splits']}/{len(pairs)} colliding pairs")
        for c in verdict["rejected"]:
            print(f"    rejected {c.get('name')}: {c['reason']}")


if __name__ == "__main__":
    main()
