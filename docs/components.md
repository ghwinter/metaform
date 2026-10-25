# Components

The measure is one crate and the proposers are others. Each algorithm gets its own crate, none of them
may change the state, and the dependency arrows point one way -- `cargo` refuses the import that points
up, which is why the layering does not need a hand-maintained list:

| crate | what it proposes | how |
|---|---|---|
| `crates/kernel` | nothing: it is the measure | the vocabulary, the equivalence classes it induces, the status they license |
| `crates/vsa` | predicates whose vector similarity to a positive/negative contrast is high | MBAT binding and bundling, `retrieve` |
| `crates/sme` | predicates inferred by mapping one relational system onto another | structure mapping, `analogize` |
| `crates/ilp` | clauses whose extension covers the positives and no negative -- **and is now a proposer, not a demonstration** | relational learning, `ilp_demo`, `ilp_learn` (JSON in, extension out) |
| `crates/runtime` | nothing: it is the kernel | protocol, validation, controller, record, `Operator` |
| `crates/components` | the concrete operators and the CLI | `VsaRetriever`, `SmeAnalogizer`, `ProbeLabels`, `metaform-runtime` |

A component proposes, the kernel judges:

```powershell
target\debug\metaform-runtime.exe retrieve  --state crates/runtime/fixtures/state.txt --out response.json
target\debug\metaform-runtime.exe analogize --state crates/runtime/fixtures/state.txt --systems crates/runtime/fixtures/systems.txt --out response.json
target\debug\metaform.exe propose --request crates/runtime/fixtures/request.json --response response.json
```

The controller does the adopting, and records what it did:

```powershell
target\debug\metaform-runtime.exe run --state crates\runtime\fixtures\state.txt --record runs.tsv
```

It asks every applicable operator, keeps what the validator accepts, adopts the proposal that
separates the most colliding pairs (cheapest on a tie), stops when nothing separates anything, and
appends one line per step -- state fingerprint, operator, proposal, pairs separated, adopted, cost,
status before and after, and the state class (objects, vocabulary size, labelled examples, colliding
pairs) before the step.

One record line is written per **offer**, not per step, so a proposal that lost is visible too.
The record is then queryable: `metaform-runtime experience --record runs.tsv --state <file>` prints
the tallies per class and says which operator has adopted most often in the class of that state;
`run --use-record runs.tsv` puts that operator first. With `--probe` the controller also reports a
`conflict:` line -- and records it -- when the adopted proposal cannot separate every pair and the
probe has named a label it had to disbelieve. Order is the only thing experience changes --
every operator is still asked, and the controller still decides by pairs separated.

Anything that is not a Rust component -- a model, a script, a person with an editor -- can compete by
writing a proposals file and passing `--proposals`:

```text
# label   name      ids
k2_0      a4        0 5 10 11
```

The third line above is the only one that can accept anything, and accepting is still not adopting:
`experiments/proposers/proposer_arms.py` gives one state to three proposers (random, retrieval, and the
proposals a language model wrote) and reports what each found, judged by the same kernel.

### Three gates, three different failures

The three ways a candidate can be wrong need three different instruments, and the measurement of
2026-10-24 says which is which:

| failure | instrument | evidence |
|---|---|---|
| the candidate contradicts the evidence | **semantic consistency** (rule 12) | ten contradictory candidates: accepted ten times out of ten without it, refused ten times out of ten with it |
| the evidence is too thin to decide | **the vote gap** (`theta`, `pending`) | not yet measured against a proposer |
| **the evidence itself is wrong** | **a probe that never read the evidence** | nine accepted candidates, all wrong about one flipped label, and no amount of comparison with that evidence would have caught it |

The third row is why the vector retriever stays in the loop even when the proposer is the inducer: a
similarity search is the only source in the loop that did not read the labels, so it is the only one
that can *suspect* a label. A criterion that only compares against the evidence cannot see the
evidence being wrong, because corrupted evidence agrees with its own corruption.

### The inductive learner became a proposer

`crates/ilp` used to be a demonstration of a relational learner. It now answers the question the
kernel cannot: given background knowledge, positives and negatives, **what is the extension over every
object** -- not over the objects the examples mentioned. `learn_extension` returns that extension and
`ilp_learn` wraps it in JSON, so a proposer can be a program.

On the proposer bench it reaches a hit rate of 0.9 against the vector retriever's 0.1, and the kernel's
own numbers are identical to the model's: ten of ten accepted, forty splits. Its cost is milliseconds
against fifteen to twenty seconds of generation.

Two costs are stated rather than smoothed over. It has **no vote-gap gate**, so one flipped label takes
it from 0.9 to 0.0; and the model arm of that family replays candidates recorded earlier, so it never
faced the corruption the learner faced. The comparison is not fair yet, and the pre-registration says so.

## Where the layering is enforced

Two questions, two scans, and the second one is the one that used to be missing.

**Inside a crate.** `metaform check layers` reads the `use crate::` lines of this crate's `src` and
refuses an import that points at a module above the importer's tier; an unclassified module is a
failure rather than a default.

**Between crates.** The compiler enforces that there is no cycle, not that the arrows point the right
way: `metaform-runtime` may name `metaform-vsa` in its `Cargo.toml` and `cargo` is perfectly happy,
because that edge is acyclic. So `check layers` also reads every workspace member's manifest and
requires each workspace dependency to sit on a strictly lower layer, from a table stated once at the
top of `crates/metaform/src/check.rs`:

```
kernel < propose < store < metaform < runtime < vsa | sme | ilp < components
```

A dependency on the same layer is a failure too -- `vsa` and `sme` are interchangeable proposers, and
a test that lets one call the other would make the two crates one crate with two names. A line in a
manifest that cannot be read is a failure rather than a skip, because an unparsed dependency is a
dependency nobody checks.

**Prints** are scanned the same way: every member's `src`, recursively, counted per file. Only the
engine crate has a silence rule (its kernel tier must not print); elsewhere a print is the component's
output and is counted, not judged -- but it is now counted, which is the difference between "we allow
it" and "we did not look".

**A member outside `crates/`** is a directory whose name describes a history rather than a role, and
`metaform check members` refuses it.
