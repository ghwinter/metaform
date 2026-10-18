"""
域 C 上的选择对照：真实特征、真实 oracle。

两个场景：
  one_atom   池子里只有一个充分谓词（overlap），三个诱饵覆盖为 0。
  two_atoms  需要两个谓词：strict_overlap（真重叠）与 touch（端点相接）。
             任一单独都不足以让目标可表达。

两种停止规则：
  inherit  只要所有等价类都有标签就停（标签会在类内"继承"）。
  probe    停之前，对"多成员且没被直接问过"的类各探一个成员；若答案与类标签冲突，
           状态会重新变成不足，继续提议。

固定 = 池子顺序；随机 = 200 个 seed 取均值；贪心 = 每步按覆盖重选。

运行：python selection_domain_c.py
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
    raise AssertionError("构造失败：初始词汇分得开正反例")


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
        # IDENTIFIED：可选的探测，防止类标签被"继承"掩盖错配
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
    print(f"{'顺序':8s} {'提议':>6s} {'提问':>6s} {'成本':>7s} {'正确':>6s}")
    for label, r in [("固定", fixed), ("随机", avg), ("贪心", greedy)]:
        print(f"{label:8s} {r['proposals']:6.2f} {r['queries']:6.2f} {r['cost']:7.2f} {str(r['ok']):>6s}")
    return {"fixed": fixed, "random": avg, "greedy": greedy}


def main() -> None:
    one_i = table("one_atom", probe=False)
    one_p = table("one_atom", probe=True)
    two_i = table("two_atoms", probe=False)
    two_p = table("two_atoms", probe=True)

    assert one_i["greedy"]["ok"] and one_i["greedy"]["proposals"] == 1
    assert two_i["greedy"]["ok"] is False, "不探测时，两原子场景会以错误外延停机"
    assert two_p["greedy"]["ok"] is True, "探测之后应当恢复正确"
    assert two_p["greedy"]["proposals"] == 2, "两原子贪心应当两步"
    print("\nOK：单原子 1 步；两原子在不探测时得到错误外延，探测后 2 步恢复正确")


if __name__ == "__main__":
    main()
