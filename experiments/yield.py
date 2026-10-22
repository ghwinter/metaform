"""What the measured recovery rates are worth once steps compound.

The arms in `RESULTS.md` §28 give a per-step number: a proposer that fails `q0` of the time, and a
message that rescues `r` of those failures. A single creative session is dozens of such steps, so
what matters is not either number but their product over the chain.

    p_eff = p + (1 - p) * d * r          d = 1 when every step is judged

`yield(N) = p_eff ** N`. This prints it from the rows files, so the numbers in `framework.md` are
computed rather than transcribed.

    python experiments/yield.py
"""
from pathlib import Path

ROWS = Path(__file__).resolve().parent / "proposer-arms"


def read(path: Path) -> list[dict]:
    lines = path.read_text(encoding="utf-8").strip().splitlines()
    head = lines[0].split("\t")
    return [dict(zip(head, line.split("\t"))) for line in lines[1:]]


def rates(path: Path) -> tuple[float, float]:
    rows = read(path)
    zero = [r for r in rows if r["r1_zero"] == "1"]
    if not zero:
        return 0.0, 0.0
    recovered = [r for r in zero if r["r2_splits"] not in ("", "0")]
    return len(zero) / len(rows), len(recovered) / len(zero)


def main() -> int:
    q0, _ = rates(ROWS / "rows.placebo.tsv")
    _, r_placebo = rates(ROWS / "rows.placebo.tsv")
    _, r_pairs = rates(ROWS / "rows.pairs.tsv")
    _, r_diag = rates(ROWS / "rows.diagnosis.tsv")

    p = 1 - q0
    arms = [
        ("no engine", p),
        ("rejected only", p + q0 * r_placebo),
        ("+ explanation", p + q0 * r_diag),
        ("+ witness pair", p + q0 * r_pairs),
    ]

    print(f"q0 (per-step failure)  {q0:.3f}   p = {p:.3f}")
    print(f"recovery: placebo {r_placebo:.3f}  diagnosis {r_diag:.3f}  pairs {r_pairs:.3f}")
    print()
    print(f"{'step':<18} {'p_eff':>6} " + " ".join(f"{'N=' + str(n):>11}" for n in (5, 10, 20, 50)))
    for name, eff in arms:
        cells = " ".join(f"{eff ** n:11.2e}" for n in (5, 10, 20, 50))
        print(f"{name:<18} {eff:6.3f} {cells}")

    # The ratio that the argument turns on: the same improvement in p_eff buys more at larger N.
    print()
    base, best = arms[0][1], arms[-1][1]
    for n in (5, 10, 20, 50):
        print(f"N={n:<4} witness pair over nothing: {(best / base) ** n:8.3g}x")

    # What p_eff would be needed to hold a given yield, which is the target a design must hit.
    print()
    for target in (0.5, 0.9):
        print(f"p_eff needed for yield {target:.0%} at N=20: {target ** (1 / 20):.4f}"
              f"   (now {best:.3f})")

    # How many *independent* detectors it takes. Each one multiplies the surviving error by (1 - r),
    # which is the only reason independence matters: correlated detectors multiply the same error
    # twice and the second one buys almost nothing.
    print()
    print(f"detectors of recovery r needed to reach p_eff (each independent of the rest):")
    print(f"{'r':>6} " + " ".join(f"{'p>=' + str(t):>9}" for t in (0.95, 0.99, 0.999)))
    for r in (0.4, 0.655, 0.8):
        residual = 1 - best
        row = []
        for target in (0.95, 0.99, 0.999):
            k, e = 0, residual
            while 1 - e < target and k < 99:
                e *= 1 - r
                k += 1
            row.append(f"{k:9d}")
        print(f"{r:6.3f} " + " ".join(row))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
