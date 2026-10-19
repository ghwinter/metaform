//! Structure mapping for metaform.
//!
//! Clean-room implementation of the structure-mapping kernel: given two relational systems, find
//! the alignments that are structurally consistent, prefer the ones with deeper and more
//! connected common structure, and project the unmatched parts of the base as candidate
//! inferences.
//!
//! The theory is Falkenhainer, Forbus and Gentner's structure-mapping engine; the scoring here
//! follows the same principles (structural consistency, one-to-one correspondences,
//! systematicity) without copying any implementation. The additions this repository needs are
//! that the output is deterministic, that a mapping can be projected back as candidate
//! statements for the exact kernel to check, and that the whole thing has no dependencies.
//!
//! Statements are defined bottom-up: a statement may only refer to statements with smaller ids.
//! That is what makes the scoring pass a single ordered sweep.

use std::collections::BTreeSet;

pub type EntityId = u32;
pub type StatementId = u32;

/// An argument of a statement: either an entity or a nested statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Term {
    Entity(EntityId),
    Statement(StatementId),
}

/// A relation applied to arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    pub relation: String,
    pub args: Vec<Term>,
}

/// A relational system: entities plus statements over them.
#[derive(Clone, Debug, Default)]
pub struct System {
    pub entities: usize,
    pub statements: Vec<Statement>,
    /// Relations whose argument order carries no meaning. SME4 gives them a twin match with the
    /// arguments reversed; the flag lives on the system because the relation name alone cannot say
    /// whether `parallel(a, b)` and `parallel(b, a)` are the same fact.
    pub symmetric: BTreeSet<String>,
    /// Relations whose arguments are unordered at any arity. The matcher tries every permutation of
    /// a target statement's arguments, not just the reversal; `between(a, b, c)` and
    /// `between(c, a, b)` are the same fact.
    pub commutative: BTreeSet<String>,
}

impl System {
    pub fn new(entities: usize) -> Self {
        System {
            entities,
            statements: Vec::new(),
            symmetric: BTreeSet::new(),
            commutative: BTreeSet::new(),
        }
    }

    /// Adds a statement and returns its id. Args may only name earlier statements.
    pub fn push(&mut self, relation: &str, args: Vec<Term>) -> StatementId {
        self.statements.push(Statement { relation: relation.to_string(), args });
        (self.statements.len() - 1) as StatementId
    }

    /// Adds a statement of a symmetric relation, so the matcher also tries the reversed alignment.
    /// This is the binary rule; for an unordered relation of any arity use
    /// [`System::push_commutative`].
    pub fn push_symmetric(&mut self, relation: &str, args: Vec<Term>) -> StatementId {
        self.symmetric.insert(relation.to_string());
        self.push(relation, args)
    }

    /// Adds a statement of a commutative relation: the arguments are unordered, so the matcher
    /// tries every permutation of a target statement's arguments, deduplicated. Past
    /// [`MAX_COMMUTATIVE_ARITY`] it falls back to the binary rule (identity and reversal), because
    /// enumerating n! stops being worthwhile.
    pub fn push_commutative(&mut self, relation: &str, args: Vec<Term>) -> StatementId {
        self.commutative.insert(relation.to_string());
        self.push(relation, args)
    }

    fn statement(&self, id: StatementId) -> &Statement {
        &self.statements[id as usize]
    }
}

/// A structural match between one base statement and one target statement, including every
/// nested statement and entity correspondence it implies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub base: StatementId,
    pub target: StatementId,
    pub statements: Vec<(StatementId, StatementId)>,
    pub entities: Vec<(EntityId, EntityId)>,
}

impl Match {
    pub fn size(&self) -> usize {
        self.statements.len()
    }
}

/// How good an alignment is. The numbers are kept separate on purpose: a single score would hide
/// whether an alignment is deep, broad, or fragmented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    /// Matched statements.
    pub matched: usize,
    /// Sum of the statement weights, where a statement's weight is one plus the weights of its
    /// matched children: deep common structure counts more than many isolated matches.
    pub systematicity: u32,
    /// Connected components of the matched structure. Fewer is better.
    pub components: usize,
}

/// One alignment of base to target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub statements: Vec<(StatementId, StatementId)>,
    pub entities: Vec<(EntityId, EntityId)>,
    pub score: Score,
}

impl Mapping {
    pub fn target_of(&self, base_entity: EntityId) -> Option<EntityId> {
        self.entities.iter().find(|(b, _)| *b == base_entity).map(|(_, t)| *t)
    }
}

/// Commutative relations try every permutation of a target statement's arguments. `n!` grows too
/// fast to be worth enumerating beyond this: a commutative relation of higher arity falls back to
/// the binary rule, so only the identity and the reversal are tried. The cap keeps a local match
/// sweep bounded at 5! = 120 alignments per statement pair.
pub const MAX_COMMUTATIVE_ARITY: usize = 5;

/// Every permutation of `0..n`, in lexicographic order with the identity first. This is the
/// classic next-permutation walk (rightmost ascent, smallest larger successor, reverse the suffix),
/// so it needs no dependency and no recursion. `n` is bounded by [`MAX_COMMUTATIVE_ARITY`].
fn permutations(n: usize) -> Vec<Vec<usize>> {
    let mut current: Vec<usize> = (0..n).collect();
    let mut out = vec![current.clone()];
    loop {
        let ascent =
            (0..current.len().saturating_sub(1)).rev().find(|&i| current[i] < current[i + 1]);
        let Some(i) = ascent else { break };
        let j = (i + 1..current.len())
            .rev()
            .find(|&j| current[i] < current[j])
            .expect("a suffix entry larger than current[i] exists");
        current.swap(i, j);
        current[i + 1..].reverse();
        out.push(current.clone());
    }
    out
}

/// Matches one base statement against one target statement, recursively. Returns `None` when the
/// two cannot be aligned, in which case nothing is added to the enclosing match.
///
/// `permutation[i]` names the target argument aligned with base argument `i`; `None` is the
/// identity, which is what nested statements are matched with (a twin lives at the level of the
/// statement that owns it).
fn match_statements(
    base: &System,
    target: &System,
    b: StatementId,
    t: StatementId,
    permutation: Option<&[usize]>,
) -> Option<Match> {
    let bs = base.statement(b);
    let ts = target.statement(t);
    if bs.relation != ts.relation || bs.args.len() != ts.args.len() {
        return None;
    }

    let mut statements: Vec<(StatementId, StatementId)> = vec![(b, t)];
    let mut entities: Vec<(EntityId, EntityId)> = Vec::new();

    for (i, ba) in bs.args.iter().enumerate() {
        let ta = &ts.args[permutation.map_or(i, |p| p[i])];
        match (ba, ta) {
            (Term::Entity(be), Term::Entity(te)) => {
                if let Some((_, mapped)) = entities.iter().find(|(x, _)| x == be) {
                    if mapped != te {
                        return None;
                    }
                }
                if let Some((mapped, _)) = entities.iter().find(|(_, y)| y == te) {
                    if mapped != be {
                        return None;
                    }
                }
                if !entities.contains(&(*be, *te)) {
                    entities.push((*be, *te));
                }
            }
            (Term::Statement(bx), Term::Statement(tx)) => {
                // The twin of a symmetric or commutative statement is at this level only; nested
                // statements are matched by their own flags.
                let nested = match_statements(base, target, *bx, *tx, None)?;
                for pair in nested.entities {
                    if let Some((_, mapped)) = entities.iter().find(|(x, _)| *x == pair.0) {
                        if *mapped != pair.1 {
                            return None;
                        }
                    }
                    if let Some((mapped, _)) = entities.iter().find(|(_, y)| *y == pair.1) {
                        if *mapped != pair.0 {
                            return None;
                        }
                    }
                    if !entities.contains(&pair) {
                        entities.push(pair);
                    }
                }
                for pair in nested.statements {
                    if !statements.contains(&pair) {
                        statements.push(pair);
                    }
                }
            }
            _ => return None,
        }
    }

    Some(Match { base: b, target: t, statements, entities })
}

/// Every local match hypothesis between the two systems, in a deterministic order.
pub fn local_matches(base: &System, target: &System) -> Vec<Match> {
    let mut out = Vec::new();
    for b in 0..base.statements.len() as StatementId {
        let bs = base.statement(b);
        // The argument orders to try beyond the identity: every permutation of the target's
        // arguments for a commutative relation, only the reversal for a symmetric one. The arity
        // cap falls back to the reversal, so the cost of a sweep stays bounded. Like the symmetric
        // flag, this is read from the base: the base says how its own relation is to be read.
        let n = bs.args.len();
        let argument_orders: Vec<Vec<usize>> = if n <= 1 {
            Vec::new()
        } else if base.commutative.contains(&bs.relation) && n <= MAX_COMMUTATIVE_ARITY {
            permutations(n)
        } else if base.commutative.contains(&bs.relation) || base.symmetric.contains(&bs.relation) {
            vec![(0..n).rev().collect()]
        } else {
            Vec::new()
        };
        for t in 0..target.statements.len() as StatementId {
            if let Some(m) = match_statements(base, target, b, t, None) {
                out.push(m);
            }
            for order in &argument_orders {
                if let Some(twin) = match_statements(base, target, b, t, Some(order)) {
                    if !out.contains(&twin) {
                        out.push(twin);
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| {
        y.size()
            .cmp(&x.size())
            .then(x.base.cmp(&y.base))
            .then(x.target.cmp(&y.target))
    });
    out
}

/// True when a correspondence set and a candidate match can belong to the same alignment: the
/// correspondences must be one-to-one and must agree wherever they overlap.
///
/// SME keeps a nogood table and shares each match's nogoods through its descendants, so a later
/// merge cannot reuse a base or target expression. Here the descendant closure is the match itself:
/// `Match::statements` and `Match::entities` already contain every nested pair a match implies, so
/// checking the candidate's closure against the alignment is equivalent, and the nogood bookkeeping
/// is subsumed rather than omitted. `every_mapping_is_one_to_one_and_duplicate_free` in the golden
/// geometry tests pins the resulting property on the corpus pair.
fn compatible_with(
    statements: &[(StatementId, StatementId)],
    entities: &[(EntityId, EntityId)],
    candidate: &Match,
) -> bool {
    for (be, te) in entities {
        if let Some((_, other)) = candidate.entities.iter().find(|(x, _)| x == be) {
            if other != te {
                return false;
            }
        }
        if let Some((other, _)) = candidate.entities.iter().find(|(_, y)| y == te) {
            if other != be {
                return false;
            }
        }
    }
    for (bs, ts) in statements {
        if let Some((_, other)) = candidate.statements.iter().find(|(x, _)| x == bs) {
            if other != ts {
                return false;
            }
        }
        if let Some((other, _)) = candidate.statements.iter().find(|(_, y)| y == ts) {
            if other != bs {
                return false;
            }
        }
    }
    true
}

fn merge(into: &mut Mapping, m: &Match) {
    for pair in &m.statements {
        if !into.statements.contains(pair) {
            into.statements.push(*pair);
        }
    }
    for pair in &m.entities {
        if !into.entities.contains(pair) {
            into.entities.push(*pair);
        }
    }
}

/// Scores an alignment. `components` counts connected pieces of the matched structure.
fn score_mapping(base: &System, target: &System, m: &Mapping) -> Score {
    let mut statements = m.statements.clone();
    statements.sort();
    let matched: BTreeSet<StatementId> = statements.iter().map(|(b, _)| *b).collect();

    // Weights: statements are defined bottom-up, so one ascending sweep is enough.
    let mut weight: Vec<u32> = vec![0; base.statements.len()];
    for &b in &matched {
        let mut w = 1u32;
        for arg in &base.statement(b).args {
            if let Term::Statement(child) = arg {
                if matched.contains(child) {
                    w += weight[*child as usize];
                }
            }
        }
        weight[b as usize] = w;
    }
    let systematicity: u32 = matched.iter().map(|b| weight[*b as usize]).sum();

    // Connected components over parent-child links among matched statements.
    let mut parent: Vec<usize> = (0..matched.len()).collect();
    let index = |s: StatementId| statements.binary_search_by_key(&s, |(b, _)| *b).unwrap();
    fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for (b, _) in &statements {
        for arg in &base.statement(*b).args {
            if let Term::Statement(child) = arg {
                if matched.contains(child) {
                    let x = find(&mut parent, index(*b));
                    let y = find(&mut parent, index(*child));
                    parent[x] = y;
                }
            }
        }
    }
    let mut roots = BTreeSet::new();
    for i in 0..matched.len() {
        let r = find(&mut parent, i);
        roots.insert(r);
    }

    let _ = target;
    Score { matched: matched.len(), systematicity, components: roots.len() }
}

/// The evidence arithmetic of SME4, measured from its own dehydrated runs.
///
/// A local match is worth 5.0e-4; a parent receives eight times the evidence of each child
/// (`trickle-down` 8.0). The corpus pins this: in the geometry runs an entity that is the argument
/// of nine matched expressions scores exactly 9 x 8 x 5.0e-4 = 0.036. Adopting the numbers rather
/// than inventing a score makes the port comparable with the original.
pub const LOCAL_EVIDENCE: f64 = 5.0e-4;
pub const TRICKLE_DOWN: f64 = 8.0;

/// SME4's greedy cutoff: an alignment scoring below 0.8 of the best is not worth reporting.
pub const GREEDY_CUTOFF: f64 = 0.8;

/// The evidence of one alignment, split into the part carried by expressions and the part carried
/// by entities.
#[derive(Clone, Debug, Default)]
pub struct Evidence {
    pub expressions: f64,
    pub entities: std::collections::BTreeMap<EntityId, f64>,
    pub total: f64,
}

/// Computes SME4's evidence over an alignment. Statements are defined bottom-up, so one ascending
/// sweep computes every expression's evidence before the entities that use it.
///
/// Entity evidence follows the mapping's correspondences, not argument position: a match produced
/// by a symmetric twin has its target arguments reversed, so a positional zip would pair the wrong
/// entities and credit neither. An entity argument counts when its mapped target entity appears
/// anywhere in the matched target statement's arguments.
pub fn evidence(base: &System, target: &System, m: &Mapping) -> Evidence {
    let lookup: std::collections::BTreeMap<StatementId, StatementId> =
        m.statements.iter().cloned().collect();
    let mut ev: std::collections::BTreeMap<StatementId, f64> = std::collections::BTreeMap::new();
    for &b in lookup.keys() {
        let bs = base.statement(b);
        let mut children = 0.0;
        for arg in &bs.args {
            if let Term::Statement(child) = arg {
                if let Some(v) = ev.get(child) {
                    children += *v;
                }
            }
        }
        ev.insert(b, LOCAL_EVIDENCE + TRICKLE_DOWN * children);
    }
    let expressions: f64 = ev.values().sum();
    let mut entities: std::collections::BTreeMap<EntityId, f64> = std::collections::BTreeMap::new();
    for (&b, &t) in &lookup {
        let bs = base.statement(b);
        let ts = target.statement(t);
        for ba in &bs.args {
            if let Term::Entity(be) = ba {
                if let Some(te) = m.target_of(*be) {
                    if ts.args.iter().any(|arg| *arg == Term::Entity(te)) {
                        *entities.entry(*be).or_default() += TRICKLE_DOWN * ev[&b];
                    }
                }
            }
        }
    }
    let total = expressions + entities.values().sum::<f64>();
    Evidence { expressions, entities, total }
}

/// All alignments worth reporting, best first.
///
/// Every local match is tried once as a seed and greedily extended with compatible matches; the
/// resulting alignments are deduplicated and sorted. This is the greedy merge of the original
/// engine in its simplest form, kept deterministic.
pub fn map(base: &System, target: &System, max_mappings: usize) -> Vec<Mapping> {
    let matches = local_matches(base, target);
    let mut mappings: Vec<Mapping> = Vec::new();

    for seed in &matches {
        let mut m = Mapping { statements: Vec::new(), entities: Vec::new(), score: Score { matched: 0, systematicity: 0, components: 0 } };
        merge(&mut m, seed);
        for candidate in &matches {
            if m.statements.contains(&(candidate.base, candidate.target)) {
                continue;
            }
            if compatible_with(&m.statements, &m.entities, candidate) {
                merge(&mut m, candidate);
            }
        }
        m.statements.sort();
        m.entities.sort();
        m.score = score_mapping(base, target, &m);
        if !mappings.iter().any(|other| other.statements == m.statements) {
            mappings.push(m);
        }
    }

    mappings.sort_by(|a, b| {
        b.score
            .systematicity
            .cmp(&a.score.systematicity)
            .then(b.score.matched.cmp(&a.score.matched))
            .then(a.score.components.cmp(&b.score.components))
            .then(a.statements.cmp(&b.statements))
    });
    if let Some(best) = mappings.first().map(|m| m.score.systematicity) {
        let floor = best as f64 * GREEDY_CUTOFF;
        mappings.retain(|m| m.score.systematicity as f64 >= floor);
    }
    mappings.truncate(max_mappings);
    mappings
}

/// A candidate inference that may stand on entities the target does not have.
///
/// SME4 writes such entities as skolems: fresh individuals that the inference is about, not ones
/// the target is known to contain. The ids here start at `target.entities`, so a caller can tell a
/// skolem from a real entity by the bound, and `skolems` says which base entity each one stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateInference {
    pub statement: Statement,
    pub skolems: Vec<(EntityId, EntityId)>,
}

/// Candidate inferences that keep unmapped base entities as skolems instead of dropping the
/// statement. This is the SME4 behaviour: the projection is proposed, and what it is about is
/// marked, not silently discarded.
pub fn candidate_inferences_with_skolems(
    base: &System,
    target: &System,
    m: &Mapping,
) -> Vec<CandidateInference> {
    let matched: BTreeSet<StatementId> = m.statements.iter().map(|(b, _)| *b).collect();
    let mut out: Vec<CandidateInference> = Vec::new();
    let mut skolems: Vec<(EntityId, EntityId)> = Vec::new();
    let mut next_skolem = target.entities as EntityId;

    for (b, st) in base.statements.iter().enumerate() {
        let b = b as StatementId;
        if matched.contains(&b) {
            continue;
        }
        let mut args = Vec::with_capacity(st.args.len());
        let mut complete = true;
        for arg in &st.args {
            match arg {
                Term::Entity(e) => {
                    if let Some(t) = m.target_of(*e) {
                        args.push(Term::Entity(t));
                    } else if let Some((_, s)) = skolems.iter().find(|(be, _)| be == e) {
                        args.push(Term::Entity(*s));
                    } else {
                        let s = next_skolem;
                        next_skolem += 1;
                        skolems.push((*e, s));
                        args.push(Term::Entity(s));
                    }
                }
                Term::Statement(s) => match m.statements.iter().find(|(bs, _)| bs == s) {
                    Some((_, ts)) => args.push(Term::Statement(*ts)),
                    None => {
                        complete = false;
                        break;
                    }
                },
            }
        }
        if !complete {
            continue;
        }
        let projected = Statement { relation: st.relation.clone(), args };
        if !target.statements.contains(&projected)
            && !out.iter().any(|c| c.statement == projected)
        {
            out.push(CandidateInference { statement: projected, skolems: skolems.clone() });
        }
    }
    out
}

/// The statements of the base that the mapping does not match but whose arguments are all mapped:
/// the base's unmatched structure, projected onto the target. These are candidate inferences, not
/// conclusions; a caller must check them.
pub fn candidate_inferences(base: &System, target: &System, m: &Mapping) -> Vec<Statement> {
    let matched: BTreeSet<StatementId> = m.statements.iter().map(|(b, _)| *b).collect();
    let mut out: Vec<Statement> = Vec::new();

    for (b, st) in base.statements.iter().enumerate() {
        let b = b as StatementId;
        if matched.contains(&b) {
            continue;
        }
        let mut args = Vec::with_capacity(st.args.len());
        let mut complete = true;
        for arg in &st.args {
            match arg {
                Term::Entity(e) => match m.target_of(*e) {
                    Some(t) => args.push(Term::Entity(t)),
                    None => {
                        complete = false;
                        break;
                    }
                },
                Term::Statement(s) => {
                    match m.statements.iter().find(|(bs, _)| bs == s) {
                        Some((_, ts)) => args.push(Term::Statement(*ts)),
                        None => {
                            complete = false;
                            break;
                        }
                    }
                }
            }
        }
        if !complete {
            continue;
        }
        let projected = Statement { relation: st.relation.clone(), args };
        if !target.statements.contains(&projected) && !out.contains(&projected) {
            out.push(projected);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water_flow() -> System {
        // cause(flow(water, pipe), flow(water, beaker)) style nesting, kept small.
        let mut s = System::new(3); // 0 water, 1 pipe, 2 beaker
        let greater = s.push("greater", vec![Term::Entity(1), Term::Entity(0)]);
        let flow = s.push("flow", vec![Term::Entity(0), Term::Entity(2)]);
        let cause = s.push("cause", vec![Term::Statement(greater), Term::Statement(flow)]);
        let _ = cause;
        s
    }

    fn heat_flow() -> System {
        let mut s = System::new(3); // 0 heat, 1 bar, 2 coffee
        let greater = s.push("greater", vec![Term::Entity(1), Term::Entity(0)]);
        let flow = s.push("flow", vec![Term::Entity(0), Term::Entity(2)]);
        let cause = s.push("cause", vec![Term::Statement(greater), Term::Statement(flow)]);
        let _ = cause;
        s
    }

    #[test]
    fn identical_systems_align_completely() {
        let s = water_flow();
        let m = map(&s, &s, 1).remove(0);
        assert_eq!(m.score.matched, 3);
        assert_eq!(m.score.components, 1);
        assert_eq!(m.entities.len(), 3);
    }

    #[test]
    fn analogy_prefers_the_deep_common_structure() {
        let base = water_flow();
        let target = heat_flow();
        let m = map(&base, &target, 1).remove(0);
        // cause(greater(bar, heat), flow(heat, coffee)) is matched as one connected piece.
        assert_eq!(m.score.matched, 3);
        assert_eq!(m.score.components, 1);
        // cause(greater, flow) contributes 1 + 1 + 1, the two children contribute 1 each.\n        assert_eq!(m.score.systematicity, 5);
        assert_eq!(m.target_of(0), Some(0)); // water -> heat
        assert_eq!(m.target_of(1), Some(1)); // pipe -> bar
    }

    #[test]
    fn correspondences_are_one_to_one() {
        // Two base entities compete for one target entity; only one may be chosen.
        let mut base = System::new(2);
        let r1 = base.push("r", vec![Term::Entity(0)]);
        let r2 = base.push("r", vec![Term::Entity(1)]);
        let mut target = System::new(1);
        let t1 = target.push("r", vec![Term::Entity(0)]);
        let m = map(&base, &target, 1).remove(0);
        assert_eq!(m.entities.len(), 1);
        let mapped = m.statements.iter().map(|(b, _)| *b).collect::<Vec<_>>();
        assert!(
            mapped == vec![r1] || mapped == vec![r2],
            "expected one of the two competing statements, got {:?} against {:?}",
            mapped,
            t1
        );
    }

    #[test]
    fn unmatched_structure_is_projected_as_an_inference() {
        // Base has one extra statement whose entities are all mapped.
        let mut base = System::new(2);
        let shared = base.push("same", vec![Term::Entity(0), Term::Entity(1)]);
        let extra = base.push("extra", vec![Term::Entity(0)]);
        let mut target = System::new(2);
        let t_shared = target.push("same", vec![Term::Entity(0), Term::Entity(1)]);
        let m = map(&base, &target, 1).remove(0);
        assert!(m.statements.contains(&(shared, t_shared)));
        let inferences = candidate_inferences(&base, &target, &m);
        assert!(inferences.iter().any(|s| s.relation == "extra"), "{:?}", inferences);
        let _ = extra;
    }

    #[test]
    fn mapping_is_deterministic() {
        let base = water_flow();
        let target = heat_flow();
        assert_eq!(map(&base, &target, 3), map(&base, &target, 3));
    }

    #[test]
    fn an_unrelated_system_yields_no_alignment() {
        let mut base = System::new(1);
        base.push("p", vec![Term::Entity(0)]);
        let mut target = System::new(1);
        target.push("q", vec![Term::Entity(0)]);
        assert!(map(&base, &target, 1).is_empty());
    }

    #[test]
    fn connected_structure_outweighs_isolated_matches() {
        // Both alignments match two statements; only the nested one is connected.
        let mut base = System::new(2);
        let b_inner = base.push("inner", vec![Term::Entity(0), Term::Entity(1)]);
        base.push("outer", vec![Term::Statement(b_inner)]);
        let mut target = System::new(2);
        let t_inner = target.push("inner", vec![Term::Entity(0), Term::Entity(1)]);
        target.push("outer", vec![Term::Statement(t_inner)]);
        let nested = map(&base, &target, 1).remove(0);
        assert_eq!(nested.score.components, 1);
        assert_eq!(nested.score.systematicity, 3); // outer 1 + inner 1, then +1 for the nesting

        let mut flat = System::new(2);
        flat.push("inner", vec![Term::Entity(0)]);
        flat.push("other", vec![Term::Entity(1)]);
        let mut flat_target = System::new(2);
        flat_target.push("inner", vec![Term::Entity(0)]);
        flat_target.push("other", vec![Term::Entity(1)]);
        let separate = map(&flat, &flat_target, 1).remove(0);
        assert_eq!(separate.score.components, 2);
        assert_eq!(separate.score.systematicity, 2);
        assert!(nested.score.systematicity > separate.score.systematicity);
    }

    #[test]
    fn unmapped_entities_become_skolems_in_candidate_inferences() {
        let mut base = System::new(2);
        base.push("matchme", vec![Term::Entity(0)]);
        base.push("infer", vec![Term::Entity(1)]);
        let mut target = System::new(1);
        target.push("matchme", vec![Term::Entity(0)]);
        let m = Mapping {
            statements: vec![(0, 0)],
            entities: vec![(0, 0)],
            score: Score { matched: 1, systematicity: 1, components: 1 },
        };
        let cis = candidate_inferences_with_skolems(&base, &target, &m);
        assert_eq!(cis.len(), 1, "one unmatched statement is projected");
        assert_eq!(cis[0].statement.relation, "infer");
        assert_eq!(cis[0].statement.args, vec![Term::Entity(1)], "1 is the first skolem id");
        assert_eq!(cis[0].skolems, vec![(1, 1)]);
        assert!(
            candidate_inferences(&base, &target, &m).is_empty(),
            "the old path drops the statement rather than marking it"
        );
    }

    #[test]
    fn symmetric_relations_get_a_twin_match() {
        let mut base = System::new(2);
        base.push_symmetric("parallel", vec![Term::Entity(0), Term::Entity(1)]);
        let mut target = System::new(2);
        target.push_symmetric("parallel", vec![Term::Entity(0), Term::Entity(1)]);
        let pairs: Vec<Vec<(EntityId, EntityId)>> =
            local_matches(&base, &target).iter().map(|m| m.entities.clone()).collect();
        assert!(pairs.iter().any(|p| p.contains(&(0, 0))), "normal alignment missing");
        assert!(pairs.iter().any(|p| p.contains(&(0, 1))), "twin alignment missing");
    }

    #[test]
    fn an_asymmetric_relation_has_no_twin() {
        let mut base = System::new(2);
        base.push("above", vec![Term::Entity(0), Term::Entity(1)]);
        let mut target = System::new(2);
        target.push("above", vec![Term::Entity(0), Term::Entity(1)]);
        let pairs: Vec<Vec<(EntityId, EntityId)>> =
            local_matches(&base, &target).iter().map(|m| m.entities.clone()).collect();
        assert!(pairs.iter().any(|p| p.contains(&(0, 0))));
        assert!(!pairs.iter().any(|p| p.contains(&(0, 1))));
    }

    #[test]
    fn commutative_relations_try_every_permutation() {
        // between(a, b, c) is unordered. The target states the same fact with its arguments in a
        // different order (c, a, b), so the identity alignment alone would pair the wrong
        // entities and miss it.
        let mut base = System::new(3); // 0 a, 1 b, 2 c
        let b_between = base.push_commutative(
            "between",
            vec![Term::Entity(0), Term::Entity(1), Term::Entity(2)],
        );
        let mut target = System::new(3); // target order c, a, b
        let t_between = target.push_commutative(
            "between",
            vec![Term::Entity(2), Term::Entity(0), Term::Entity(1)],
        );

        let matches = local_matches(&base, &target);
        let for_pair: Vec<&Match> = matches
            .iter()
            .filter(|m| m.base == b_between && m.target == t_between)
            .collect();
        assert_eq!(
            for_pair.len(),
            6,
            "arity 3 has 3! distinct permutations, got {:?}",
            for_pair
        );

        // Permutation (1, 2, 0) of the target's arguments is the one that yields a->a, b->b, c->c.
        let identity = for_pair
            .iter()
            .find(|m| {
                m.entities.contains(&(0, 0))
                    && m.entities.contains(&(1, 1))
                    && m.entities.contains(&(2, 2))
            })
            .expect("the identity correspondence must be reachable through some permutation");
        assert_eq!(identity.entities.len(), 3);

        // Each permutation is a bijection, and no two permutations collapse to the same match.
        for m in &for_pair {
            let base_entities: BTreeSet<EntityId> = m.entities.iter().map(|(b, _)| *b).collect();
            let target_entities: BTreeSet<EntityId> = m.entities.iter().map(|(_, t)| *t).collect();
            assert_eq!(
                (base_entities.len(), target_entities.len()),
                (3, 3),
                "a permutation must stay one-to-one: {:?}",
                m.entities
            );
        }
        let distinct: BTreeSet<Vec<(EntityId, EntityId)>> =
            for_pair.iter().map(|m| m.entities.clone()).collect();
        assert_eq!(distinct.len(), 6, "permutations must deduplicate: {:?}", distinct);
    }

    #[test]
    fn commutative_relation_above_the_arity_cap_falls_back_to_the_reversal() {
        // Past MAX_COMMUTATIVE_ARITY only the identity and the reversal are tried, so a pair of
        // six-argument statements yields two matches rather than 6! = 720.
        let mut base = System::new(6);
        let b_rel = base.push_commutative("six", (0..6).map(Term::Entity).collect());
        let mut target = System::new(6);
        let t_rel = target.push_commutative("six", (0..6).map(Term::Entity).collect());
        let count = local_matches(&base, &target)
            .iter()
            .filter(|m| m.base == b_rel && m.target == t_rel)
            .count();
        assert_eq!(count, 2, "identity and reversal only above the cap");
    }

    #[test]
    fn entity_evidence_matches_the_sme4_arithmetic() {
        // Nine matched expressions over entity 0. SME4's corpus scores the entity match
        // 9 x 8 x 5.0e-4 = 0.036; this pins the constants to the original rather than to taste.
        let mut base = System::new(1);
        let mut target = System::new(1);
        for i in 0..9 {
            let r = format!("p{i}");
            base.push(&r, vec![Term::Entity(0)]);
            target.push(&r, vec![Term::Entity(0)]);
        }
        let m = map(&base, &target, 1).pop().expect("a mapping");
        let e = evidence(&base, &target, &m);
        assert!((e.entities[&0] - 0.036).abs() < 1e-9, "entity evidence {}", e.entities[&0]);
        assert!(e.total > e.expressions, "the entity carries the trickle-down");
    }

    #[test]
    fn nested_structure_pays_eight_times_per_level() {
        // A statement that contains a matched child is worth 5.0e-4 + 8 x 5.0e-4 = 0.0045.
        let mut base = System::new(2);
        let inner = base.push("inner", vec![Term::Entity(0), Term::Entity(1)]);
        base.push("outer", vec![Term::Statement(inner)]);
        let mut target = System::new(2);
        let t_inner = target.push("inner", vec![Term::Entity(0), Term::Entity(1)]);
        target.push("outer", vec![Term::Statement(t_inner)]);
        let m = map(&base, &target, 1).remove(0);
        let e = evidence(&base, &target, &m);
        assert!((e.expressions - (0.0045 + 0.0005)).abs() < 1e-9, "expressions {}", e.expressions);
    }
}
