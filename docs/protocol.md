# The proposal protocol

The engine never calls a model itself; it validates what a model returns. This page is the contract
between the two, and the record it leaves behind.

## Validating a response

`metaform propose` reads a request file and a response file under protocol `metaform.propose/1` and
reports what it accepts. Candidates are extensional (a name plus the object ids it covers) or defined
(`all_of` / `any_of` over names that already exist in the vocabulary). The validator rejects ids
outside the object range, constant or empty extensions, names that repeat the vocabulary or an
earlier candidate, duplicate ids, and anything arriving under the wrong protocol or request id; a
model may also decline, which is recorded as `refused` rather than retried. Validation is pure: it
writes no file and changes no state, so a proposal cannot overwrite evidence; `apply` is the explicit
adoption step.

Adopting an accepted definition persistently uses `metaform state add --project . --name p --define
all-of:a,b` (or `any-of:a,b`); `--ids` keeps the extensional form. The state file stores the
definition next to its extension, and every load recomputes that extension from the operands and the
current object count, so a project that grows past the object set it was saved with keeps a correct
predicate. A missing operand or a definition cycle is a clear error rather than a silently dropped
predicate; files written before definitions existed load unchanged and stay extensional.

## Adopting a response

`metaform apply --project . --request <request.json> --response <response.json>` collapses proposal
and adoption into one command. It runs the same validator, adopts every accepted candidate exactly as
`state add` would (an extensional candidate by its ids, a defined candidate by its definition, so the
state recomputes the extension), and writes `.metaform/state.json` once at the end. A candidate whose
name already exists in the state is skipped and reported (`name_exists`); candidates the validator
rejected are reported in `skipped` with the validator's reason, and the rest of the response is still
applied. A whole-response rejection (wrong protocol or request id, malformed JSON) exits non-zero and
leaves the file unchanged, as does a definition that cannot resolve, because every adoption happens on
a copy before the single write. The command prints one deterministic JSON line:
`{"applied":n,"skipped":[{"name":...,"reason":...}],"before":{"status":...,"colliding":n},"after":{"status":...,"colliding":n},"request_id":"..."}`.
`before` and `after` use the same recommended-`theta` status and colliding-pair count as `suggest` and
`state need`, so the one-command path cannot disagree with the read-only report. The request and
response files are only read.

## The held-out prediction (L11)

L11 makes the held-out prediction part of validation. A request that carries `object_ids` names its
held-out test: every id in that list that is absent from `evidence` is *unmeasured*. On such a request
a candidate must carry `"predict":{"id":<object id>,"label":"pos"|"neg"}`, or it is rejected with
`missing_prediction`; the prediction is echoed unchanged on acceptance. When present it is always
checked: an id outside `0..objects` is `prediction_out_of_range`, an id already in `evidence` is
`predicted_measured_object` (it is not a held-out test), and a label other than `pos` / `neg` is
`prediction_bad_label`. A request without `object_ids` -- the pre-L11 shape -- declares no unmeasured
object, so its candidates keep the old optional field and the old verdicts. A prediction only counts
as a bet when another rule could have made a different one, so a candidate may name the rule it is
competing with: `"rival":{"ids":[...]}"` or `"rival":{"define":{"all_of"|"any_of":[...]}}`. The rival
is parsed by the candidate's own rules and its extension is recomputed from the request vocabulary;
the prediction must then name an object where the two extensions disagree, or the candidate is
rejected with `predict_not_discriminating`. A rival without a prediction is `missing_prediction`,
because there is nothing left to compare. A candidate without `rival` behaves exactly as before and
keeps its old verdict, so a coherent but false rule can no longer pass on a prediction that every
rule in its neighbourhood would make too.

## Legality: a rule, not a patch

A request may carry `"legality": {"consistent": true, "must_place": [...]}`, and then a candidate has
to be a rule rather than a patch on the objects it was shown. Both clauses are optional; a request
without the field keeps every earlier verdict, which is why this did not bump the protocol version.

| clause | what it demands | rejection |
|---|---|---|
| `consistent` | the extension contains every positive and no negative | `legality_inconsistent` |
| `must_place` | the extension puts at least one of these objects in and at least one out | `legality_no_placement` |

The second clause is the one that closes a real hole. In a world where any subset of objects is a
legal predicate, `{"ids": [7]}` separates every colliding pair that involves object 7 and costs
nothing -- so a proposer never has to invent a rule, and an instrument that only checks "did it
separate the pair I asked about" cannot be priced. With `must_place`, a singleton contradicts the
evidence (it drops the other positives) or decides nothing about the unmeasured region; either way it
is refused before adoption. `{"legality": {}}` is itself an error: an experiment that believes it has
turned the rule on when it has not is worse than one that never turned it on.

## Recording predictions, and settling them

`apply` records every adopted prediction in `.metaform/predictions.tsv`: one header line plus one
tab-separated row per prediction (`request_id`, candidate name, object id, predicted label, the
candidate's extension as a comma-separated id list, and the rival's extension, empty when the
candidate named none). Rows dedupe on `(request_id, name, id)`, so replaying a response does not
duplicate a commitment, and the same input writes the same bytes. The rival column is last, so a
record written before it existed still loads: a five-column header and four- or five-field rows are
read with an empty rival.

`metaform check-predictions --project .` is read-only: it takes the same recommended-`theta` strong
labels the kernel uses and compares each recorded prediction with the object's actual label, printing
`{"checked":n,"correct":n,"wrong":[{"name":...,"id":...,"predicted":...,"actual":...}],"open":n,"discriminating":n}`;
`open` counts predictions whose object has no strong label yet, and `discriminating` counts the
checked predictions whose non-empty rival disagreed with the candidate on the predicted object.

## The falsifier (L1)

A candidate may state the observation that would refute it, and the statement has two grades. A prose
`falsifier` is stored, echoed back, and never checked -- it is a note, not a commitment. A
`{"pair":[a,b]}` falsifier is a commitment: it says these two objects differ on the concept the
candidate names. It is settled twice. At admission, the pair must be inside the object range
(`falsifier_bad_pair`), must actually be colliding (`falsifier_not_separating`), and must not already
be contradicted by the evidence (`falsifier_already_fired`); the accepted pair is echoed in the
verdict. After the fact, `check-predictions` re-reads the recorded pair and reports it under
`refuted`, so a falsifier that later fires is visible as a settlement rather than as silence.

Under the cheap-proposal assumption this is the division that matters: the model supplies many
candidates per call, the engine pays for the queries.
