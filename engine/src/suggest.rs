//! `metaform suggest`: run the Boolean search that an `INSUFFICIENT` state invites, and show why it
//! cannot produce a separating predicate.
//!
//! Every candidate is an `all_of` / `any_of` of predicates already in the vocabulary, so its extension
//! is a function of the current signature. Two operand sets that produce the same object mask are the
//! same candidate: the first in enumeration order survives, with canonical sorted operands, and
//! `deduplicated` counts the rest. The enumeration still runs and counts every candidate it `tried`,
//! because the empty result is the evidence: the next move must be outside Boolean recombination.
//! `reason` states that structural fact, and `next` names the action that can actually change the state.
//!
//! A colliding pair is two objects with the same signature
//! (Theorem 11; `README.md`: "Boolean recombination inside the vocabulary cannot break a collision").
//!
//! When `next` is `ask`, the report also names the question: one representative per colliding
//! equivalence class that still has a member never asked directly. `why` distinguishes a class with
//! no direct answer (`unlabelled_class`) from one whose unasked member inherits a label from an
//! asked sibling (`unprobed_member`). The `ask` field is omitted for `new_atom`, where no question
//! can repair the state -- including a class whose members were all asked and whose direct labels
//! disagree, which no further vote can settle.
//!
//! The command is read-only: it loads the project state through the same path as `state need`, never
//! saves it, and never creates a file. Output is one line of JSON, built by hand so the field order,
//! pair order, question order and candidate order are deterministic: names go through
//! `BTreeMap`/`BTreeSet`, classes are ordered by their lowest member, and colliding pairs are sorted
//! before scoring.
//!
//! `--emit-request` reuses the same collision definition to build the `metaform.propose/1` request a
//! proposer needs after `next` says `new_atom`: [`request_for_state`] returns it only for an
//! `INSUFFICIENT` state, with a deterministic `request_id` and every field derived from the kernel's
//! current view. It writes nothing; the CLI prints the request (or writes `--out`).

use std::collections::{BTreeMap, BTreeSet};

use crate::{bit, canonical_names, DefOp, Definition, Explorer, Mask, Status};

/// The protocol a ready-to-fill proposal request declares.
pub const PROPOSE_PROTOCOL: &str = "metaform.propose/1";

/// What a candidate may be under this protocol: an extensional `ids` list, or a Boolean `define`
/// over names that already exist in `vocabulary` (the only operands `propose` resolves).
const ALLOWED: &str =
    r#"{"ids":true,"define":{"ops":["all_of","any_of"],"operands":"vocabulary"}}"#;

/// One candidate definition and the colliding pairs it separates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub name: String,
    pub definition: Definition,
    pub splits: usize,
    pub covers: Vec<(usize, usize)>,
    pub arity: usize,
}

/// Why the enumeration found no usable Boolean candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Every colliding pair shares a signature, so every Boolean function of the vocabulary is
    /// constant on it and the search is guaranteed to find nothing.
    BooleanRecombinationCannotSeparate,
    /// The pairs do not all share a signature, yet none of the enumerated candidates split one.
    NoneSeparate,
    /// A candidate did split a pair. Unreachable while Theorem 11 holds; kept so the output stays
    /// truthful if the candidate space ever changes.
    CandidatesFound,
}

impl Reason {
    fn as_str(self) -> &'static str {
        match self {
            Reason::BooleanRecombinationCannotSeparate => "boolean_recombination_cannot_separate",
            Reason::NoneSeparate => "none_separate",
            Reason::CandidatesFound => "candidates_found",
        }
    }
}

/// What can actually reduce the collision. `ask` is possible only while some colliding class still
/// has a member that was never asked directly: probing that member verifies a label it inherited.
/// Once every member of every colliding class has been asked, no question can change the state, so
/// it needs a predicate outside the current atoms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Ask,
    NewAtom,
}

impl Next {
    fn as_str(self) -> &'static str {
        match self {
            Next::Ask => "ask",
            Next::NewAtom => "new_atom",
        }
    }
}

/// Why a colliding class still deserves an oracle answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// No member of the class was ever asked directly.
    UnlabelledClass,
    /// The class label is carried by a probed sibling; the representative's label is inherited.
    UnprobedMember,
}

impl Why {
    fn as_str(self) -> &'static str {
        match self {
            Why::UnlabelledClass => "unlabelled_class",
            Why::UnprobedMember => "unprobed_member",
        }
    }
}

/// The questions an `ask` state still deserves: one representative per colliding class that still
/// has a member never asked directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    /// One id per class, in ascending class order (lowest member first). `questions()` is its length.
    pub objects: Vec<usize>,
    /// The reason that applies to the first class in `objects`.
    pub why: Why,
}

impl Ask {
    fn new(objects: Vec<usize>, why: Why) -> Self {
        Self { objects, why }
    }

    /// One question per class; the count is the length of `objects` by construction.
    pub fn questions(&self) -> usize {
        self.objects.len()
    }
}

/// The whole answer for one `suggest` invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub status: Status,
    pub colliding: usize,
    /// Present only for `INSUFFICIENT`; `None` keeps the compact shape for the other statuses.
    pub reason: Option<Reason>,
    pub colliding_pairs: Vec<(usize, usize)>,
    pub next: Option<Next>,
    /// How many Boolean definitions were enumerated (including ones with no effect).
    pub tried: usize,
    /// How many otherwise-viable candidates were dropped because an earlier candidate had the same
    /// object mask. Dedup happens after evaluation, so `tried` still counts them.
    pub deduplicated: usize,
    pub suggestions: Vec<Suggestion>,
    /// Present only when `next == Ask`: which objects to ask and why. `NewAtom` omits it because no
    /// question can repair the state.
    pub ask: Option<Ask>,
}

/// The result of one enumeration: ranked candidates plus the two counts that make dedup visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enumerated {
    /// Candidates after deduplication, ranked and truncated to `max`.
    pub suggestions: Vec<Suggestion>,
    /// How many definitions were evaluated. Dedup happens after evaluation, so this counts every
    /// enumerated expression, including the ones dropped as duplicates.
    pub tried: usize,
    /// How many candidates were dropped because an earlier candidate covered the same object mask.
    pub deduplicated: usize,
}

/// Enumerate the Boolean vocabulary and report the candidates that split at least one colliding pair.
///
/// `max_arity` is the largest `all_of` size; `all_of` runs over every size from 1 through it. `any_of`
/// runs over size `max_arity` exactly. At arity 1 the `any_of` form is the predicate itself, already
/// emitted as an `all_of` singleton, so it is skipped. `max` truncates the ranked suggestions.
///
/// The `next` rule follows what an answer can do. Asking appends evidence and cannot remove a
/// colliding pair (Theorem 5), so `ask` is the next move only while some colliding class still has a
/// member that was never asked directly: probing that member verifies the label it inherited.
/// Otherwise `new_atom` is required -- including for a class whose members were all asked, even when
/// their direct labels conflict, because that class is a fixed point for questions.
pub fn suggest(e: &Explorer, max_arity: usize, max: usize) -> Report {
    let theta = e.recommended_theta();
    let status = e.status_with(theta);
    let (sp, sn) = e.strong(theta);
    let pairs = colliding_pairs(e, sp, sn);

    if status != Status::Insufficient {
        return Report {
            status,
            colliding: pairs.len(),
            reason: None,
            colliding_pairs: pairs,
            next: None,
            tried: 0,
            deduplicated: 0,
            suggestions: Vec::new(),
            ask: None,
        };
    }

    let preds = vocabulary(e);
    let objects = object_mask(e.n);
    let found = enumerate(&preds, objects, &pairs, max_arity, max);
    let suggestions = found.suggestions;

    let all_share = pairs.iter().all(|&(a, b)| e.signature(a) == e.signature(b));
    let reason = if suggestions.is_empty() {
        Some(if all_share {
            Reason::BooleanRecombinationCannotSeparate
        } else {
            Reason::NoneSeparate
        })
    } else {
        Some(Reason::CandidatesFound)
    };
    // `ask_plan` returns exactly the classes that still have an unasked member, so its presence is
    // the dispatch condition: a question is worth asking only while one is left. The empty-search
    // guard is defensive -- Theorem 11 means no candidate is ever found -- and keeps a hypothetical
    // separating candidate ahead of a question, as before.
    let ask = ask_plan(e, &pairs);
    let next = Some(if suggestions.is_empty() && ask.is_some() {
        Next::Ask
    } else {
        Next::NewAtom
    });
    let ask = if next == Some(Next::Ask) { ask } else { None };

    Report {
        status,
        colliding: pairs.len(),
        reason,
        colliding_pairs: pairs,
        next,
        tried: found.tried,
        deduplicated: found.deduplicated,
        suggestions,
        ask,
    }
}

/// Render a report as one line of JSON. The field order is fixed; the non-`INSUFFICIENT` form keeps
/// the compact shape promised by the CLI. `ask` is appended only when it exists, so `new_atom`
/// reports retain the old shape.
pub fn to_json(report: &Report) -> String {
    if report.status != Status::Insufficient {
        return format!(
            "{{\"status\":\"{}\",\"colliding\":{},\"suggestions\":[]}}",
            status_str(report.status),
            report.colliding
        );
    }
    let suggestions: Vec<String> = report.suggestions.iter().map(suggestion_json).collect();
    let ask = match &report.ask {
        Some(ask) => format!(",\"ask\":{}", ask_json(ask)),
        None => String::new(),
    };
    format!(
        "{{\"status\":\"{}\",\"colliding\":{},\"reason\":\"{}\",\"colliding_pairs\":{},\"next\":\"{}\",\"tried\":{},\"deduplicated\":{},\"suggestions\":[{}]{}}}",
        status_str(report.status),
        report.colliding,
        report.reason.map(Reason::as_str).unwrap_or("none_separate"),
        pairs_json(&report.colliding_pairs),
        report.next.map(Next::as_str).unwrap_or("new_atom"),
        report.tried,
        report.deduplicated,
        suggestions.join(","),
        ask
    )
}

/// The upper-case status word used by both `suggest` output shapes.
pub fn status_str(status: Status) -> &'static str {
    match status {
        Status::Insufficient => "INSUFFICIENT",
        Status::Undetermined => "UNDETERMINED",
        Status::Identified => "IDENTIFIED",
    }
}

/// The status and colliding-pair count `suggest` reports, under the recommended `theta`.
///
/// `metaform apply` calls this before and after adoption so its `before` / `after` fields use the
/// same view as `suggest` and `state need`; a second definition of "colliding" here would let the
/// one-command path disagree with the read-only report.
pub fn status_and_colliding(e: &Explorer) -> (Status, usize) {
    let theta = e.recommended_theta();
    let (sp, sn) = e.strong(theta);
    (e.status_with(theta), colliding_pairs(e, sp, sn).len())
}

/// Build the `metaform.propose/1` request that a proposer should fill once the state is
/// `INSUFFICIENT`, or report that the state does not need one.
///
/// The status is computed exactly as `suggest` computes it (recommended `theta`), so the two
/// commands cannot disagree about whether a proposal is due. A request is emitted only for
/// `INSUFFICIENT`; every other status returns `None` and the CLI prints
/// `{"status":"<STATUS>","request":null}`.
///
/// Every field is derived from the kernel's current view: `objects` is `n` (the integer `propose`
/// reads), `object_ids` is `0..n`, `vocabulary` is the resolved vocabulary (last occurrence of a
/// repeated name, sorted by name), `evidence` is the strong-positive/strong-negative mask under the
/// recommended threshold, `theta` is that threshold, and `colliding_pairs` is the same pair set the
/// suggest report shows. `allowed` names the candidate forms `propose` accepts. `request_id` hashes
/// the canonical state view, so replaying an unchanged state emits the identical request and no
/// other field can drift while the id stays fixed.
///
/// The request is deliberately built here, next to the collision definition, so `suggest` and
/// `propose` cannot disagree about what a pair is. It is one line by construction.
pub fn request_for_state(e: &Explorer) -> (Status, Option<String>) {
    let theta = e.recommended_theta();
    let status = e.status_with(theta);
    if status != Status::Insufficient {
        return (status, None);
    }

    let (sp, sn) = e.strong(theta);
    let pairs = colliding_pairs(e, sp, sn);
    let vocab = vocabulary(e);
    let objects: Vec<usize> = (0..e.n).collect();
    let pos = mask_ids(sp, e.n);
    let neg = mask_ids(sn, e.n);

    let request_id = format!(
        "suggest-{:016x}",
        state_hash(e.n, theta, &vocab, &pos, &neg, &pairs)
    );
    let vocabulary_json = vocab
        .iter()
        .map(|(name, ext)| {
            format!(
                "{{\"name\":{},\"ids\":{}}}",
                json_string(name),
                ids_json(&mask_ids(*ext, e.n))
            )
        })
        .collect::<Vec<String>>()
        .join(",");

    let request = format!(
        "{{\"protocol\":\"{PROPOSE_PROTOCOL}\",\"request_id\":{id},\"objects\":{n},\"object_ids\":{objects},\"vocabulary\":[{vocab}],\"evidence\":{{\"pos\":{pos},\"neg\":{neg}}},\"theta\":{theta},\"colliding_pairs\":{pairs},\"allowed\":{ALLOWED}}}",
        id = json_string(&request_id),
        n = e.n,
        objects = ids_json(&objects),
        vocab = vocabulary_json,
        pos = ids_json(&pos),
        neg = ids_json(&neg),
        pairs = pairs_json(&pairs),
    );
    (status, Some(request))
}

/// Ids set in `mask`, ascending. `n` bounds the scan so the request never mentions a nonexistent
/// object.
fn mask_ids(mask: Mask, n: usize) -> Vec<usize> {
    (0..n).filter(|&x| (mask >> x) & 1 == 1).collect()
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over a canonical text of every state-derived request field. Deterministic across runs and
/// independent of map iteration order, which is what `request_id` promises.
fn state_hash(
    n: usize,
    theta: u8,
    vocab: &[(String, Mask)],
    pos: &[usize],
    neg: &[usize],
    pairs: &[(usize, usize)],
) -> u64 {
    let mut text = format!("n={n}\ntheta={theta}\n");
    for (name, ext) in vocab {
        text.push_str(&format!("v={name}|{}\n", join_ids(&mask_ids(*ext, n))));
    }
    text.push_str(&format!("pos={}\n", join_ids(pos)));
    text.push_str(&format!("neg={}\n", join_ids(neg)));
    for (a, b) in pairs {
        text.push_str(&format!("p={a},{b}\n"));
    }
    let mut h = FNV_OFFSET;
    for &b in text.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn join_ids(ids: &[usize]) -> String {
    ids.iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>()
        .join(",")
}

/// Colliding pairs under the `theta` labels, with the class iteration order removed.
fn colliding_pairs(e: &Explorer, sp: Mask, sn: Mask) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for c in e.classes() {
        let ins: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (sp & bit(x)) != 0)
            .collect();
        let outs: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (sn & bit(x)) != 0)
            .collect();
        for a in &ins {
            for b in &outs {
                out.push((*a, *b));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Name one question per colliding class that still has a member never asked directly.
///
/// `e.asked` (written by `Explorer::answer_votes`) records which members were asked directly, so
/// inheritance is visible here: a class whose label comes from an asked sibling can still be probed.
/// Asking only appends evidence, so a class with no unasked member contributes 0 -- no answer can
/// change its labels. Otherwise the class contributes one representative:
///
/// - `unlabelled_class`: no member was ever asked, so the lowest member is asked;
/// - `unprobed_member`: an asked sibling carries the class label and the lowest unasked member is
///   asked instead (its label is inherited and may be wrong).
///
/// Classes are ordered by their lowest member (a `HashMap` order would not be deterministic) and
/// `why` describes the first class, matching the first entry of `objects`.
///
/// A genuine colliding pair is a strong positive and a strong negative in one class, so at least one
/// member of its class was asked. `unlabelled_class` is therefore only reachable when callers pass a
/// synthetic pair (the unit tests do, as in `rank`); keeping the branch makes the rule total.
fn ask_plan(e: &Explorer, pairs: &[(usize, usize)]) -> Option<Ask> {
    let mut classes = e.classes();
    classes.sort_unstable();

    let mut chosen: BTreeMap<usize, (usize, Why)> = BTreeMap::new();
    for &(a, b) in pairs {
        for x in [a, b] {
            let Some(class) = classes.iter().copied().find(|&c| c & bit(x) != 0) else {
                continue;
            };
            let anchor = class.trailing_zeros() as usize;
            if chosen.contains_key(&anchor) {
                continue;
            }
            let members: Vec<usize> = (0..e.n).filter(|&m| class & bit(m) != 0).collect();
            let unprobed = members.iter().copied().find(|&m| e.asked & bit(m) == 0);
            let entry = if class & e.asked == 0 {
                // No member was ever asked: the class has no direct answer at all.
                (anchor, Why::UnlabelledClass)
            } else if let Some(member) = unprobed {
                // An asked sibling carries the label; this member's label is inherited.
                (member, Why::UnprobedMember)
            } else {
                // Every member was asked directly, so no probe is left. A conflict in such a class
                // is a fixed point for questions: only a new atom can change it.
                continue;
            };
            chosen.insert(anchor, entry);
        }
    }

    let why = chosen.values().next()?.1;
    let objects = chosen.values().map(|&(representative, _)| representative).collect();
    Some(Ask::new(objects, why))
}

/// The vocabulary as sorted `(name, extension)` pairs. A repeated name resolves to its last
/// occurrence, matching the state loader's `by_name` rule.
fn vocabulary(e: &Explorer) -> Vec<(String, Mask)> {
    let mut by_name: BTreeMap<String, Mask> = BTreeMap::new();
    for p in &e.vocab.preds {
        by_name.insert(p.name.clone(), p.ext);
    }
    by_name.into_iter().collect()
}

fn object_mask(n: usize) -> Mask {
    if n >= 64 {
        Mask::MAX
    } else if n == 0 {
        0
    } else {
        (1u64 << n) - 1
    }
}

/// Enumerate candidates, dedupe them by object mask, keep the ones that split a pair, rank and
/// truncate them.
///
/// `pairs` is passed separately from `e` so the enumeration can be exercised with pairs that are not
/// class-derived; the CLI never does that (it always passes the colliding pairs of the state).
///
/// Dedup happens after each candidate's extension is computed and after it earns a place by
/// splitting a pair. The first candidate in enumeration order for a given mask is kept, so the
/// surviving representative -- and therefore the ranked output -- is deterministic. Dropping a later
/// operand set does not decrement `tried`: that count reports how many expressions were evaluated.
pub fn enumerate(
    preds: &[(String, Mask)],
    objects: Mask,
    pairs: &[(usize, usize)],
    max_arity: usize,
    max: usize,
) -> Enumerated {
    let mut en = Enumerator {
        preds,
        objects,
        pairs,
        taken: preds.iter().map(|(name, _)| name.clone()).collect(),
        seen: BTreeSet::new(),
        suggestions: Vec::new(),
        tried: 0,
        deduplicated: 0,
    };
    en.run(max_arity);
    en.finish(max)
}

struct Enumerator<'a> {
    preds: &'a [(String, Mask)],
    objects: Mask,
    pairs: &'a [(usize, usize)],
    /// Candidate names already used, including every vocabulary name.
    taken: BTreeSet<String>,
    /// Object masks already claimed by a candidate that split a pair.
    seen: BTreeSet<Mask>,
    suggestions: Vec<Suggestion>,
    tried: usize,
    deduplicated: usize,
}

impl<'a> Enumerator<'a> {
    fn run(&mut self, max_arity: usize) {
        let m = self.preds.len();
        for k in 1..=max_arity.min(m) {
            let mut current = Vec::with_capacity(k);
            self.walk(DefOp::AllOf, k, 0, &mut current);
        }
        // At arity 1 the disjunction is the predicate itself, already emitted above.
        if max_arity >= 2 && max_arity <= m {
            let mut current = Vec::with_capacity(max_arity);
            self.walk(DefOp::AnyOf, max_arity, 0, &mut current);
        }
    }

    fn walk(&mut self, op: DefOp, k: usize, start: usize, current: &mut Vec<usize>) {
        if current.len() == k {
            self.consider(op, current);
            return;
        }
        let m = self.preds.len();
        let needed = k - current.len();
        if m < start + needed {
            return;
        }
        for i in start..m {
            current.push(i);
            self.walk(op, k, i + 1, current);
            current.pop();
        }
    }

    fn consider(&mut self, op: DefOp, indices: &[usize]) {
        self.tried += 1;
        let mut ext = match op {
            DefOp::AllOf => Mask::MAX,
            DefOp::AnyOf => 0,
        };
        for &i in indices {
            let mask = self.preds[i].1;
            ext = match op {
                DefOp::AllOf => ext & mask,
                DefOp::AnyOf => ext | mask,
            };
        }
        ext &= self.objects;

        let covers: Vec<(usize, usize)> = self
            .pairs
            .iter()
            .copied()
            .filter(|&(a, b)| ((ext >> a) & 1) != ((ext >> b) & 1))
            .collect();
        if covers.is_empty() {
            return;
        }

        // The extension is the candidate's identity. A later operand set that covers the same
        // objects adds no information, so it is dropped here -- after `tried` counted it -- and the
        // first representative in enumeration order survives.
        if !self.seen.insert(ext) {
            self.deduplicated += 1;
            return;
        }

        // Canonical operand order: sorted and unique, independent of the order the caller supplied
        // `preds` in. The generated name follows the canonical order too.
        let operands = canonical_names(indices.iter().map(|&i| self.preds[i].0.clone()).collect());
        let arity = operands.len();
        let name = self.unique_name(op, &operands);
        self.suggestions.push(Suggestion {
            name,
            definition: Definition::new(op, operands),
            splits: covers.len(),
            covers,
            arity,
        });
    }

    fn unique_name(&mut self, op: DefOp, operands: &[String]) -> String {
        let joiner = match op {
            DefOp::AllOf => "_and_",
            DefOp::AnyOf => "_or_",
        };
        let mut base = operands.join(joiner);
        base = base.trim().to_string();
        if base.is_empty() {
            base = "suggestion".to_string();
        }
        if !self.taken.contains(&base) {
            self.taken.insert(base.clone());
            return base;
        }
        let mut n = 2usize;
        loop {
            let candidate = format!("{base}_{n}");
            if !self.taken.contains(&candidate) {
                self.taken.insert(candidate.clone());
                return candidate;
            }
            n += 1;
        }
    }

    fn finish(mut self, max: usize) -> Enumerated {
        self.suggestions.sort_by(|a, b| {
            b.splits
                .cmp(&a.splits)
                .then(a.arity.cmp(&b.arity))
                .then(a.name.cmp(&b.name))
        });
        self.suggestions.truncate(max);
        Enumerated {
            suggestions: self.suggestions,
            tried: self.tried,
            deduplicated: self.deduplicated,
        }
    }
}

fn suggestion_json(s: &Suggestion) -> String {
    let op = match s.definition.op {
        DefOp::AllOf => "all_of",
        DefOp::AnyOf => "any_of",
    };
    let operands: Vec<String> = s
        .definition
        .operands
        .iter()
        .map(|name| json_string(name))
        .collect();
    format!(
        "{{\"name\":{},\"define\":{{\"{op}\":[{}]}},\"splits\":{},\"covers\":{},\"arity\":{}}}",
        json_string(&s.name),
        operands.join(","),
        s.splits,
        pairs_json(&s.covers),
        s.arity
    )
}

fn pairs_json(pairs: &[(usize, usize)]) -> String {
    let items: Vec<String> = pairs.iter().map(|(a, b)| format!("[{a},{b}]")).collect();
    format!("[{}]", items.join(","))
}

fn ids_json(ids: &[usize]) -> String {
    let items: Vec<String> = ids.iter().map(|x| x.to_string()).collect();
    format!("[{}]", items.join(","))
}

fn ask_json(ask: &Ask) -> String {
    format!(
        "{{\"questions\":{},\"objects\":{},\"why\":\"{}\"}}",
        ask.questions(),
        ids_json(&ask.objects),
        ask.why.as_str()
    )
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Predicate, Vocabulary};

    fn explorer(n: usize, preds: &[(&str, Mask)], votes: &[(usize, u8, u8)]) -> Explorer {
        let preds = preds
            .iter()
            .map(|(name, ext)| Predicate::new(*name, *ext))
            .collect();
        let mut e = Explorer::new(n, Vocabulary::with(preds));
        for &(id, yes, no) in votes {
            e.answer_votes(id, yes, no);
        }
        e
    }

    /// A fully probed collision is a fixed point for questions: both members were asked and their
    /// direct labels disagree, so no further vote can remove the pair. The report must say
    /// `new_atom` and omit `ask`. (Changed under the new dispatch rule: this used to assert `ask`
    /// with `why: conflicting_labels`, because the rule counted colliding pairs.)
    #[test]
    fn one_collision_reports_the_impossibility_and_is_deterministic() {
        let e = explorer(2, &[("a", bit(0) | bit(1))], &[(0, 3, 0), (1, 0, 3)]);
        let report = suggest(&e, 2, 20);
        assert_eq!(report.status, Status::Insufficient);
        assert_eq!(report.colliding, 1);
        assert_eq!(report.colliding_pairs, vec![(0, 1)]);
        assert_eq!(
            report.reason,
            Some(Reason::BooleanRecombinationCannotSeparate)
        );
        assert_eq!(report.next, Some(Next::NewAtom));
        assert_eq!(report.tried, 1);
        assert_eq!(report.deduplicated, 0);
        assert!(report.suggestions.is_empty());
        assert!(
            report.ask.is_none(),
            "every member was asked, so no probe is left"
        );

        let json = to_json(&report);
        assert_eq!(
            json,
            concat!(
                r#"{"status":"INSUFFICIENT","colliding":1,"#,
                r#""reason":"boolean_recombination_cannot_separate","#,
                r#""colliding_pairs":[[0,1]],"next":"new_atom","tried":1,"deduplicated":0,"suggestions":[]}"#
            )
        );
        assert_eq!(
            json,
            to_json(&suggest(&e, 2, 20)),
            "the output must be stable"
        );
    }

    /// A class that was never asked has no direct answer at all, so its lowest member is the
    /// question. A real colliding pair always carries two strong labels, so the pair is synthetic
    /// here, exactly as in the ranking tests.
    #[test]
    fn ask_names_a_representative_of_an_unlabelled_class() {
        let e = explorer(2, &[("a", 0)], &[]);
        let ask = ask_plan(&e, &[(0, 1)]).expect("an unlabelled class is still worth asking");
        assert_eq!(ask.questions(), 1);
        assert_eq!(ask.objects, vec![0]);
        assert_eq!(ask.why, Why::UnlabelledClass);
    }

    /// The class label comes from object 0; object 2's label is inherited and has never been
    /// tested, so it is the question even though 0 and 1 conflict. The dispatch still says `ask`.
    #[test]
    fn ask_prefers_the_inherited_member_of_a_probed_class() {
        let e = explorer(
            3,
            &[("a", bit(0) | bit(1) | bit(2))],
            &[(0, 3, 0), (1, 0, 3)],
        );
        let report = suggest(&e, 2, 20);
        assert_eq!(report.colliding_pairs, vec![(0, 1)]);
        assert_eq!(report.next, Some(Next::Ask), "a probe is still available");
        let ask = report.ask.as_ref().expect("ask is still the next move");
        assert_eq!(ask.questions(), 1);
        assert_eq!(ask.objects, vec![2], "the unprobed member is preferred");
        assert_eq!(ask.why, Why::UnprobedMember);
    }

    /// Every member was asked, so no probe is left even though the direct labels disagree. The
    /// class contributes no question; the report recommends `new_atom` instead. (Changed under the
    /// new dispatch rule: this test used to assert `why: conflicting_labels` and object 0.)
    #[test]
    fn ask_plan_skips_a_fully_probed_conflicting_class() {
        let e = explorer(2, &[("a", bit(0) | bit(1))], &[(0, 3, 0), (1, 0, 3)]);
        assert_eq!(ask_plan(&e, &[(0, 1)]), None);
    }

    /// One question per class, not per pair: two pairs in one class and a pair in another produce
    /// two questions, in ascending class order, and `why` follows the first class. Both classes
    /// must still have an unasked member, because asking a fully probed class changes nothing.
    #[test]
    fn ask_counts_one_question_per_class_and_dedups_repeated_pairs() {
        let e = explorer(
            5,
            &[("a", bit(0) | bit(1) | bit(2)), ("b", bit(3) | bit(4))],
            &[(0, 3, 0), (1, 0, 3), (3, 3, 0)],
        );
        let ask = ask_plan(&e, &[(0, 1), (0, 2), (3, 4)]).expect("two classes still have a probe");
        assert_eq!(ask.questions(), 2, "one per class, not one per pair");
        assert_eq!(ask.objects, vec![2, 4]);
        assert_eq!(ask.why, Why::UnprobedMember, "the first class by anchor");
        assert_eq!(ask.questions(), ask.objects.len());
    }

    /// A class whose members were all asked and whose direct labels agree is settled; it does not
    /// add a question. Another question cannot change it.
    #[test]
    fn ask_skips_a_class_settled_by_agreeing_answers() {
        let e = explorer(2, &[("a", 0)], &[(0, 3, 0), (1, 3, 0)]);
        assert_eq!(ask_plan(&e, &[(0, 1)]), None);
    }

    /// Every member of the class was asked, so no question can repair the state; the `ask` field is
    /// omitted rather than emitted as `null`. The many-pair case and the fully probed
    /// conflicting class agree under the new rule.
    #[test]
    fn new_atom_omits_the_ask_field() {
        let e = explorer(
            6,
            &[("a", 0b111111)],
            &[
                (0, 3, 0),
                (1, 3, 0),
                (2, 3, 0),
                (3, 0, 3),
                (4, 0, 3),
                (5, 0, 3),
            ],
        );
        let report = suggest(&e, 2, 20);
        assert_eq!(report.colliding, 9, "3 positives x 3 negatives");
        assert_eq!(report.next, Some(Next::NewAtom));
        assert!(report.ask.is_none());
        assert!(
            !to_json(&report).contains("\"ask\""),
            "new_atom keeps the old JSON shape"
        );
    }

    #[test]
    fn non_insufficient_states_keep_the_compact_shape() {
        let e = explorer(2, &[("p", bit(0))], &[]);
        let report = suggest(&e, 2, 20);
        assert_eq!(report.status, Status::Undetermined);
        assert_eq!(report.colliding, 0);
        assert_eq!(report.suggestions, Vec::new());
        assert!(report.ask.is_none());
        assert_eq!(
            to_json(&report),
            r#"{"status":"UNDETERMINED","colliding":0,"suggestions":[]}"#
        );
    }

    /// The ranking rules: splits desc, arity asc, name; `tried` counts every definition. The pairs
    /// here are synthetic: a real colliding pair is constant on the vocabulary, so this is the only
    /// way to exercise the ordering without claiming that Boolean recombination can separate.
    #[test]
    fn ranking_orders_and_truncates_boolean_candidates() {
        let preds = vec![
            ("a".to_string(), bit(0) | bit(1)),
            ("b".to_string(), bit(1) | bit(2)),
            ("c".to_string(), bit(3)),
        ];
        let pairs = vec![(0usize, 2usize), (1, 3)];
        let found = enumerate(&preds, 0b1111, &pairs, 2, 4);
        let suggestions = found.suggestions;

        assert_eq!(found.tried, 9, "3 singletons + 3 conjunctions + 3 disjunctions");
        let names: Vec<&str> = suggestions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["a_2", "b_2", "c_2", "a_and_b"]);
        assert_eq!(suggestions[0].splits, 2);
        assert_eq!(suggestions[0].covers, vec![(0, 2), (1, 3)]);
        assert_eq!(suggestions[0].arity, 1);
        assert_eq!(suggestions[3].definition.op, DefOp::AllOf);
        assert_eq!(suggestions[3].definition.operands, vec!["a", "b"]);
        assert_eq!(suggestions[3].splits, 1);
    }

    #[test]
    fn dedup_by_extension_keeps_the_first_canonical_candidate() {
        // Deliberately unsorted vocabulary: `b & a` is enumerated first for {1}, but the surviving
        // definition must still spell its operands as `[a, b]`.
        let preds = vec![
            ("b".to_string(), bit(1) | bit(2)),
            ("a".to_string(), bit(0) | bit(1)),
            ("c".to_string(), bit(1) | bit(3)),
        ];
        let found = enumerate(&preds, 0b1111, &[(0, 1)], 2, 20);

        assert_eq!(
            found.tried, 9,
            "3 singletons + 3 conjunctions + 3 disjunctions"
        );
        assert_eq!(
            found.deduplicated, 2,
            "b&c and a&c repeat b&a's extension {{1}}"
        );

        let ones: Vec<&Suggestion> = found
            .suggestions
            .iter()
            .filter(|s| s.definition.operands == vec!["a", "b"])
            .collect();
        assert_eq!(
            ones.len(),
            1,
            "one candidate for extension {{1}}: {:?}",
            found.suggestions
        );
        assert_eq!(ones[0].definition.op, DefOp::AllOf);
        assert!(
            found.suggestions.iter().all(|s| {
                s.definition
                    .operands
                    .windows(2)
                    .all(|w| w[0] < w[1])
            }),
            "every surviving operand list is sorted and unique: {:?}",
            found.suggestions
        );

        let json = to_json(&Report {
            status: Status::Insufficient,
            colliding: 1,
            reason: Some(Reason::CandidatesFound),
            colliding_pairs: vec![(0, 1)],
            next: Some(Next::NewAtom),
            tried: found.tried,
            deduplicated: found.deduplicated,
            suggestions: found.suggestions.clone(),
            ask: None,
        });
        assert!(
            json.contains(r#""tried":9,"deduplicated":2,"#),
            "the JSON records the drops without changing `tried`: {json}"
        );
    }

    #[test]
    fn arity_one_skips_the_duplicate_any_of_form() {
        let preds = vec![
            ("a".to_string(), bit(0) | bit(1)),
            ("b".to_string(), bit(1) | bit(2)),
        ];
        let found = enumerate(&preds, 0b111, &[(0, 2)], 1, 20);
        assert_eq!(found.tried, 2, "only the two singletons, no duplicate any_of");
        assert_eq!(found.suggestions.len(), 2);
        assert!(found
            .suggestions
            .iter()
            .all(|s| s.arity == 1 && s.definition.op == DefOp::AllOf));
    }

    #[test]
    fn generated_names_are_legal_unique_and_not_in_the_vocabulary() {
        let preds = vec![
            ("a".to_string(), bit(0) | bit(1)),
            ("b".to_string(), bit(1) | bit(2)),
            ("a_and_b".to_string(), bit(0)),
        ];
        let vocab: BTreeSet<&str> = preds.iter().map(|(name, _)| name.as_str()).collect();
        let found = enumerate(&preds, 0b1111, &[(0, 2), (1, 3)], 2, 20);
        let suggestions = found.suggestions;
        assert!(
            !suggestions.is_empty(),
            "the synthetic pairs must produce candidates"
        );

        let mut seen = BTreeSet::new();
        for s in &suggestions {
            assert!(!s.name.trim().is_empty(), "propose rejects an empty name");
            assert!(
                !vocab.contains(s.name.as_str()),
                "name collides with the vocabulary"
            );
            assert!(
                seen.insert(s.name.clone()),
                "candidate names must be unique"
            );
            assert!(!s.definition.operands.is_empty());
            assert!(s
                .definition
                .operands
                .iter()
                .all(|o| vocab.contains(o.as_str())));
        }
        assert!(
            suggestions.iter().any(|s| s.name.starts_with("a_and_b_")),
            "a generated name that collides must get a deterministic suffix: {suggestions:?}"
        );
    }
}
