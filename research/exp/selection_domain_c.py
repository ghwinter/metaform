"""
Selection comparison on domain C: real features, real oracle.

Two scenarios:
  one_atom   the pool holds exactly one sufficient predicate (overlap); the three decoys have coverage 0.
  two_atoms  two predicates are needed: strict_overlap (proper overlap) and touch (endpoints touching).
             neither one alone makes the target expressible.

Two stopping rules:
  inherit  stop as soon as every equivalence class has a label (labels "inherit" within a class).
  probe    before stopping, probe one member of every class that has several members and was never asked directly; if the answer conflicts with the class label,
           the status falls back to insufficient and proposals continue.

Fixed = pool order; random = mean over 200 seeds; greedy = re-pick by coverage at each step.

Run: python selection_domain_c.py
"""
from __future__ import annotations

import itertools
import random

from engine import Explorer, Status
from domain_c import CASES, DECOYS, NEEDED, PREDICATES, mask_of, target

N = len(CASES)
TARGET_MASK = mask_of(target)


def p_strict_overlap(c) -> bool:
    for (a1, b1), (a2, b2) in itertools.combinations(c, 2):
        if max(a1, a2) < min(b1, b2):
            return True
    return False


def p_touch(c) -> bool:
    for i, j in itertools.permutations(range(len(c)), 2):
        if c[i][1] == c[j][0]:
            return True
    return False


def initial() -> Explorer:
    e = Explorer(N)
    for name, fn in PREDICATES.items():
        e.propose(name, mask_of(fn))
    for i, j in itertools.combinations(range(N), 2):
        if e.signature(i) == e.signature(j) and target(CASES[i]) != target(CASES[j]):
            pos, neg = (i, j) if target(CASES[i]) else (j, i)
            e.answer(pos, True)
            e.answer(neg, False)
            return e
    raise AssertionError("setup failed: the initial vocabulary separates positives from negatives")


def scenario_masks(name: str) -> dict[str, int]:
    masks = {n: mask_of(f) for n, f in DECOYS.items()}
    if name == "one_atom":
        masks[NEEDED[0]] = mask_of(NEEDED[1])
    else:
        masks["strict_overlap"] = mask_of(p_strict_overlap)
        masks["touch"] = mask_of(p_touch)
    return masks


def colliding(e: Explorer) -> list[tuple[int, int]]:
    out = []
    for x in range(N):
        if not (e.pos >> x) & 1:
            continue
        for y in range(N):
            if (e.neg >> y) & 1 and e.signature(x) == e.signature(y):
                out.append((x, y))
    return out


def coverage(e: Explorer, mask: int) -> int:
    return sum(1 for (x, y) in colliding(e) if ((mask >> x) & 1) != ((mask >> y) & 1))


def target_definable(e: Explorer) -> bool:
    for c in e.classes():
        vals = {target(CASES[x]) for x in range(N) if (c >> x) & 1}
        if len(vals) > 1:
            return False
    return True


def play(e: Explorer, masks: dict[str, int], strategy: str, probe: bool, seed: int = 0) -> dict:
    rng = random.Random(seed)
    remaining = list(masks)
    asked: set[int] = set()
    proposals = queries = 0
    while True:
        st = e.status()
        if st is Status.INSUFFICIENT:
            if not remaining:
                break
            if strategy == "fixed":
                idx = 0
            elif strategy == "random":
                idx = rng.randrange(len(remaining))
            else:
                idx = max(range(len(remaining)), key=lambda i: coverage(e, masks[remaining[i]]))
            name = remaining.pop(idx)
            e.propose(name, masks[name])
            proposals += 1
            continue
        if st is Status.UNDETERMINED:
            x = e.next_question()
            e.answer(x, target(CASES[x]))
            asked.add(x)
            queries += 1
            continue
        # IDENTIFIED: optional probe, so an inherited class label cannot hide a mismatch
        if probe:
            pick = None
            for c in e.classes():
                members = [x for x in range(N) if (c >> x) & 1]
                unasked = [x for x in members if x not in asked]
                if len(members) > 1 and unasked:
                    pick = unasked[0]
                    break
            if pick is not None:
                e.answer(pick, target(CASES[pick]))
                asked.add(pick)
                queries += 1
                continue
        break

    derived = 0
    for c in e.classes():
        if c & e.pos:
            derived |= c
    return {
        "proposals": proposals,
        "queries": queries,
        "cost": proposals * 20 + queries,
        "ok": derived == TARGET_MASK and target_definable(e),
    }


def table(name: str, probe: bool) -> dict:
    fixed = play(initial(), scenario_masks(name), "fixed", probe)
    greedy = play(initial(), scenario_masks(name), "greedy", probe)
    rand = [play(initial(), scenario_masks(name), "random", probe, s) for s in range(200)]
    mean = lambda k: sum(r[k] for r in rand) / len(rand)  # noqa: E731
    avg = {
        "proposals": mean("proposals"),
        "queries": mean("queries"),
        "cost": mean("cost"),
        "ok": sum(x["ok"] for x in rand) / len(rand),
    }
    tag = "probe" if probe else "inherit"
    print(f"\n[{name} / {tag}]")
    print(f"{'order':8s} {'props':>6s} {'asks':>6s} {'cost':>7s} {'ok':>6s}")
    for label, r in [("fixed", fixed), ("random", avg), ("greedy", greedy)]:
        print(f"{label:8s} {r['proposals']:6.2f} {r['queries']:6.2f} {r['cost']:7.2f} {str(r['ok']):>6s}")
    return {"fixed": fixed, "random": avg, "greedy": greedy}


def main() -> None:
    one_i = table("one_atom", probe=False)
    one_p = table("one_atom", probe=True)
    two_i = table("two_atoms", probe=False)
    two_p = table("two_atoms", probe=True)

    assert one_i["greedy"]["ok"] and one_i["greedy"]["proposals"] == 1
    assert two_i["greedy"]["ok"] is False, "without probing, the two-atom scenario stops with a wrong extension"
    assert two_p["greedy"]["ok"] is True, "probing should restore correctness"
    assert two_p["greedy"]["proposals"] == 2, "two-atom greedy should take two steps"
    print("\nOK: one atom takes 1 step; two atoms give a wrong extension without probing, and probing restores correctness in 2 steps")


if __name__ == "__main__":
    main()
