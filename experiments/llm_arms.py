"""Two arms, one model: direct answer versus the kernel's proposal loop.

The only thing that varies between arms is what the model is asked and what happens to its answer.

  Arm D (direct) sees the evidence and answers with one rule.
  Arm S (scaffolded) sees the kernel's `metaform.propose/1` request, proposes predicates, and the
  kernel adopts what separates the most colliding pairs and asks the next question. The model is
  called again with the updated state, up to a fixed number of rounds.

Both arms are scored by the same question: does the final hypothesis identify the hidden rule?
For arm D that is the rule it wrote. For arm S it is the concept the vocabulary decides: exactly
one consistent conjunction, and it is the target. The scaffolded arm may therefore fail even when
it proposed the right predicate, if the evidence still leaves another concept alive.

Nothing here talks to a model. Prompts are written to files, a model (or a person) answers them,
and `score` compares. The kernel calls go through the real `metaform` binary.

Usage:
  python llm_arms.py instances --seeds 1,2,3,4 --k 2 --out instances.json
  python llm_arms.py prompts  --instances instances.json --arm direct --out d.json
  python llm_arms.py prompts  --instances instances.json --arm scaffold --round 1 --out s1.json
  python llm_arms.py step     --instances instances.json --candidates s1_answers.json \
                              --out-instances next.json --out-prompts s2.json
  python llm_arms.py score    --instances instances.json --arm direct --answers d_answers.json
"""
from __future__ import annotations

import argparse
import json
import random
import subprocess
import tempfile
from itertools import combinations
from pathlib import Path

ATTRS = "abcdefgh"
ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ENGINE = ROOT / "target" / "debug" / "metaform.exe"


def make_instance(seed: int, k: int, objects: int = 12, attrs: int = 8) -> dict:
    rng = random.Random(seed)
    while True:
        world = [[rng.random() < 0.5 for _ in range(attrs)] for _ in range(objects)]
        target = rng.sample(range(attrs), k)
        pos = [i for i in range(objects) if all(world[i][a] for a in target)]
        neg = [i for i in range(objects) if world[i][target[0]] and not all(world[i][a] for a in target)]
        if len(pos) >= 2 and len(neg) >= 2:
            break
    return {
        "seed": seed,
        "k": k,
        "objects": objects,
        "attrs": attrs,
        "target": target,
        "target_name": "&".join(ATTRS[a] for a in target),
        "target_ext": [i for i in range(objects) if all(world[i][a] for a in target)],
        "world": world,
        "ext": {ATTRS[a]: [i for i in range(objects) if world[i][a]] for a in range(attrs)},
        "vocabulary": [ATTRS[target[0]]],
        "pos": sorted(rng.sample(pos, 2)),
        "neg": sorted(rng.sample(neg, 2)),
    }


def table(inst: dict) -> str:
    rows = [f"objects: {inst['objects']}  attributes: {inst['attrs']}"]
    for name, ids in inst["ext"].items():
        rows.append(f"attribute {name} holds for objects: {ids}")
    rows.append(f"labelled positive: {inst['pos']}")
    rows.append(f"labelled negative: {inst['neg']}")
    rows.append(f"vocabulary in force: {inst['vocabulary']} (predicates the kernel can already see)")
    return "\n".join(rows)


def direct_prompt(inst: dict) -> str:
    return (
        "You are the proposer in a concept-identification task. The table below is all the information "
        "there is. A hidden rule is a conjunction of one or more attributes; the labelled objects are "
        "positive exactly when they satisfy it.\n\n"
        f"{table(inst)}\n\n"
        "Question: which rule explains the labels? Answer with one conjunction of attribute names, "
        "for example a&c. Do not explain, do not use tools. Output only JSON: "
        '{"rule": "<conjunction>"}'
    )


def scaffold_prompt(inst: dict, history: str) -> str:
    return (
        "You are the proposer behind a kernel that keeps a vocabulary of predicates over objects. "
        "The kernel accepts a proposal only if its extension is legal, adopts the proposal that "
        "separates the most pairs of a positive and a negative object that share a signature under "
        "the vocabulary, and then asks about another object; you will be called again.\n\n"
        f"{table(inst)}\n{history}\n"
        "Propose at most four predicates. A predicate is a conjunction of attribute names; give the "
        "objects it holds for, a warrant (why it might be the missing predicate) and a falsifier (what "
        "would refute it). Do not explain, do not use tools. Output only JSON: "
        '{"candidates":[{"name":"<conjunction>","ids":[<objects>],"warrant":"...","falsifier":"..."}]}'
    )


def extension(inst: dict, name: str) -> list[int] | None:
    parts = [p.strip() for p in name.split("&") if p.strip()]
    if not parts or any(p not in inst["ext"] for p in parts):
        return None
    ext = set(range(inst["objects"]))
    for p in parts:
        ext &= set(inst["ext"][p])
    return sorted(ext)


def decided_correct(inst: dict, adopted: str | None) -> bool:
    names = list(inst["vocabulary"])
    if adopted:
        names += [p.strip() for p in adopted.split("&") if p.strip()]
    names = sorted(set(names))
    consistent = set()
    for size in range(1, min(3, len(names)) + 1):
        for combo in combinations(names, size):
            ext = set(range(inst["objects"]))
            for n in combo:
                ext &= set(inst["ext"][n])
            if all(i in ext for i in inst["pos"]) and all(i not in ext for i in inst["neg"]):
                consistent.add(tuple(sorted(ext)))
    return len(consistent) == 1 and list(consistent)[0] == tuple(sorted(inst["target_ext"]))


def run_kernel_step(inst: dict, candidates: list[dict], engine: Path) -> dict:
    """Validate and adopt through the real kernel, then read the next question off `need`/`ask`."""

    def demo(lines: list[str]) -> list[str]:
        with tempfile.NamedTemporaryFile("w", suffix=".demo", delete=False, encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
            path = fh.name
        out = subprocess.run([str(engine), path], capture_output=True, text=True, encoding="utf-8")
        return out.stdout.splitlines()

    def ids(values) -> str:
        return " ".join(str(i) for i in values)

    base = [f"objects {inst['objects']}"]
    for name in inst["vocabulary"]:
        base.append(f"add {name} {ids(inst['ext'][name])}")
    base += [f"vote {i} 3 0" for i in inst["pos"]]
    base += [f"vote {i} 0 3" for i in inst["neg"]]

    # The model states conjunctions; their extension is computed here, not trusted from the JSON.
    usable = []
    for c in candidates:
        ext = extension(inst, c["name"])
        if ext:
            usable.append({"name": c["name"].replace(" ", ""), "ext": ext})

    splits: dict[str, int] = {}
    for c in usable:
        for line in demo(base + [f"score {c['name']} {ids(c['ext'])}"]):
            if line.startswith("score "):
                head, rest = line[len("score "):].split(":", 1)
                splits[head.strip()] = int(rest.split("splits", 1)[1].split("/", 1)[0].strip())

    best = None
    if usable:
        ranked = sorted(usable, key=lambda c: (-splits.get(c["name"], 0), c["name"]))
        if splits.get(ranked[0]["name"], 0) > 0:
            best = ranked[0]

    lines = list(base)
    if best:
        lines.append(f"add {best['name']} {ids(best['ext'])}")
    lines += ["need", "ask"]
    out2 = demo(lines)
    need = next((json.loads(l) for l in out2 if l.strip().startswith("{")), None)
    asked = None
    for line in out2:
        if line.startswith("suggested question: object "):
            asked = int(line.split("object ", 1)[1].split(" ", 1)[0])
    labelled = set(inst["pos"]) | set(inst["neg"])
    query = None
    if need and need.get("status") != "STOP" and asked is not None and asked not in labelled:
        query = {"object": asked, "positive": asked in inst["target_ext"]}
    if query is None and need and need.get("colliding_pairs"):
        p, n = need["colliding_pairs"][0]
        obj = p if p not in labelled else (n if n not in labelled else None)
        if obj is not None:
            query = {"object": obj, "positive": obj in inst["target_ext"]}
    return {
        "splits": splits,
        "adopted": best["name"] if best else None,
        "status": need.get("status") if need else None,
        "query": query,
        "decided_correct": decided_correct(inst, best["name"] if best else None),
    }


def apply_query(inst: dict, query: dict | None) -> None:
    if not query:
        return
    target = inst["pos"] if query["positive"] else inst["neg"]
    if query["object"] not in target:
        target.append(query["object"])
        target.sort()


def score_direct(inst: dict, rule: str) -> bool:
    names = [p.strip() for p in rule.replace(" ", "").split("&") if p.strip()]
    if not names or any(n not in inst["ext"] for n in names):
        return False
    ext = set(range(inst["objects"]))
    for n in names:
        ext &= set(inst["ext"][n])
    return sorted(ext) == sorted(inst["target_ext"])


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)

    g = sub.add_parser("instances")
    g.add_argument("--seeds", default="1,2,3,4")
    g.add_argument("--k", type=int, default=2)
    g.add_argument("--out", required=True)

    p = sub.add_parser("prompts")
    p.add_argument("--instances", required=True)
    p.add_argument("--arm", choices=["direct", "scaffold"], required=True)
    p.add_argument("--round", type=int, default=1)
    p.add_argument("--out", required=True)

    st = sub.add_parser("step")
    st.add_argument("--instances", required=True)
    st.add_argument("--candidates", required=True)
    st.add_argument("--out-instances", required=True)
    st.add_argument("--out-prompts", required=True)
    st.add_argument("--round", type=int, default=2)
    st.add_argument("--engine", default=str(DEFAULT_ENGINE))

    s = sub.add_parser("score")
    s.add_argument("--instances", required=True)
    s.add_argument("--arm", choices=["direct", "scaffold"], required=True)
    s.add_argument("--answers", required=True)

    args = ap.parse_args()
    if args.cmd == "instances":
        seeds = [int(x) for x in args.seeds.split(",")]
        data = [make_instance(s, args.k) for s in seeds]
        Path(args.out).write_text(json.dumps(data, indent=1), encoding="utf-8")
        for inst in data:
            print(f"seed {inst['seed']}: target {inst['target_name']} ext {inst['target_ext']} "
                  f"pos {inst['pos']} neg {inst['neg']}")
        return

    data = json.loads(Path(args.instances).read_text(encoding="utf-8"))

    if args.cmd == "prompts":
        out = []
        for inst in data:
            text = direct_prompt(inst) if args.arm == "direct" else scaffold_prompt(inst, "")
            out.append({"seed": inst["seed"], "prompt": text})
        Path(args.out).write_text(json.dumps(out, indent=1), encoding="utf-8")
        print(f"wrote {len(out)} prompts to {args.out}")
        return

    if args.cmd == "step":
        answers = {a["seed"]: a["candidates"] for a in json.loads(Path(args.candidates).read_text(encoding="utf-8"))}
        report, prompts, updated = [], [], []
        for inst in data:
            step = run_kernel_step(inst, answers.get(inst["seed"], []), Path(args.engine))
            history = ""
            if step["adopted"]:
                history += f"\nThe kernel adopted: {step['adopted']}.\n"
            if step["query"]:
                history += (
                    f"The kernel then asked about object {step['query']['object']}: it is "
                    f"{'positive' if step['query']['positive'] else 'negative'}.\n"
                )
            prompts.append({"seed": inst["seed"], "prompt": scaffold_prompt(inst, history)})
            apply_query(inst, step["query"])
            updated.append(inst)
            step["seed"] = inst["seed"]
            step["decided_correct"] = decided_correct(inst, step["adopted"])
            report.append(step)
        Path(args.out_instances).write_text(json.dumps(updated, indent=1), encoding="utf-8")
        Path(args.out_prompts).write_text(json.dumps(prompts, indent=1), encoding="utf-8")
        print(json.dumps(report, indent=1))
        return

    answers = {a["seed"]: a for a in json.loads(Path(args.answers).read_text(encoding="utf-8"))}
    hits = 0
    for inst in data:
        ans = answers.get(inst["seed"], {})
        ok = score_direct(inst, ans.get("rule", "")) if args.arm == "direct" else bool(ans.get("decided_correct"))
        hits += int(ok)
        print(f"seed {inst['seed']}: {'HIT' if ok else 'MISS'}  {ans}")
    print(f"{hits}/{len(data)} correct")


if __name__ == "__main__":
    main()
