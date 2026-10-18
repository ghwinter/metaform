"""
Cost experiment: object queries vs predicate proposals — which one determines total cost?

Model (single missing predicate):
  - Target S = the extension of some attribute a*, and a* is not in the initial vocabulary V0.
  - An object query costs 1, a predicate proposal costs r, and a proposal hits a* with probability q (a miss wastes r).
  - "Identify" = know whether each object belongs to S.

Two policies:
  ask_first     first ask one representative for each undetermined signature class under V0, then propose until a hit, then fill in the remaining classes
  propose_first propose until a hit first, then ask all undetermined classes

Claim to verify (C3'): **the order of magnitude of total cost is set by proposal quality (r/q); the stopping rule is only a second-order term.**
"""
from __future__ import annotations

import random
from statistics import mean

from domain_a import Instance, make_instance


def classes(objects: list[int], attrs: list[int]) -> dict[tuple[bool, ...], list[int]]:
    out: dict[tuple[bool, ...], list[int]] = {}
    for o in objects:
        key = tuple(bool((o >> a) & 1) for a in attrs)
        out.setdefault(key, []).append(o)
    return out


def free_reps(objects: list[int], attrs: list[int], known: set[int]) -> list[int]:
    """Pick one representative for each signature class not yet covered."""
    reps = []
    for _key, members in classes(objects, attrs).items():
        if any(m in known for m in members):
            continue
        reps.append(members[0])
    return reps


def propose_until_hit(rng: random.Random, q: float) -> int:
    n = 0
    while True:
        n += 1
        if rng.random() < q:
            return n


def total_cost(inst: Instance, policy: str, r: float, q: float, rng: random.Random) -> float:
    known: set[int] = set(inst.pos) | set(inst.neg)
    cost = 0.0
    if policy == "ask_first":
        reps = free_reps(inst.objects, inst.attrs, known)
        known |= set(reps)
        cost += len(reps)
    cost += propose_until_hit(rng, q) * r
    attrs1 = inst.attrs + [inst.extra]
    cost += len(free_reps(inst.objects, attrs1, known))
    return cost


def main() -> None:
    rng = random.Random(41)
    insts = [i for i in (make_instance(rng, 14, 8) for _ in range(300)) if i]
    print(f"instances={len(insts)}  objects≈14  attrs=8\n")
    for q in (1 / 8, 1 / 4, 1 / 2):
        print(f"q={q:.3f} （平均 {1/q:.1f} 次提议命中）")
        for r in (5, 20, 100):
            a = mean(total_cost(i, "ask_first", r, q, rng) for i in insts)
            b = mean(total_cost(i, "propose_first", r, q, rng) for i in insts)
            print(f"   r={r:3d}  ask_first={a:7.2f}  propose_first={b:7.2f}  diff={b-a:+6.2f}  ({(b-a)/a*100:+.1f}%)")
        print()


if __name__ == "__main__":
    main()
