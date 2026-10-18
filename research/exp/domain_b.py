"""
Domain B: use Lean as the oracle (slow, trustworthy, batch-required).

Objects = integers 11..31; hidden concept = "is prime".
Predicates = decidable features of the integers; oracle = `lean check.lean` computes the whole truth table at once
(and proves in Lean, via `decide`, that "prime ⟺ coprime to 30" holds on this interval).

Looks at two things:
  1. the loop does not change shape under a slow oracle (the state machine is identical to domain C);
  2. a single Lean call is ~2.6 s, so one call per query would cost 2.6 × the number of queries;
     batching fetches the whole table once, at a cost independent of the query count.

Run: python domain_b.py
"""
from __future__ import annotations

import itertools
import shutil
import subprocess
import time
from pathlib import Path

from engine import Explorer, Status

HERE = Path(__file__).resolve().parent
LEAN_DIR = HERE / "lean_oracle"


def find_lean() -> str:
    exe = shutil.which("lean")
    if exe:
        return exe
    fallback = Path(r"C:\Users\xzxxn\.elan\bin\lean.exe")
    if fallback.exists():
        return str(fallback)
    raise RuntimeError("找不到 lean.exe")


def lean_truth_table() -> tuple[dict[int, tuple[bool, bool]], float]:
    """Run Lean once and return {n: (isPrime, coprime30)} plus the elapsed time."""
    lean = find_lean()
    t0 = time.perf_counter()
    proc = subprocess.run(
        [lean, str(LEAN_DIR / "check.lean")],
        capture_output=True,
        text=True,
        encoding="utf-8",
        cwd=str(LEAN_DIR),
        timeout=600,
    )
    dt = time.perf_counter() - t0
    if proc.returncode != 0:
        raise RuntimeError(f"lean failed:\n{proc.stdout}\n{proc.stderr}")

    table: dict[int, tuple[bool, bool]] = {}
    for line in proc.stdout.splitlines():
        if not line.startswith("ROW "):
            continue
        _, n, p, c = line.split()
        table[int(n)] = (p == "true", c == "true")
    return table, dt


# ---------- predicates (the three in V0) ----------

def is_even(n: int) -> bool: return n % 2 == 0
def div3(n: int) -> bool: return n % 3 == 0
def div7(n: int) -> bool: return n % 7 == 0


V0 = {"even": is_even, "div3": div3, "div7": div7}

# decoy: takes the same value on the colliding pair (13, 25), so it cannot change the signature
DECOYS = {"odd": lambda n: n % 2 == 1,
          "gt_11": lambda n: n > 11,
          "div_11": lambda n: n % 11 == 0}


def main() -> None:
    table, lean_secs = lean_truth_table()
    objs = sorted(table)
    n = len(objs)
    idx = {o: i for i, o in enumerate(objs)}
    target_bits = sum(1 << idx[o] for o in objs if table[o][0])
    print(f"oracle: lean  {lean_secs:.2f}s   rows={n}   正例 {bin(target_bits).count('1')}/{n}")

    e = Explorer(n)
    for name, fn in V0.items():
        e.propose(name, sum(1 << idx[o] for o in objs if fn(o)))

    # find a pair of objects with the same V0 signature but different truth values
    pair = None
    for a, b in itertools.combinations(objs, 2):
        if e.signature(idx[a]) == e.signature(idx[b]) and table[a][0] != table[b][0]:
            pair = (a, b)
            break
    assert pair, "初始词汇竟然分得开正反例"
    a, b = pair
    pos, neg = (a, b) if table[a][0] else (b, a)
    e.answer(idx[pos], True)
    e.answer(idx[neg], False)
    print(f"seed: +{pos}  -{neg}   V0 签名相同 → {e.status().value}")

    proposals = 0
    queries = 0
    # A decoy only makes sense when it takes the same value on the colliding pair; otherwise it splits that pair locally,
    # which looks like progress until a collision appears elsewhere (see the loop below, the status falls back to insufficient).
    decoys = [(k, f) for k, f in DECOYS.items() if f(pos) == f(neg)]
    pool = decoys + [("coprime30", None)]
    print(f"可用诱饵: {[k for k, _ in decoys]}")

    steps = 0
    while e.status() is not Status.IDENTIFIED:
        steps += 1
        assert steps < 100, "循环没有收敛"
        if e.status() is Status.INSUFFICIENT:
            if not pool:
                print("  ! 提议池耗尽，仍未脱离不足")
                break
            name, fn = pool.pop(0)
            if fn is None:
                mask = sum(1 << idx[o] for o in objs if table[o][1])
            else:
                mask = sum(1 << idx[o] for o in objs if fn(o))
            e.propose(name, mask)
            proposals += 1
            print(f"  propose #{proposals}: {name:10s} → {e.status().value}")
        else:
            x = e.next_question()
            e.answer(x, table[objs[x]][0])
            queries += 1

    derived = 0
    for c in e.classes():
        if c & e.pos:
            derived |= c
    ok = derived == target_bits
    print(f"\nidentified: 提议 {proposals}，查询 {queries}")
    print(f"外延 == 素数列: {ok}")
    print(f"oracle 成本：批处理 1 次 = {lean_secs:.2f}s；若逐查询调用 = {queries} × {lean_secs:.2f}s "
          f"= {queries * lean_secs:.1f}s")


if __name__ == "__main__":
    main()
