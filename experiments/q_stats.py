"""Wilson intervals and a two-proportion test for the q0/q1 measurement.

`score` prints shares; this prints the interval around each share and the contrast that decides
whether the diagnosis was worth anything. It reads `rows-*.tsv` files as written by
`local_proposer.py score`, so nothing here can disagree with what the engine judged.

    python experiments/q_stats.py tmp/phi60/rows_diagnosis.tsv tmp/phi60/rows_placebo.tsv

Two things it refuses to do: pool two arms with different instance sets without saying so, and
report a difference without its interval.
"""
import math
import sys
from pathlib import Path


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n == 0:
        return (0.0, 1.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def read_rows(path: str) -> list[dict]:
    lines = Path(path).read_text(encoding="utf-8").strip().splitlines()
    head = lines[0].split("\t")
    out = []
    for line in lines[1:]:
        cells = line.split("\t")
        row = dict(zip(head, cells))
        out.append(row)
    return out


def summarise(name: str, rows: list[dict]) -> dict:
    n = len(rows)
    zero1 = [r for r in rows if r["r1_zero"] == "1"]
    second = "r2_splits" in rows[0] and rows[0]["r2_splits"] != ""
    recovered = [r for r in zero1 if second and r["r2_splits"] not in ("", "0")]
    lo_q0, hi_q0 = wilson(len(zero1), n)
    print(f"{name}")
    print(f"  n                    {n}")
    print(f"  q0  (splits==0)      {len(zero1)}/{n} = {len(zero1) / n:.3f}  "
          f"95% CI [{lo_q0:.3f}, {hi_q0:.3f}]")
    if not second:
        return {"n": n, "zero": len(zero1), "rec": None}
    if zero1:
        lo, hi = wilson(len(recovered), len(zero1))
        print(f"  recovery             {len(recovered)}/{len(zero1)} = "
              f"{len(recovered) / len(zero1):.3f} 95% CI [{lo:.3f}, {hi:.3f}]")
    return {"n": n, "zero": len(zero1), "rec": len(recovered)}


def two_prop(k1: int, n1: int, k2: int, n2: int) -> float:
    if n1 == 0 or n2 == 0:
        return 1.0
    p = (k1 + k2) / (n1 + n2)
    se = math.sqrt(p * (1 - p) * (1 / n1 + 1 / n2))
    if se == 0:
        return 1.0
    z = (k1 / n1 - k2 / n2) / se
    # two-sided normal tail
    return math.erfc(abs(z) / math.sqrt(2))


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    a = summarise(Path(sys.argv[1]).stem, read_rows(sys.argv[1]))
    print()
    b = summarise(Path(sys.argv[2]).stem, read_rows(sys.argv[2]))
    if a["rec"] is None or b["rec"] is None:
        return 0
    print()
    p = two_prop(a["rec"], a["zero"], b["rec"], b["zero"])
    d = a["rec"] / a["zero"] - b["rec"] / b["zero"]
    lo1, hi1 = wilson(a["rec"], a["zero"])
    lo2, hi2 = wilson(b["rec"], b["zero"])
    print(f"contrast  recovery(a) - recovery(b) = {d:+.3f}")
    print(f"          a CI [{lo1:.3f}, {hi1:.3f}]   b CI [{lo2:.3f}, {hi2:.3f}]")
    print(f"          two-proportion p = {p:.3f}"
          f"{'   (not distinguishable)' if p > 0.05 else '   (distinguishable)'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
