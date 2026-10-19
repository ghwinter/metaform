"""Three proposers, one validator, three task families.

Every arm sees the same state -- objects, attributes, the vocabulary in force, and four labelled
examples -- and may offer one proposal. All proposals go through `metaform propose`, and the driver
counts how many separate every pair that the *true* labels make indistinguishable under the
vocabulary in force.

Families:
  k2     the target is a conjunction of two attributes, the vocabulary holds one of them;
  k3     the target is a conjunction of three attributes, the vocabulary holds one of them;
  noisy  like k2, but one evidence label is wrong.

Arms:
  random  draws an attribute;
  vsa     ranks conjunctions of up to the family's arity by vector similarity (crates/vsa, through
          crates/runtime's CLI);
  llm     the proposals written by the model driving the session, in research/exp/llm_proposals.json.

Run: python proposer_arms.py --family k2 [--instances 5] [--write-requests]
"""
from __future__ import annotations

import argparse
import json
import random
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEFAULT_ENGINE = ROOT / "engine" / "target" / "debug" / "metaform.exe"
DEFAULT_RUNTIME = ROOT / "crates" / "runtime" / "target" / "debug" / "metaform-runtime.exe"


def arity(family: str) -> int:
    return 3 if family == "k3" else 2


def instance(seed: int, family: str, n_obj: int = 12, n_attr: int = 8):
    """One instance. The vocabulary in force holds only the first target attribute, so a labelled
    positive and a labelled negative always share a signature."""
    rng = random.Random(seed)
    size = arity(family)
    while True:
        world = [[rng.random() < 0.5 for _ in range(n_attr)] for _ in range(n_obj)]
        target = rng.sample(range(n_attr), size)
        x = target[0]
        pos_true = [i for i in range(n_obj) if all(world[i][a] for a in target)]
        neg_true = [i for i in range(n_obj) if world[i][x] and not all(world[i][a] for a in target)]
        if len(pos_true) >= 2 and len(neg_true) >= 2:
            break

    ev_pos = sorted(rng.sample(pos_true, 2))
    ev_neg = sorted(rng.sample(neg_true, 2))
    noise = None
    if family == "noisy":
        flip = rng.choice(ev_pos)
        ev_pos = [i for i in ev_pos if i != flip]
        ev_neg = sorted(ev_neg + [flip])
        noise = flip

    attrs = {a: [i for i in range(n_obj) if world[i][a]] for a in range(n_attr)}
    return {
        "seed": seed,
        "family": family,
        "world": world,
        "target": target,
        "attrs": attrs,
        "base": [x],
        "evidence_pos": ev_pos,
        "evidence_neg": ev_neg,
        "true_pos": pos_true,
        "true_neg": neg_true,
        "noise": noise,
    }


def state_text(inst, n_obj: int, n_attr: int) -> str:
    lines = [f"objects {n_obj}", "request_id arm"]
    for a in inst["base"]:
        lines.append(f"vocab a{a} " + " ".join(str(i) for i in inst["attrs"][a]))
    for a in range(n_attr):
        lines.append(f"attr a{a} " + " ".join(str(i) for i in inst["attrs"][a]))
    lines.append("pos " + " ".join(str(i) for i in inst["evidence_pos"]))
    lines.append("neg " + " ".join(str(i) for i in inst["evidence_neg"]))
    return "\n".join(lines) + "\n"


def true_colliding_pairs(inst, n_obj: int):
    """Pairs that the true labels make indistinguishable under the vocabulary in force."""
    sig = {i: tuple(bool(inst["world"][i][a]) for a in inst["base"]) for i in range(n_obj)}
    return [(p, n) for p in inst["true_pos"] for n in inst["true_neg"] if sig[p] == sig[n]]


def validate(engine: Path, inst, n_obj: int, candidates: list[dict]) -> dict:
    request = {
        "protocol": "metaform.propose/1",
        "request_id": "arm",
        "objects": n_obj,
        "vocabulary": [{"name": f"a{a}", "ids": inst["attrs"][a]} for a in inst["base"]],
        "evidence": {"pos": inst["evidence_pos"], "neg": inst["evidence_neg"]},
        "limit": 8,
    }
    response = {
        "protocol": "metaform.propose/1",
        "request_id": "arm",
        "candidates": [
            {"name": c["name"], "ids": c["ids"], "warrant": "", "falsifier": ""} for c in candidates
        ],
        "refused": None,
    }
    with tempfile.TemporaryDirectory() as tmp:
        req = Path(tmp) / "req.json"
        res = Path(tmp) / "res.json"
        req.write_text(json.dumps(request), encoding="utf-8")
        res.write_text(json.dumps(response), encoding="utf-8")
        out = subprocess.run(
            [str(engine), "propose", "--request", str(req), "--response", str(res)],
            capture_output=True, text=True, encoding="utf-8",
        )
        if out.returncode != 0:
            raise SystemExit(f"propose failed: {out.stderr[:300]}")
        return json.loads(out.stdout.strip().splitlines()[-1])


def separates_all(inst, n_obj: int, candidate: dict) -> bool:
    pairs = true_colliding_pairs(inst, n_obj)
    ext = set(candidate["ids"])
    return bool(pairs) and all((p in ext) != (n in ext) for p, n in pairs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--family", choices=("k2", "k3", "noisy"), default="k2")
    ap.add_argument("--instances", type=int, default=5)
    ap.add_argument("--objects", type=int, default=12)
    ap.add_argument("--attrs", type=int, default=8)
    ap.add_argument("--seed", type=int, default=11)
    ap.add_argument("--engine", default=str(DEFAULT_ENGINE))
    ap.add_argument("--runtime", default=str(DEFAULT_RUNTIME))
    ap.add_argument("--llm", default=str(HERE / "llm_proposals.json"))
    ap.add_argument("--write-requests", action="store_true")
    args = ap.parse_args()

    engine, runtime = Path(args.engine), Path(args.runtime)
    instances = [
        instance(args.seed + k, args.family, args.objects, args.attrs) for k in range(args.instances)
    ]

    out_dir = HERE / "tmp"
    out_dir.mkdir(exist_ok=True)

    if args.write_requests:
        for k, inst in enumerate(instances):
            (out_dir / f"{args.family}_{k}.txt").write_text(
                state_text(inst, args.objects, args.attrs), encoding="utf-8"
            )
            print(f"== {args.family} instance {k} (seed {inst['seed']}) ==")
            print(f"  labelled positives: {inst['evidence_pos']}  negatives: {inst['evidence_neg']}")
            if inst["noise"] is not None:
                print(f"  one label is wrong: object {inst['noise']} is truly positive")
            for a in range(args.attrs):
                print(f"  a{a}: {inst['attrs'][a]}")
            print(f"  true colliding pairs: {true_colliding_pairs(inst, args.objects)}")
        return

    llm = json.loads(Path(args.llm).read_text(encoding="utf-8")) if Path(args.llm).exists() else {}
    arms = {"random": [0, 0], "vsa": [0, 0], "llm": [0, 0]}
    rng = random.Random(args.seed)

    for k, inst in enumerate(instances):
        state_file = out_dir / f"{args.family}_{k}.txt"
        state_file.write_text(state_text(inst, args.objects, args.attrs), encoding="utf-8")

        # random arm
        r = rng.randrange(args.attrs)
        cands = [{"name": f"a{r}", "ids": inst["attrs"][r]}]
        validate(engine, inst, args.objects, cands)
        arms["random"][0] += sum(separates_all(inst, args.objects, c) for c in cands)
        arms["random"][1] += len(cands)

        # vsa arm through the component's CLI
        response_file = out_dir / f"{args.family}_vsa_{k}.json"
        out = subprocess.run(
            [str(runtime), "retrieve", "--state", str(state_file), "--out", str(response_file),
             "--limit", "1", "--max-arity", str(arity(args.family))],
            capture_output=True, text=True, encoding="utf-8",
        )
        if out.returncode != 0:
            raise SystemExit(f"retrieve failed: {out.stderr[:300]}")
        cands = [
            {"name": c["name"], "ids": c["ids"]}
            for c in json.loads(response_file.read_text(encoding="utf-8"))["candidates"]
        ]
        validate(engine, inst, args.objects, cands)
        arms["vsa"][0] += sum(separates_all(inst, args.objects, c) for c in cands)
        arms["vsa"][1] += len(cands)

        # model arm
        entry = llm.get(f"{args.family}:{k}", [])
        cands = [{"name": e["name"], "ids": e["ids"]} for e in entry]
        if cands:
            validate(engine, inst, args.objects, cands)
            arms["llm"][0] += sum(separates_all(inst, args.objects, c) for c in cands)
            arms["llm"][1] += len(cands)

    print(f"family {args.family} ({args.instances} instances, arity {arity(args.family)})")
    print(f"{'arm':8s} {'hit rate':>9s} {'proposals':>10s} {'cost r=20':>10s}")
    for name, (good, offered) in arms.items():
        if offered == 0:
            print(f"{name:8s} {'n/a':>9s} {0:>10d} {'n/a':>10s}")
            continue
        q = good / offered
        cost = 20.0 / q + 7.6 if q > 0 else float("inf")
        print(f"{name:8s} {q:9.3f} {offered:10d} {cost:10.1f}")


if __name__ == "__main__":
    main()
