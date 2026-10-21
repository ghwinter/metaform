"""Three rates for the closed-world claim, computed from files rather than from a model.

readings  -- a table of candidate readings, each with an extension, and which of them the user
             actually stated. Reports:
               complement coverage  = non-stated readings whose extension differs from every stated one
               axis equivalence rate = non-stated readings whose extension equals a stated one
             A high equivalence rate means the "new angles" were rewordings.

bets      -- a predictions table (the engine's own format). Reports the divergence rate: the share of
             recorded bets whose predicted object sits where the candidate and its rival disagree.
             A bet on which every candidate agrees could not have failed and carries no information.

Both are pure functions over files; neither needs a model or a network.
"""
import argparse
import json
import sys


def parse_extension(payload, vocab):
    if payload.startswith("ids:"):
        return set(int(x) for x in payload[4:].split(",") if x.strip())
    op, _, names = payload.partition(":")
    names = [n for n in names.split(",") if n.strip()]
    sets = [vocab[n] for n in names]
    if op == "all_of":
        out = set(sets[0])
        for s in sets[1:]:
            out &= s
        return out
    if op == "any_of":
        out = set()
        for s in sets:
            out |= s
        return out
    raise ValueError(f"unknown reading kind: {payload}")


def load_vocab(path):
    vocab = {}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            name, ids = line.split("\t")
            vocab[name] = set(int(x) for x in ids.split(",") if x.strip())
    return vocab


def load_readings(path, vocab):
    rows = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            name, payload, stated = line.split("\t")
            rows.append({"name": name, "ext": parse_extension(payload, vocab), "stated": stated == "yes"})
    return rows


def readings_rates(rows):
    stated = [r for r in rows if r["stated"]]
    other = [r for r in rows if not r["stated"]]
    if not other:
        return {"readings": 0}
    equivalent = [r for r in other if any(r["ext"] == s["ext"] for s in stated)]
    complement = [r for r in other if not any(r["ext"] == s["ext"] for s in stated)]
    return {
        "readings": len(other),
        "stated": len(stated),
        "complement": len(complement),
        "complement_coverage": round(len(complement) / len(other), 3),
        "axis_equivalence_rate": round(len(equivalent) / len(other), 3),
        "equivalent_to": {r["name"]: next(s["name"] for s in stated if s["ext"] == r["ext"])
                          for r in equivalent},
        "complement_names": [r["name"] for r in complement],
    }


def bets_rate(path):
    total = divergent = 0
    with open(path, encoding="utf-8") as fh:
        header = fh.readline().rstrip("\n").split("\t")
        idx = {name: i for i, name in enumerate(header)}
        for line in fh:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            cells = line.split("\t")
            # A file written before the rival column exists still counts: its bets simply had no
            # rival, so they are non-divergent rather than absent.
            rival = cells[idx["rival"]].strip() if "rival" in idx and len(cells) > idx["rival"] else ""
            total += 1
            if not rival:
                continue
            predicted = cells[idx["id"]]
            in_ext = predicted in cells[idx["extension"]].split(",")
            in_rival = predicted in rival.split(",")
            if in_ext != in_rival:
                divergent += 1
    return {"bets": total, "divergent": divergent,
            "bet_divergence_rate": round(divergent / total, 3) if total else 0.0}


SELFTEST = """stated-a\tall_of:a,b\tyes
reword-of-a\tall_of:b,a\tno
new-wider\tany_of:a,c\tno
new-narrow\tall_of:a,c\tno
"""


def selftest():
    import os
    import tempfile
    vocab = {"a": {0, 1}, "b": {1, 2}, "c": {2, 3}}
    d = tempfile.mkdtemp()
    vp = os.path.join(d, "vocab.tsv")
    rp = os.path.join(d, "readings.tsv")
    with open(vp, "w", encoding="utf-8") as fh:
        for k, v in vocab.items():
            fh.write(f"{k}\t{','.join(str(x) for x in sorted(v))}\n")
    with open(rp, "w", encoding="utf-8") as fh:
        fh.write(SELFTEST)
    rates = readings_rates(load_readings(rp, load_vocab(vp)))
    ok = (rates["readings"] == 3 and rates["complement"] == 2
          and rates["axis_equivalence_rate"] == 0.333 and rates["complement_coverage"] == 0.667)
    print("[readings] self-test", "ok" if ok else f"FAILED {rates}")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", nargs="?", choices=["readings", "bets"])
    ap.add_argument("--vocab")
    ap.add_argument("--readings")
    ap.add_argument("--predictions")
    ap.add_argument("--selftest", action="store_true")
    args = ap.parse_args()
    if args.selftest:
        return selftest()
    if args.mode == "readings":
        vocab = load_vocab(args.vocab)
        print(json.dumps(readings_rates(load_readings(args.readings, vocab)), indent=1))
        return 0
    if args.mode == "bets":
        print(json.dumps(bets_rate(args.predictions), indent=1))
        return 0
    return print("usage: readings.py readings|bets ... | --selftest") or 2


if __name__ == "__main__":
    sys.exit(main())
