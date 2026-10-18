"""
Empirical version of T6: does giving more examples "fix" vocabulary insufficiency?

Intuition (also T6): no. More examples only add constraints; they do not change the available language.
Stronger claim: more examples **expose** vocabulary insufficiency that went unnoticed before — that is,
the probability that F goes from non-empty to empty rises with the number of examples.

Method: build a hidden target S = the extension of attribute a*, but the initial vocabulary V0 does not contain a*.
Sample for k = 1..8; each time draw ceil(k/2) positives from inside S and the rest as negatives from outside S,
and measure the fraction of runs where the consistent conjunctive hypotheses within V0 are empty.

If the no-difference phenomenon were "empty from the start", these instances would be too easy to expose;
per the problem statement, some instances are **non-empty** for small k (positives and negatives happen to have
different signatures) and only collide as k grows.
"""
from __future__ import annotations

import random

from domain_a import Instance, conj_class, make_instance
from domain_a import consistent


def main() -> None:
    rng = random.Random(31)
    insts = [i for i in (make_instance(rng, 14, 8) for _ in range(400)) if i]
    print(f"instances={len(insts)}, objects≈14, attrs=8 (a* not in V0)")

    ks = [1, 2, 3, 4, 6, 8, 10]
    print("\nk  examples    P(F empty in V0)")
    for k in ks:
        empty = 0
        total = 0
        for i in insts:
            n_pos = max(1, k // 2)
            n_neg = max(1, k - n_pos)
            pos_pool = list(i.target)
            neg_pool = [o for o in i.objects if o not in i.target]
            if len(pos_pool) < n_pos or len(neg_pool) < n_neg:
                continue
            for _ in range(20):  # repeat sampling per instance to reduce variance
                pos = set(rng.sample(pos_pool, n_pos))
                neg = set(rng.sample(neg_pool, n_neg))
                cands = conj_class(i.objects, i.attrs, 2)
                total += 1
                if not consistent(cands, pos, neg):
                    empty += 1
        print(f"{k:2d}  {k:8d}    {100*empty/total:5.1f}%   ({empty}/{total})")


if __name__ == "__main__":
    main()
