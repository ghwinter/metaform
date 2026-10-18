"""
域 B：用 Lean 当 oracle（慢、可信、需要批处理）。

对象 = 11..31 的整数；隐藏概念 = "是素数"。
谓词 = 整数的可判定特征；oracle = `lean check.lean` 一次算完全部真值表
（并在 Lean 里用 `decide` 证明"素数 ⟺ 与 30 互素"在这个区间上成立）。

看两件事：
  1. 循环在慢 oracle 下不变形（状态机与域 C 完全相同）；
  2. 单次 Lean 调用 ~2.6 s，如果按查询逐个调用就要 2.6 × 查询数次；
     批处理一次拿全表，成本与查询数无关。

运行：python domain_b.py
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
    """跑一次 Lean，返回 {n: (isPrime, coprime30)} 与耗时。"""
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


# ---------- 谓词（V0 里的三个） ----------

def is_even(n: int) -> bool: return n % 2 == 0
def div3(n: int) -> bool: return n % 3 == 0
def div7(n: int) -> bool: return n % 7 == 0


V0 = {"even": is_even, "div3": div3, "div7": div7}

# 诱饵：在撞车那一对（13, 25）上取相同值，因此不可能改变签名
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

    # 找一对 V0 签名相同、真假不同的对象
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
    # 诱饵只在"与撞车那一对取值相同"时才有意义；否则它会局部切开这一对，
    # 看起来有进展，之后又在别处撞车（见下面的循环，状态会退回 insufficient）。
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
