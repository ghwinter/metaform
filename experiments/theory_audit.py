"""
Falsifier: attack the theorems in the paper with brute-force enumeration; only what survives is not lying.

Unlike verify_claims.py — that one checks "experimental numbers", this one checks the "theorem statements" themselves:
enumerate random formal contexts with n objects and m predicates exhaustively and look for a counterexample.

Checks:
  T1  definable ⇔ is a union of equivalence classes (brute-force over all Boolean functions)
  T2  positives and negatives colliding in the same class ⇔ the candidate set is empty
  T5  once the candidate set is empty, adding examples keeps it empty (numbered T6 in the Lean file)
  T4  the number of distinct restrictions of definable sets to the sample ≤ 2^m
  T7  every positive/negative pair is separated by some predicate ⇔ no class contains both a positive and a negative
  cover  the predicates picked greedily do separate all positive/negative pairs (sanity of the tooling)
  ops    scheduling self-check of the five operator kinds: skipping redundancy does not change Φ, savings = cost of the redundant steps; early stopping is wrong, probing is right (`metaform ops`)

Run: python theory_audit.py [--trials 300]
"""
from __future__ import annotations

import argparse
import itertools
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path


def signatures(preds: list[list[bool]], n: int) -> list[tuple]:
    return [tuple(p[x] for p in preds) for x in range(n)]


def classes_of(sigs: list[tuple]) -> list[list[int]]:
    groups: dict[tuple, list[int]] = {}
    for x, s in enumerate(sigs):
        groups.setdefault(s, []).append(x)
    return list(groups.values())


def definable_by_enumeration(S: list[bool], sigs: list[tuple]) -> bool:
    """Enumerate every Boolean function φ: sig → {0,1} and see whether one agrees with S exactly."""
    vals = sorted(set(sigs))
    for bits in itertools.product([0, 1], repeat=len(vals)):
        phi = dict(zip(vals, bits))
        if all(phi[sigs[x]] == S[x] for x in range(len(S))):
            return True
    return False


def is_union_of_classes(S: list[bool], cls: list[list[int]]) -> bool:
    return all(len({S[x] for x in c}) == 1 for c in cls)


def candidates(cls: list[list[int]], pos: set[int], neg: set[int]) -> list[frozenset[int]]:
    """Independent enumeration: among all sets assembled from equivalence classes, those consistent with the sample."""
    out = []
    for mask in range(1 << len(cls)):
        chosen = frozenset(x for i, c in enumerate(cls) if mask >> i & 1 for x in c)
        if pos <= chosen and not (chosen & neg):
            out.append(chosen)
    return out


def separate_all(preds: list[list[bool]], pos: set[int], neg: set[int]) -> bool:
    return all(any(p[x] != p[y] for p in preds) for x in pos for y in neg)


def greedy_cover(preds, pos, neg):
    pairs = [(x, y) for x in pos for y in neg]
    uncovered = set(range(len(pairs)))
    chosen = []
    while uncovered:
        best, gain = None, 0
        for i, p in enumerate(preds):
            g = sum(1 for j in uncovered if p[pairs[j][0]] != p[pairs[j][1]])
            if g > gain:
                best, gain = i, g
        if best is None:
            return None
        chosen.append(best)
        uncovered = {j for j in uncovered if preds[best][pairs[j][0]] == preds[best][pairs[j][1]]}
    return chosen


def engine_status(engine: str, preds: list[list[bool]], pos: set[int], neg: set[int]) -> str:
    """Write the random context as a scenario file, let the engine decide once, and return its status."""
    n = len(preds[0]) if preds else 0
    lines = [f"objects {n}"]
    for i, p in enumerate(preds):
        ids = " ".join(str(x) for x in range(n) if p[x])
        lines.append(f"add p{i} {ids}".rstrip())
    for x in pos:
        lines.append(f"vote {x} 3 0")
    for x in neg:
        lines.append(f"vote {x} 0 3")
    lines.append("need")
    with tempfile.NamedTemporaryFile("w", suffix=".demo", delete=False, encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
        path = fh.name
    try:
        out = subprocess.run([engine, path], capture_output=True, text=True, encoding="utf-8")
    finally:
        Path(path).unlink(missing_ok=True)
    for line in reversed(out.stdout.splitlines()):
        if line.startswith("{"):
            return json.loads(line)["status"]
    return "ERROR"


def ops_audit(engine: str) -> int:
    """Run `metaform ops`: check the self-test, the cost identities of the three scenarios, and the two assertions of the probe demo."""
    out = subprocess.run([engine, "ops"], capture_output=True, text=True, encoding="utf-8")
    try:
        doc = json.loads(out.stdout.strip().splitlines()[-1])
    except Exception:
        print("  ! ops output is not parseable")
        return 1
    bad = 0
    audit = doc.get("audit", {})
    if audit.get("violations", 1) != 0:
        bad += 1
        print(f"  ! ops self-check has violations: {audit}")
    for s in doc.get("scenarios", []):
        if s["fixed_cost"] - s["scheduled_cost"] != s["skipped_cost"]:
            bad += 1
            print(f"  ! cost identity does not hold: {s['name']}")

    # no collision ≠ correct: early stopping must be wrong, the probe version must be right, and the verified version must pay more.
    probe = doc.get("probe", {})
    if probe.get("early_phi") != 1:
        bad += 1
        print(f"  ! Phi is not 1 on early stop: {probe}")
    if probe.get("early_correct") is not False:
        bad += 1
        print(f"  ! early stop produced a correct extension (the probe demo is broken): {probe}")
    if probe.get("verified_correct") is not True:
        bad += 1
        print(f"  ! still incorrect after probing: {probe}")
    if probe.get("verified_cost", 0) <= probe.get("early_queries", 0):
        bad += 1
        print(f"  ! the verified version did not pay extra: {probe}")
    if probe.get("verified_queries") != 5 or probe.get("verified_cost") != 14:
        bad += 1
        print(f"  ! the verified version's fixed numbers changed (expected 5 queries, cost 14): {probe}")

    # two-atom scenario: greedy must succeed in two steps, fixed order must fail.
    compose = doc.get("compose", {})
    if compose.get("greedy") != 2.0 or compose.get("greedy_ok") != 1.0:
        bad += 1
        print(f"  ! greedy did not succeed in two steps: {compose}")
    if compose.get("fixed_ok") != 0.0:
        bad += 1
        print(f"  ! fixed order unexpectedly succeeded within budget: {compose}")
    if not compose.get("greedy", 9) < compose.get("random", 0):
        bad += 1
        print(f"  ! greedy is not better than random: {compose}")

    print(f"ops: {len(doc.get('scenarios', []))} scenarios + {audit.get('trials', 0)} self-checks, {bad} violations")
    return bad


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--trials", type=int, default=300)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--engine", help="qde executable; when given, its verdicts are checked against the theory")
    args = ap.parse_args()

    rng = random.Random(args.seed)
    fails = {k: 0 for k in ["T1", "T2", "T6", "T4", "T7", "cover"]}
    examples = 0

    for _ in range(args.trials):
        n = rng.randint(3, 6)
        m = rng.randint(1, 4)
        preds = [[rng.random() < 0.5 for _ in range(n)] for _ in range(m)]
        S = [rng.random() < 0.5 for _ in range(n)]
        sigs = signatures(preds, n)
        cls = classes_of(sigs)

        # T1
        if definable_by_enumeration(S, sigs) != is_union_of_classes(S, cls):
            fails["T1"] += 1

        # random sample
        pos = {i for i in range(n) if S[i] and rng.random() < 0.6}
        neg = {i for i in range(n) if not S[i] and rng.random() < 0.6}
        collision = any(any(x in pos for x in c) and any(x in neg for x in c) for c in cls)
        cands = candidates(cls, pos, neg)

        # T2: collision ⇔ candidate set is empty (the candidate set comes from independent enumeration, so this is not a tautology)
        if collision != (len(cands) == 0):
            fails["T2"] += 1

        # T6: once the candidate set is empty, adding examples keeps it empty
        if not cands:
            pos2 = pos | {i for i in range(n) if S[i] and i not in pos and rng.random() < 0.3}
            neg2 = neg | {i for i in range(n) if not S[i] and i not in neg and rng.random() < 0.3}
            if candidates(cls, pos2, neg2):
                fails["T6"] += 1

        # T4: the number of distinct restrictions on the sample ≤ 2^(number of distinct signatures)
        sample = sorted(pos | neg)
        if sample:
            restr = set()
            # enumerate all unions of classes (all definable sets) and count their restrictions to the sample
            for mask in range(1 << len(cls)):
                chosen = {x for i, c in enumerate(cls) if mask >> i & 1 for x in c}
                restr.add(tuple(x in chosen for x in sample))
            m_sig = len({sigs[x] for x in sample})
            if len(restr) > 2 ** m_sig:
                fails["T4"] += 1

        # T7
        if separate_all(preds, pos, neg) != (not collision):
            fails["T7"] += 1

        # cover: the predicates picked greedily should together separate all pairs
        if not collision and pos and neg:
            chosen = greedy_cover(preds, pos, neg)
            if chosen is None or not separate_all([preds[i] for i in chosen], pos, neg):
                fails["cover"] += 1
        examples += 1

    if args.engine:
        rng2 = random.Random(args.seed + 1)
        mism = 0
        for _ in range(25):
            n = rng2.randint(3, 7)
            m = rng2.randint(1, 3)
            preds = [[rng2.random() < 0.5 for _ in range(n)] for _ in range(m)]
            pos, neg = set(), set()
            for x in range(n):
                r = rng2.random()
                if r < 0.3:
                    pos.add(x)
                elif r < 0.6:
                    neg.add(x)
            cls = classes_of(signatures(preds, n))
            collision = any(any(x in pos for x in c) and any(x in neg for x in c) for c in cls)
            st = engine_status(args.engine, preds, pos, neg)
            if (st == "INSUFFICIENT") != collision:
                mism += 1
                print(f"  ! engine disagrees with the theory: status={st} collision={collision}")
        print(f"engine: 25 random scenarios, {mism} mismatches")
        fails["engine"] = mism
        fails["ops"] = ops_audit(args.engine)

    print(f"trials={examples}  seed={args.seed}")
    for k, v in fails.items():
        print(f"  {k:6s} counterexamples {v}")
    if any(fails.values()):
        print("\na theorem was knocked down: the statement or the implementation is wrong")
        sys.exit(1)
    print(f"\n{len(fails)} checks, zero counterexamples in all")


if __name__ == "__main__":
    main()


