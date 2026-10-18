"""
Bilingual alignment check: English is authoritative, Chinese is the translation.
This script checks whether the translation has quietly drifted.

It compares four things:
  1. section count and order
  2. environment counts (theorem / definition / figure / table / axis)
  3. the set of citation keys used
  4. whether the key numbers in the paper appear on both sides

It does not judge translation quality; it only catches mechanical problems such as
missing structure or numbers.

Run: python scripts/align-papers.py
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EN = ROOT / "paper" / "en" / "main.tex"
ZH = ROOT / "paper" / "zh" / "main.tex"

ENVS = ["theorem", "definition", "figure", "table", "tabular", "axis"]
KEYS = [
    "2.01", "1.91", "6.14", "2.10", "1.85", "4.28",
    "0.06", "0.07", "0.23", "0.09",
    "27.6", "49.4", "88.9", "162.7", "7.6",
    "3.11", "3.50", "3.60", "57.2",
    "$28$", "$88$", "$51$", "$111$", "2.5", "12.6",
    r"2.5\%", r"29.5\%", r"98.7\%", r"0.5\%", r"8\%",
    "r/q", "300/300",
]


def sections(t: str) -> list[str]:
    return re.findall(r"\\section\{([^}]*)\}", t)


def env_counts(t: str) -> dict[str, int]:
    return {e: len(re.findall(r"\\begin\{" + e + r"\}", t)) for e in ENVS}


def cites(t: str) -> set[str]:
    out: set[str] = set()
    for m in re.findall(r"\\cite\{([^}]*)\}", t):
        out |= {k.strip() for k in m.split(",")}
    return out


LEAN = ROOT / "research" / "lean" / "QueryDriven.lean"


def lean_table_names(tex: str) -> list[str]:
    """Lean names listed in the appendix table."""
    m = re.search(r"\\midrule(.*?)\\bottomrule", tex, re.S)
    if not m:
        return []
    raw = re.findall(r"\\texttt\{([A-Za-z0-9_\\]+)\}", m.group(1))
    return [n.replace("\\_", "_") for n in raw]


def main() -> None:
    en = EN.read_text(encoding="utf-8")
    zh = ZH.read_text(encoding="utf-8")
    problems = 0

    se, sz = sections(en), sections(zh)
    print(f"sections: en={len(se)} zh={len(sz)}")
    if len(se) != len(sz):
        problems += 1
        print("  ! 章节数不一致")
    for i, (a, b) in enumerate(zip(se, sz)):
        print(f"  {i}: {a[:36]:38s} | {b[:26]}")

    ee, ez = env_counts(en), env_counts(zh)
    print(f"environments: en={ee}")
    print(f"              zh={ez}")
    for k in ENVS:
        if ee[k] != ez[k]:
            problems += 1
            print(f"  ! {k}: en={ee[k]} zh={ez[k]}")

    ce, cz = cites(en), cites(zh)
    print(f"citations: en={len(ce)} zh={len(cz)}")
    if ce != cz:
        problems += 1
        print(f"  ! 差异: {sorted(ce ^ cz)}")

    missing = [k for k in KEYS if (k in en) != (k in zh)]
    print(f"key numbers: {len(KEYS) - len(missing)}/{len(KEYS)} 两边一致")
    if missing:
        problems += 1
        print(f"  ! 只在一侧出现: {missing}")

    names = lean_table_names(en)
    if not names:
        problems += 1
        print("  ! 附录表里没解析到 Lean 名称")
    else:
        lean = LEAN.read_text(encoding="utf-8")
        absent = [n for n in names if f"theorem {n}" not in lean and f"def {n}" not in lean]
        print(f"lean table: {len(names)} 个名称，缺失 {len(absent)}")
        if absent:
            problems += 1
            print(f"  ! 附录表列了但 Lean 文件里没有: {absent}")

    if problems:
        print(f"\n{problems} 处结构漂移")
        sys.exit(1)
    print("\n结构与数字对齐")


if __name__ == "__main__":
    main()
