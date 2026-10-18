# 04 Engine design: three-layer counterparts

v0.1 · 2026-10-19

## 0 In one sentence

The theory (Lean), the reference kernel (Rust), and the experiment bench (Python) share the same set of definitions;
the engine itself is deliberately kept "dumb" — it does only three things: decide whether the vocabulary is sufficient, ask about a representative, and keep the books.

## 1 Three-layer correspondence

| Paper | Lean | Rust | Python |
|---|---|---|---|
| Signature `sig_V(x)` | `QueryDriven.sig` | `Explorer::signature` | `signature()` |
| Equivalence classes | `Indist` | `Explorer::classes` | `classes()` |
| T2 vocabulary insufficiency | `no_consistent_concept` | `Status::Insufficient` | `consistent(...) == []` |
| T4 capacity | `capacity_bound` | `Explorer::capacity` | cardinality of `full_class` |
| T6 more data does not help | `no_consistent_concept_mono` | `t6_more_data_never_repairs_insufficiency` | `domain_more_data.py` |
| Vocabulary expansion | adding items to `Vocab` | `Explorer::propose` | `attrs + [a*]` |

Rust kernel: `engine/src/lib.rs`, 6 tests, `cargo test` all green (2.7 s).
Lean: `research/lean/QueryDriven.lean`, 6 theorems, 0 sorry.
Python experiments: `research/exp/*.py`; `RESULTS.md` is the data ledger.

## 2 State machine (the only control logic)

```
        ┌──────────────┐
        │  Insufficient │  ← positive and negative examples share a class (T2)
        └──────┬───────┘
               │ propose(p)
               ▼
        ┌──────────────┐
        │ Undetermined  │  ← there are still uncovered classes
        └──────┬───────┘
               │ answer(next_question(), ·)
               ▼
        ┌──────────────┐
        │  Identified   │  ← every class is covered by a positive or a negative example
        └──────────────┘
```

Three rules, nothing else:

1. `Insufficient` → the vocabulary must change (asking more objects cannot help, T6).
2. `Undetermined` → ask about a representative of an uncovered class (any one will do; domain A has already shown this).
3. `Identified` → stop, output the extension, not a story.

### Three additional rules under noise (`status_with(theta)`)

1. **Weak-vote blocking**: if a class still contains a member that "has voted but has not reached the vote margin θ" → it is not counted as decided (return `Undetermined`).
   Otherwise "one strong label + one weak objection" would be silently accepted, yielding a wrong extension.
2. **Top up votes before asking**: `next_question_with(theta)` returns objects with pending votes first, then representatives of uncovered classes.
3. **Conflict re-check**: on a collision, re-check for two rounds before proposing predicates (done by the driver; the kernel only provides `strong(theta)` for the judgment).

Default θ=3 (based on `RESULTS.md` §3.11: with ε=0.1, θ=1 succeeds only 25% of the time, θ=3 is 98%).

## 3 What the engine explicitly does not do

| Does not | Why |
|---|---|
| Use information gain to select objects | On par with "deduplicated random"; the gain comes from avoiding redundancy, not from selecting well (RESULTS §2) |
| Score and rank predicates | No reliable signal: a predicate's extension requires the oracle, and semantic quality has no judge |
| Self-assess answer quality | Self-scoring from the same source is unreliable (TACL 2024 survey conclusion) |
| Automatically submit "the most likely S" | When the vocabulary is insufficient, any submission is fabrication; returning `Insufficient` is preferable |

## 4 Interface contracts (traits to implement next)

```rust
pub trait Proposer {
    /// Given the samples and the current vocabulary, propose candidate predicates. The implementer may be an LLM, retrieval, or a human.
    fn propose(&self, ctx: &Explorer, k: usize) -> Vec<Predicate>;
}

pub trait Oracle {
    /// Membership query. May return `Unknown`, leaving room for noise / refusal.
    fn ask(&mut self, x: usize) -> Option<bool>;
}

pub trait CapacityGuard {
    /// T4: restrict how the vocabulary may grow, preventing "definable inside the sample, arbitrary outside it."
    fn allow(&self, ctx: &Explorer, p: &Predicate) -> bool;
}
```

Separation of responsibilities is this paper's structural principle: **the proposer cannot adjudicate, the adjudicator cannot propose, and the selector cannot self-assess.**

## 5 Known limitations (to be written into paper §7)

1. Number of objects ≤ 64 (`Mask = u64`); switching to `bitvec` is enough, semantics unaffected.
2. The oracle is noise-free; `Oracle::ask` returning `Option<bool>` only leaves room, there is no error-correction logic.
3. Deduplication of extensionally equivalent predicates is not done (two different predicates may share an extension, which inflates the capacity count).
4. With >64 classes, `capacity` saturates (`checked_shl`).

## 6 Reproduction

```powershell
cd engine
cargo test                      # 6 items, covering T2/T4/T6
pwsh -File ..\scripts\check-lean.ps1   # 6 theorems
cd ..\research\exp
& $py domain_a.py --n 300 --objects 12 --attrs 8 --k 2
& $py domain_cost.py
```

