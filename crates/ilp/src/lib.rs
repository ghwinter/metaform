//! Relational rule learning for metaform.
//!
//! This is a small top-down learner in the spirit of Popper and Metagol: a clause is a head atom
//! over fresh variables and a body of atoms over them; the body is grown by adding literals from a
//! fixed signature, and a candidate is kept only if it covers positives and no negative. The search
//! is exhaustive within a body-length bound and deterministic; there is no conflict-driven
//! reasoning and no metarule machinery, and this file does not copy any code from those systems.
//!
//! What the runtime needs from it is not a competitive ILP system but a *proposer*: given evidence
//! over a small domain, return a clause whose extension can be checked by the exact kernel.

use std::collections::BTreeMap;

/// A term: a variable (index) or a constant (object id).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Term {
    Var(u32),
    Const(u32),
}

/// An atom: a predicate applied to terms.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Atom {
    pub pred: String,
    pub args: Vec<Term>,
}

/// A definite clause: head :- body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clause {
    pub head: Atom,
    pub body: Vec<Atom>,
}

impl Clause {
    pub fn to_string(&self) -> String {
        let head = render(&self.head);
        if self.body.is_empty() {
            return format!("{}.", head);
        }
        let body: Vec<String> = self.body.iter().map(render).collect();
        format!("{} :- {}.", head, body.join(", "))
    }
}

fn render(atom: &Atom) -> String {
    let args: Vec<String> = atom
        .args
        .iter()
        .map(|t| match t {
            Term::Var(v) => format!("V{}", v),
            Term::Const(c) => format!("c{}", c),
        })
        .collect();
    if args.is_empty() {
        atom.pred.clone()
    } else {
        format!("{}({})", atom.pred, args.join(","))
    }
}

/// A substitution from variables to terms.
pub type Subst = BTreeMap<u32, u32>;

/// The predicates the body may use, in the order the learner tries them.
#[derive(Clone, Debug)]
pub struct Signature {
    pub name: String,
    pub arity: usize,
}

/// Ground facts plus the body signature.
#[derive(Clone, Debug, Default)]
pub struct Background {
    pub facts: Vec<Atom>,
    pub signature: Vec<Signature>,
}

impl Background {
    pub fn new(signature: Vec<Signature>) -> Self {
        Background { facts: Vec::new(), signature }
    }

    pub fn fact(&mut self, pred: &str, args: &[u32]) {
        self.facts.push(Atom {
            pred: pred.to_string(),
            args: args.iter().map(|&c| Term::Const(c)).collect(),
        });
    }
}

/// Matches a pattern atom against a ground atom, extending the substitution. Variables are
/// assigned consistently; constants must match exactly.
fn unify(pattern: &Atom, ground: &Atom, subst: &mut Subst) -> bool {
    if pattern.pred != ground.pred || pattern.args.len() != ground.args.len() {
        return false;
    }
    for (p, g) in pattern.args.iter().zip(ground.args.iter()) {
        let Term::Const(c) = *g else { return false };
        match p {
            Term::Const(pc) => {
                if *pc != c {
                    return false;
                }
            }
            Term::Var(v) => match subst.get(v) {
                Some(bound) if *bound != c => return false,
                Some(_) => {}
                None => {
                    subst.insert(*v, c);
                }
            },
        }
    }
    true
}

/// True when the body, under the given head substitution, is entailed by the facts.
fn body_holds(body: &[Atom], subst: &Subst, facts: &[Atom]) -> bool {
    let Some((first, rest)) = body.split_first() else {
        return true;
    };
    for fact in facts {
        if fact.pred != first.pred || fact.args.len() != first.args.len() {
            continue;
        }
        let mut next = subst.clone();
        if unify(first, fact, &mut next) && body_holds(rest, &next, facts) {
            return true;
        }
    }
    false
}

/// True when the clause covers the ground example (head unifies and the body holds).
pub fn covers(clause: &Clause, example: &Atom, bg: &Background) -> bool {
    let mut subst = Subst::new();
    if !unify(&clause.head, example, &mut subst) {
        return false;
    }
    body_holds(&clause.body, &subst, &bg.facts)
}

/// Every ground instance of the head over `domain` for which the body holds.
pub fn extension(clause: &Clause, bg: &Background, domain: &[u32]) -> Vec<Vec<u32>> {
    let arity = clause.head.args.len();
    let mut out = Vec::new();
    let mut tuple = vec![0u32; arity];
    loop {
        let head = Atom {
            pred: clause.head.pred.clone(),
            args: tuple.iter().map(|&c| Term::Const(c)).collect(),
        };
        let mut subst = Subst::new();
        if unify(&clause.head, &head, &mut subst) && body_holds(&clause.body, &subst, &bg.facts) {
            out.push(tuple.clone());
        }
        // odometer over the domain
        let mut i = arity;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            tuple[i] += 1;
            if let Some(pos) = domain.iter().position(|&d| d == tuple[i]) {
                let _ = pos;
                break;
            }
            tuple[i] = domain[0];
        }
    }
}

/// The literals the learner may add to a body, given the variables in play.
fn candidate_literals(bg: &Background, max_vars: u32) -> Vec<Atom> {
    let mut out = Vec::new();
    for sig in &bg.signature {
        // All tuples of arguments drawn from the variables, without repeating the same variable
        // in one atom position pattern more than necessary.
        let mut tuple = vec![0u32; sig.arity];
        loop {
            out.push(Atom {
                pred: sig.name.clone(),
                args: tuple.iter().map(|&v| Term::Var(v)).collect(),
            });
            let mut i = sig.arity;
            loop {
                if i == 0 {
                    break;
                }
                i -= 1;
                tuple[i] += 1;
                if tuple[i] >= max_vars {
                    tuple[i] = 0;
                } else {
                    break;
                }
            }
            if i == 0 && tuple[0] == 0 {
                break;
            }
        }
    }
    out
}

/// Learns the first clause (shortest body, then lexicographic) that covers every positive example
/// and no negative one.
pub fn learn(
    target: &str,
    arity: usize,
    positives: &[Atom],
    negatives: &[Atom],
    bg: &Background,
    max_body: usize,
    max_vars: u32,
) -> Option<Clause> {
    let head = Atom {
        pred: target.to_string(),
        args: (0..arity as u32).map(Term::Var).collect(),
    };
    let literals = candidate_literals(bg, max_vars);
    let all_positive = positives.iter().all(|_| true);

    // A body cannot be longer than the literals available to fill it. Without this the enumeration
    // below indexes past the literal list and panics, which a library API must never do.
    let max_body = max_body.min(literals.len());
    if max_body == 0 {
        return None;
    }

    for len in 1..=max_body {
        // Enumerate bodies of this length in a fixed order (indices strictly increasing).
        let mut idx: Vec<usize> = (0..len).collect();
        loop {
            let body: Vec<Atom> = idx.iter().map(|&i| literals[i].clone()).collect();
            // Safety: every head variable must occur in the body. Otherwise a clause like
            // `grandparent(V0,V1) :- parent(V2,V3)` would "cover" an example by ignoring it.
            let mut used = std::collections::BTreeSet::new();
            for atom in &body {
                for arg in &atom.args {
                    if let Term::Var(v) = arg {
                        used.insert(*v);
                    }
                }
            }
            if (0..arity as u32).any(|v| !used.contains(&v)) {
                // still advance the enumeration below
            } else {
                let clause = Clause { head: head.clone(), body };
                let covers_all = all_positive && positives.iter().all(|e| covers(&clause, e, bg));
                if covers_all && !negatives.iter().any(|e| covers(&clause, e, bg)) {
                    return Some(clause);
                }
            }
            // next combination
            let mut i = len;
            loop {
                if i == 0 {
                    idx.clear();
                    break;
                }
                i -= 1;
                if idx[i] != i + literals.len() - len {
                    idx[i] += 1;
                    for j in (i + 1)..len {
                        idx[j] = idx[j - 1] + 1;
                    }
                    break;
                }
                if i == 0 {
                    idx.clear();
                }
            }
            if idx.is_empty() {
                break;
            }
        }
    }
    None
}

/// The candidate a proposer hands to the kernel: the extension of the first clause that covers every
/// positive and no negative.
///
/// The extension is over the **whole** domain, not over the objects the examples mentioned. That is
/// the part a language model cannot do by itself: it has to say which of the objects it never saw
/// belong to the concept, and enumerating them is exactly where it stops. A learner can, because
/// the rule it found is a rule over the background facts.
///
/// Returns `None` when no clause within the bound separates the evidence. That is a normal answer
/// and not a failure: it is "this language cannot say it", and there is nothing for the kernel to
/// judge.
pub fn learn_extension(
    target: &str,
    positives: &[Atom],
    negatives: &[Atom],
    bg: &Background,
    max_body: usize,
    max_vars: u32,
    domain: &[u32],
) -> Option<(Clause, Vec<u32>)> {
    let clause = learn(target, 1, positives, negatives, bg, max_body, max_vars)?;
    let ext = extension(&clause, bg, domain);
    let ids: Vec<u32> = ext.into_iter().filter_map(|args| args.first().copied()).collect();
    Some((clause, ids))
}

/// Helper: a ground atom over constants.
pub fn ground(pred: &str, args: &[u32]) -> Atom {
    Atom { pred: pred.to_string(), args: args.iter().map(|&c| Term::Const(c)).collect() }
}

// ---------------------------------------------------------------------------
// Subsumption, coverage, refinement
// ---------------------------------------------------------------------------

/// Extends `theta` so that `general` matches `specific`, or fails.
fn match_atom(general: &Atom, specific: &Atom, theta: &mut Subst) -> bool {
    if general.pred != specific.pred || general.args.len() != specific.args.len() {
        return false;
    }
    for (g, s) in general.args.iter().zip(specific.args.iter()) {
        match g {
            Term::Const(c) => {
                if Term::Const(*c) != *s {
                    return false;
                }
            }
            Term::Var(v) => {
                let s_id = match s {
                    Term::Var(x) => *x,
                    Term::Const(x) => *x,
                };
                match theta.get(v) {
                    Some(bound) if *bound != s_id => return false,
                    Some(_) => {}
                    None => {
                        theta.insert(*v, s_id);
                    }
                }
            }
        }
    }
    true
}

fn subsumes_from(general: &[Atom], specific: &[Atom], used: &mut [bool], theta: &mut Subst) -> bool {
    match general.split_first() {
        None => true,
        Some((first, rest)) => {
            for (i, cand) in specific.iter().enumerate() {
                if used[i] {
                    continue;
                }
                let mut next = theta.clone();
                if !match_atom(first, cand, &mut next) {
                    continue;
                }
                used[i] = true;
                if subsumes_from(rest, specific, used, &mut next) {
                    return true;
                }
                used[i] = false;
            }
            false
        }
    }
}

/// True when some substitution maps every literal of `general` onto a distinct literal of
/// `specific` (head included), the standard theta-subsumption test with injective matching.
///
/// Popper carries no such test: its only subsumption is a Prolog query used to delete redundant
/// literals, and its real redundancy test is coverage on bitarrays. This one is general, which is
/// what a *necessity* criterion needs: to say "no clause in the language separates these two
/// objects" one has to compare clause sets, not just count covered examples.
pub fn subsumes(general: &Clause, specific: &Clause) -> bool {
    let mut g: Vec<Atom> = Vec::with_capacity(general.body.len() + 1);
    g.push(general.head.clone());
    g.extend(general.body.iter().cloned());
    let mut s: Vec<Atom> = Vec::with_capacity(specific.body.len() + 1);
    s.push(specific.head.clone());
    s.extend(specific.body.iter().cloned());
    if g.len() > s.len() {
        return false;
    }
    let mut used = vec![false; s.len()];
    let mut theta = Subst::new();
    subsumes_from(&g, &s, &mut used, &mut theta)
}

/// A bitset over examples, the representation Popper uses for its redundancy tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    words: Vec<u64>,
    len: usize,
}

impl Coverage {
    pub fn new(len: usize) -> Self {
        Coverage { words: vec![0; (len + 63) / 64], len }
    }

    pub fn set(&mut self, i: usize) {
        assert!(i < self.len);
        self.words[i / 64] |= 1 << (i % 64);
    }

    pub fn get(&self, i: usize) -> bool {
        i < self.len && (self.words[i / 64] >> (i % 64)) & 1 == 1
    }

    pub fn count(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn is_subset_of(&self, other: &Coverage) -> bool {
        self.words.iter().zip(other.words.iter()).all(|(a, b)| a & !b == 0)
    }
}

/// The coverage of a clause over a list of examples.
pub fn cover(clause: &Clause, examples: &[Atom], bg: &Background) -> Coverage {
    let mut out = Coverage::new(examples.len());
    for (i, e) in examples.iter().enumerate() {
        if covers(clause, e, bg) {
            out.set(i);
        }
    }
    out
}

/// Every clause obtained by adding one literal that keeps the head variables covered and introduces
/// at most one new variable. This is the literal-addition refinement Popper compiles into ASP; here
/// it is an ordinary function, which is what a Rust runtime can call and measure.
pub fn refine(clause: &Clause, bg: &Background, max_vars: u32) -> Vec<Clause> {
    let mut next = 0u32;
    for atom in std::iter::once(&clause.head).chain(clause.body.iter()) {
        for arg in &atom.args {
            if let Term::Var(v) = arg {
                next = next.max(*v + 1);
            }
        }
    }
    let mut out: Vec<Clause> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for sig in &bg.signature {
        let mut tuple = vec![0u32; sig.arity];
        loop {
            // The literal uses variables already in play, plus at most one fresh variable: the
            // tree-shaped growth the fragment is defined by.
            let fresh = tuple.iter().filter(|&&v| v == next).count();
            if fresh <= 1 && tuple.iter().all(|&v| v <= next) {
                let atom = Atom {
                    pred: sig.name.clone(),
                    args: tuple.iter().map(|&v| Term::Var(v)).collect(),
                };
                if !clause.body.contains(&atom) {
                    let mut body = clause.body.clone();
                    body.push(atom);
                    let mut used = std::collections::BTreeSet::new();
                    for a in body.iter() {
                        for arg in &a.args {
                            if let Term::Var(v) = arg {
                                used.insert(*v);
                            }
                        }
                    }
                    let head_vars_ok = clause
                        .head
                        .args
                        .iter()
                        .all(|arg| !matches!(arg, Term::Var(v) if !used.contains(v)));
                    if head_vars_ok && used.len() as u32 <= max_vars {
                        let candidate = Clause { head: clause.head.clone(), body };
                        if seen.insert(candidate.to_string()) {
                            out.push(candidate);
                        }
                    }
                }
            }
            // odometer over 0..=next
            let mut i = sig.arity;
            let mut carry = true;
            while carry && i > 0 {
                i -= 1;
                tuple[i] += 1;
                if tuple[i] > next {
                    tuple[i] = 0;
                } else {
                    carry = false;
                }
            }
            if carry {
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Relational distinguishability
// ---------------------------------------------------------------------------

/// The result of colour refinement over the ground facts.
///
/// Two objects get the same colour when no tree-shaped conjunctive pattern distinguishes them: a
/// colour is the set of unary predicates plus the multiset of (relation, position, colour) edges.
/// That is the decidable fragment this learner can express, and saying so precisely is the point;
/// general conjunctive queries would need a homomorphism test, which is NP-hard.
#[derive(Clone, Debug)]
pub struct Partition {
    pub colour: BTreeMap<u32, u32>,
    pub rounds: usize,
    /// `signature[round][colour]` = the unary predicates and edge items that define the colour.
    pub signature: Vec<BTreeMap<u32, ColourSignature>>,
}

/// The definition of one colour at one round.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColourSignature {
    pub unary: Vec<String>,
    pub edges: Vec<(String, u8, u32)>,
}

fn unary_holds(bg: &Background, pred: &str, x: u32) -> bool {
    bg.facts.iter().any(|f| f.pred == pred && f.args == [Term::Const(x)])
}

fn colour_signature(bg: &Background, colour: &BTreeMap<u32, u32>, x: u32) -> ColourSignature {
    let mut unary: Vec<String> = bg
        .signature
        .iter()
        .filter(|s| s.arity == 1)
        .map(|s| s.name.clone())
        .filter(|p| unary_holds(bg, p, x))
        .collect();
    unary.sort();
    let mut edges: Vec<(String, u8, u32)> = Vec::new();
    for f in &bg.facts {
        if f.args.len() != 2 {
            continue;
        }
        let (a, b) = (f.args[0], f.args[1]);
        if a == Term::Const(x) {
            if let Term::Const(o) = b {
                edges.push((f.pred.clone(), 0, *colour.get(&o).unwrap_or(&u32::MAX)));
            }
        }
        if b == Term::Const(x) {
            if let Term::Const(o) = a {
                edges.push((f.pred.clone(), 1, *colour.get(&o).unwrap_or(&u32::MAX)));
            }
        }
    }
    edges.sort();
    ColourSignature { unary, edges }
}

/// Refines colours until the partition stops changing.
///
/// Round 0 is the unary signature, so objects that differ in a unary predicate are separated
/// immediately; each later round adds the multiset of (relation, position, neighbour colour) items.
/// Refinement only splits classes, so the number of classes is the stopping signal.
pub fn colour_refine(bg: &Background, domain: &[u32]) -> Partition {
    let mut colour: BTreeMap<u32, u32> = BTreeMap::new();
    let mut ids: BTreeMap<Vec<String>, u32> = BTreeMap::new();
    let mut round_sig: BTreeMap<u32, ColourSignature> = BTreeMap::new();
    for &x in domain {
        let unary: Vec<String> = bg
            .signature
            .iter()
            .filter(|s| s.arity == 1)
            .map(|s| s.name.clone())
            .filter(|p| unary_holds(bg, p, x))
            .collect();
        let n = ids.len() as u32;
        let id = *ids.entry(unary.clone()).or_insert(n);
        colour.insert(x, id);
        round_sig.insert(id, ColourSignature { unary, edges: Vec::new() });
    }
    let mut history: Vec<BTreeMap<u32, ColourSignature>> = vec![round_sig];
    let mut rounds = 0;
    loop {
        let sigs: Vec<(u32, ColourSignature)> =
            domain.iter().map(|&x| (x, colour_signature(bg, &colour, x))).collect();
        let mut ids: BTreeMap<ColourSignature, u32> = BTreeMap::new();
        let mut next: BTreeMap<u32, u32> = BTreeMap::new();
        let mut round_sig: BTreeMap<u32, ColourSignature> = BTreeMap::new();
        for (x, s) in sigs {
            let n = ids.len() as u32;
            let id = *ids.entry(s.clone()).or_insert(n);
            next.insert(x, id);
            round_sig.insert(id, s);
        }
        let old_classes = colour.values().collect::<std::collections::BTreeSet<_>>().len();
        let new_classes = ids.len();
        colour = next;
        history.push(round_sig);
        rounds += 1;
        if new_classes == old_classes {
            break;
        }
    }
    Partition { colour, rounds, signature: history }
}

/// The pattern that holds of one colour and of nothing else, built from the refinement history.
fn colour_pattern(
    partition: &Partition,
    round: usize,
    colour: u32,
    next_var: &mut u32,
) -> Vec<Atom> {
    let Some(sig) = partition.signature.get(round).and_then(|m| m.get(&colour)) else {
        return Vec::new();
    };
    let mut body: Vec<Atom> = sig
        .unary
        .iter()
        .map(|p| Atom { pred: p.clone(), args: vec![Term::Var(0)] })
        .collect();
    if round == 0 {
        return body;
    }
    // Edges of this colour point at colours of the previous round.
    for (pred, pos, child) in &sig.edges {
        let child_var = *next_var;
        *next_var += 1;
        let args = if *pos == 0 {
            vec![Term::Var(0), Term::Var(child_var)]
        } else {
            vec![Term::Var(child_var), Term::Var(0)]
        };
        body.push(Atom { pred: pred.clone(), args });
        let sub = colour_pattern(partition, round - 1, *child, next_var);
        for atom in sub {
            let remapped: Vec<Term> = atom
                .args
                .iter()
                .map(|t| match t {
                    Term::Var(0) => Term::Var(child_var),
                    other => *other,
                })
                .collect();
            body.push(Atom { pred: atom.pred.clone(), args: remapped });
        }
    }
    body
}

fn unary_set(bg: &Background, x: u32) -> Vec<String> {
    bg.signature
        .iter()
        .filter(|s| s.arity == 1)
        .map(|s| s.name.clone())
        .filter(|p| unary_holds(bg, p, x))
        .collect()
}

/// True when the pattern holds of the object: the head unifies with `sep(obj)` and the body holds.
fn pattern_holds_of(clause: &Clause, bg: &Background, obj: u32) -> bool {
    covers(clause, &ground("sep", &[obj]), bg)
}

/// Greedily drops literals while the clause still holds of `x` and not of `y`.
///
/// The refinement history yields an exact class description, which can carry edges that the
/// separation does not need. A witness is more useful small: it is a proposition a person reads and
/// a kernel checks, not a dump of the neighbourhood.
pub fn minimize_pattern(clause: &Clause, bg: &Background, x: u32, y: u32) -> Clause {
    let mut body = clause.body.clone();
    let mut i = 0;
    while i < body.len() {
        let mut trial = body.clone();
        trial.remove(i);
        let candidate = Clause { head: clause.head.clone(), body: trial.clone() };
        if pattern_holds_of(&candidate, bg, x) && !pattern_holds_of(&candidate, bg, y) {
            body = trial;
        } else {
            i += 1;
        }
    }
    Clause { head: clause.head.clone(), body }
}

/// A clause `sep(V0) :- pattern` that holds of `x` and not of `y`, when the tree-shaped fragment
/// can tell them apart. `None` is the interesting answer: no pattern in the language separates the
/// two objects, so the current vocabulary is insufficient in a way that adding more of the same
/// relations cannot fix.
pub fn separating_pattern(
    bg: &Background,
    domain: &[u32],
    x: u32,
    y: u32,
) -> Option<Clause> {
    let partition = colour_refine(bg, domain);
    if partition.colour.get(&x) == partition.colour.get(&y) {
        return None;
    }
    // A unary predicate may already decide it.
    let ux = unary_set(bg, x);
    let uy = unary_set(bg, y);
    if let Some(p) = ux.iter().find(|p| !uy.contains(p)) {
        return Some(Clause {
            head: Atom { pred: "sep".to_string(), args: vec![Term::Var(0)] },
            body: vec![Atom { pred: p.clone(), args: vec![Term::Var(0)] }],
        });
    }
    let colour = *partition.colour.get(&x)?;
    let mut next_var = 1u32;
    let body = colour_pattern(&partition, partition.rounds, colour, &mut next_var);
    if body.is_empty() {
        return None;
    }
    let clause = Clause {
        head: Atom { pred: "sep".to_string(), args: vec![Term::Var(0)] },
        body,
    };
    Some(minimize_pattern(&clause, bg, x, y))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_learned_clause_is_extended_over_the_whole_domain() {
        // One-place attributes over five objects. The positive example is the only object with both
        // attributes; the negative one has the second attribute alone. The clause the learner finds
        // is "the first attribute", and its extension has to name every object it holds of -- the
        // ones nobody mentioned included. That enumeration is the part a model cannot do, and it is
        // what makes the learner a proposer rather than a lookup.
        let mut bg = Background::new(vec![
            Signature { name: "a0".to_string(), arity: 1 },
            Signature { name: "a1".to_string(), arity: 1 },
        ]);
        for id in [0, 1, 4] {
            bg.fact("a0", &[id]);
        }
        for id in [2, 3, 4] {
            bg.fact("a1", &[id]);
        }
        let pos = vec![ground("target", &[4])];
        let neg = vec![ground("target", &[2])];
        let domain: Vec<u32> = (0..5).collect();
        let (clause, ids) =
            learn_extension("target", &pos, &neg, &bg, 2, 1, &domain).expect("a rule exists here");
        assert_eq!(clause.body.len(), 1, "the shortest separating body is one literal");
        assert_eq!(ids, vec![0, 1, 4], "the extension covers the objects the rule holds of");
        // The objects nobody mentioned are in it, and the negative example is not.
        assert!(ids.contains(&0) && ids.contains(&1));
        assert!(!ids.contains(&2) && !ids.contains(&3));
    }

    #[test]
    fn an_unlearnable_case_returns_nothing_rather_than_a_guess() {
        // Every object has the same attribute, so nothing separates the evidence. "Nothing" is the
        // answer: a proposer that returned a set here would be reporting a guess as a rule.
        let mut bg = Background::new(vec![Signature { name: "a0".to_string(), arity: 1 }]);
        for id in 0..4 {
            bg.fact("a0", &[id]);
        }
        let pos = vec![ground("target", &[0])];
        let neg = vec![ground("target", &[1])];
        let domain: Vec<u32> = (0..4).collect();
        assert!(learn_extension("target", &pos, &neg, &bg, 2, 1, &domain).is_none());
    }

    #[test]
    fn a_body_longer_than_the_available_literals_returns_none_instead_of_panicking() {
        // Regression: the enumeration used to index past the literal list when the requested body
        // length exceeded the number of candidate literals.
        let bg = Background::new(vec![Signature { name: "unary".to_string(), arity: 1 }]);
        let pos = vec![ground("p", &[0])];
        let neg = vec![ground("p", &[1])];
        let learned = learn("p", 1, &pos, &neg, &bg, 9, 1);
        assert!(learned.is_none(), "an empty background cannot yield a clause");
    }

    #[test]
    fn an_empty_background_returns_none() {
        let bg = Background::default();
        let pos = vec![ground("p", &[0])];
        let neg = vec![ground("p", &[1])];
        assert!(learn("p", 1, &pos, &neg, &bg, 3, 1).is_none());
    }

    /// Two family chains: 0 -> 1 -> 2 -> 3 and 4 -> 5.
    fn family() -> Background {
        let mut bg = Background::new(vec![
            Signature { name: "parent".to_string(), arity: 2 },
            Signature { name: "same_city".to_string(), arity: 2 },
        ]);
        for (a, b) in [(0, 1), (1, 2), (2, 3), (4, 5)] {
            bg.fact("parent", &[a, b]);
        }
        for (a, b) in [(0, 1), (1, 2), (4, 5)] {
            bg.fact("same_city", &[a, b]);
        }
        bg
    }

    #[test]
    fn a_two_step_chain_is_learned_as_a_conjunction() {
        let bg = family();
        let positives = [ground("grandparent", &[0, 2]), ground("grandparent", &[1, 3])];
        let negatives = [ground("grandparent", &[0, 3]), ground("grandparent", &[2, 3])];
        let clause = learn("grandparent", 2, &positives, &negatives, &bg, 2, 3).expect("a clause");
        assert_eq!(clause.body.len(), 2);
        assert!(clause.to_string().contains("parent"));
        assert!(positives.iter().all(|e| covers(&clause, e, &bg)));
        assert!(negatives.iter().all(|e| !covers(&clause, e, &bg)));
    }

    #[test]
    fn a_negative_forces_a_longer_or_absent_clause() {
        let bg = family();
        let positives = [ground("grandparent", &[0, 2])];
        // (0, 1) is a parent, so a one-literal clause parent(V0,V1) would cover it.
        let negatives = [ground("grandparent", &[0, 1])];
        let clause = learn("grandparent", 2, &positives, &negatives, &bg, 2, 3).expect("a clause");
        assert!(negatives.iter().all(|e| !covers(&clause, e, &bg)));
    }

    #[test]
    fn nothing_covers_an_impossible_example() {
        let bg = family();
        let positives = [ground("grandparent", &[6, 0])]; // person 6 appears in no fact
        let negatives = [ground("grandparent", &[0, 1])];
        assert!(learn("grandparent", 2, &positives, &negatives, &bg, 2, 3).is_none());
    }

    #[test]
    fn the_extension_is_every_satisfying_tuple() {
        let bg = family();
        let clause = Clause {
            head: Atom { pred: "grandparent".to_string(), args: vec![Term::Var(0), Term::Var(1)] },
            body: vec![
                Atom { pred: "parent".to_string(), args: vec![Term::Var(0), Term::Var(2)] },
                Atom { pred: "parent".to_string(), args: vec![Term::Var(2), Term::Var(1)] },
            ],
        };
        let mut ext = extension(&clause, &bg, &[0, 1, 2, 3, 4, 5]);
        ext.sort();
        assert_eq!(ext, vec![vec![0, 2], vec![1, 3]]);
    }

    #[test]
    fn learning_is_deterministic() {
        let bg = family();
        let positives = [ground("grandparent", &[0, 2]), ground("grandparent", &[1, 3])];
        let negatives = [ground("grandparent", &[0, 3])];
        let a = learn("grandparent", 2, &positives, &negatives, &bg, 2, 3);
        let b = learn("grandparent", 2, &positives, &negatives, &bg, 2, 3);
        assert_eq!(a, b);
    }

    fn var(v: u32) -> Term {
        Term::Var(v)
    }

    fn clause(head: Atom, body: Vec<Atom>) -> Clause {
        Clause { head, body }
    }

    fn atom(pred: &str, args: Vec<Term>) -> Atom {
        Atom { pred: pred.to_string(), args }
    }

    #[test]
    fn subsumption_accepts_a_more_specific_clause() {
        let general = clause(atom("p", vec![var(0)]), vec![atom("q", vec![var(0)])]);
        let specific = clause(
            atom("p", vec![var(0)]),
            vec![atom("q", vec![var(0)]), atom("r", vec![var(0)])],
        );
        assert!(subsumes(&general, &specific));
        assert!(!subsumes(&specific, &general));
    }

    #[test]
    fn subsumption_ignores_variable_names() {
        let a = clause(atom("p", vec![var(0)]), vec![atom("q", vec![var(0)])]);
        let b = clause(atom("p", vec![var(7)]), vec![atom("q", vec![var(7)])]);
        assert!(subsumes(&a, &b));
        assert!(subsumes(&b, &a));
    }

    #[test]
    fn subsumption_rejects_a_different_predicate() {
        let a = clause(atom("p", vec![var(0)]), vec![atom("q", vec![var(0)])]);
        let b = clause(atom("p", vec![var(0)]), vec![atom("r", vec![var(0)])]);
        assert!(!subsumes(&a, &b));
    }

    #[test]
    fn coverage_bitset_matches_the_ground_test() {
        let bg = family();
        let clause = clause(
            atom("grandparent", vec![var(0), var(1)]),
            vec![
                atom("parent", vec![var(0), var(2)]),
                atom("parent", vec![var(2), var(1)]),
            ],
        );
        let examples = [
            ground("grandparent", &[0, 2]),
            ground("grandparent", &[0, 3]),
            ground("grandparent", &[1, 3]),
        ];
        let bits = cover(&clause, &examples, &bg);
        for (i, e) in examples.iter().enumerate() {
            assert_eq!(bits.get(i), covers(&clause, e, &bg), "example {i}");
        }
        assert_eq!(bits.count(), 2);
    }

    #[test]
    fn refinement_adds_one_literal_and_keeps_the_head_covered() {
        let bg = family();
        let start = clause(atom("grandparent", vec![var(0), var(1)]), vec![]);
        let children = refine(&start, &bg, 3);
        assert!(!children.is_empty());
        for child in &children {
            assert_eq!(child.body.len(), 1);
            let used: std::collections::BTreeSet<u32> = child
                .body
                .iter()
                .flat_map(|a| a.args.iter())
                .filter_map(|t| match t {
                    Term::Var(v) => Some(*v),
                    _ => None,
                })
                .collect();
            assert!(used.contains(&0) && used.contains(&1), "head vars must occur");
        }
    }

    fn trees() -> Background {
        let mut bg = Background::new(vec![
            Signature { name: "red".to_string(), arity: 1 },
            Signature { name: "blue".to_string(), arity: 1 },
            Signature { name: "green".to_string(), arity: 1 },
            Signature { name: "link".to_string(), arity: 2 },
        ]);
        bg.fact("red", &[0]);
        bg.fact("red", &[1]);
        bg.fact("link", &[0, 2]);
        bg.fact("link", &[1, 3]);
        bg.fact("green", &[2]);
        bg.fact("blue", &[3]);
        bg
    }

    #[test]
    fn unary_predicates_separate_at_round_zero() {
        let bg = trees();
        let domain = [0u32, 1, 2, 3];
        let p = separating_pattern(&bg, &domain, 0, 2).expect("red separates 0 from 2");
        let ext = extension(&p, &bg, &domain);
        assert!(ext.contains(&vec![0]));
        assert!(!ext.contains(&vec![2]));
        assert!(separating_pattern(&bg, &domain, 2, 0).is_some());
    }

    #[test]
    fn a_relation_is_necessary_when_unary_predicates_collide() {
        let bg = trees();
        let domain = [0u32, 1, 2, 3];
        let p = separating_pattern(&bg, &domain, 0, 1).expect("a link pattern separates 0 from 1");
        assert!(p.body.iter().any(|a| a.pred == "link"), "pattern: {}", p.to_string());
        assert!(p.body.iter().any(|a| a.pred == "green"), "pattern: {}", p.to_string());
        assert_eq!(p.body.len(), 2, "the witness should be minimal: {}", p.to_string());
        let ext = extension(&p, &bg, &domain);
        assert_eq!(ext, vec![vec![0]]);
    }

    #[test]
    fn bisimilar_objects_have_no_separating_pattern() {
        let mut bg = Background::new(vec![
            Signature { name: "red".to_string(), arity: 1 },
            Signature { name: "green".to_string(), arity: 1 },
            Signature { name: "link".to_string(), arity: 2 },
        ]);
        for x in [0u32, 1] {
            bg.fact("red", &[x]);
        }
        for (a, b) in [(0, 2), (1, 3)] {
            bg.fact("link", &[a, b]);
        }
        for x in [2u32, 3] {
            bg.fact("green", &[x]);
        }
        let domain = [0u32, 1, 2, 3];
        assert!(separating_pattern(&bg, &domain, 0, 1).is_none());
        assert!(separating_pattern(&bg, &domain, 2, 3).is_none());
    }
}
