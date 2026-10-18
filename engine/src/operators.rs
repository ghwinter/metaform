//! Multi-operator typing: the executable counterpart of paper §4 (`Operators as typed transitions`).
//!
//! The paper splits 'what to do next' into five classes of steps and compares them with a single measure:
//!
//! | Paper | Here |
//! |---|---|
//! | State `s=(H,V,E,B)` | [`State`] |
//! | Live set `L(s)` / potential `Φ(s)` | [`State::live`] / [`State::phi`] |
//! | Operator `(rd, wr, warrant, cost, falsifier)` | [`Op`] + [`Op::class`] + [`Op::cost`] + [`Op::warrant`] |
//! | T8 generation is monotone | generation only adds to `H`, so `Φ` never drops |
//! | T9 closure is idempotent | closing twice = closing once |
//! | T10 evidence is the only strict reducer | a question that splits the live set strictly lowers `Φ` |
//! | T11 recombination cannot break a collision | Boolean combinations inside the vocabulary are rejected outright by `warrant` |
//! | Corollary (alternation) | at `Φ=0` only a new atom can help; at `Φ≥2` only evidence can lower it |
//! | The whole gain of control = the redundant spend it saves | `skip_redundant` in [`run`] and [`demo`] |
//!
//! Search and ranking are deliberately not implemented here: this module only answers 'can this step change the measure'.
//!
//! ## What is not implemented (do not over-read the correspondence)
//!
//! - **The warrant for generation is not implemented**: the paper says the warrant for generation is 'cover + simplicity' (T7 and MDL),
//!   but here only the direction of T8 is checked (adding candidates never lowers `Φ`), not whether a candidate is good.
//! - **The warrant for representation checks two things**: the new atom is not a Boolean combination of the current vocabulary (the negative side of T11),
//!   and every candidate is definable under `V ∪ {new atom}` — otherwise the step records the unexpressed as expressed.
//!   Coverage (T7) is only a scheduling score [`Op::coverage`], never a rejection condition; the fidelity condition (rebuilding
//!   the old evidence after a representation change) is still unimplemented.
//! - **Inherited labels need verification**: ordinary evidence records the object in `asked`; [`Op::Probe`] can only ask about members
//!   'never asked directly' inside a multi-member equivalence class, to test the inherited labels. `State::unprobed` lists these members.
//! - **Relations enter the vocabulary only through projections**: `State` stores binary relations (`rels[r][x]` is the successor set),
//!   but `signature` / `definable` are still unary; a relation must first pass through [`State::image`] or
//!   [`State::preimage`] to become a unary predicate before `Represent` can propose it as an atom.
//! - **The cost model of control is not implemented**: `is_discriminating` only says 'would it split the live set',
//!   not 'is it worth the cost'; the experiment's cost lives with the caller.
//! - **Control is not a step**: it is the `skip_redundant` mode of [`run`]; the paper's `B` is accounted for by `run`.

use crate::{bit, Mask};

/// Four classes of executable operators + control. The type is the paper's `(rd, wr)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Induction / abduction / analogy: writes `H` only, growing by inclusion.
    Generation,
    /// Deduction / consistency checking: writes `H` only, filtering to candidates consistent with the evidence.
    Closure,
    /// Vocabulary expansion / anti-unification / symmetry quotient: writes `V` only.
    Representation,
    /// Counterexample search / active learning / experiment: writes `E` only.
    Evidence,
    /// Metareasoning: writes `B` only, choosing among the other four classes.
    Control,
}

impl Class {
    pub fn writes(self) -> &'static str {
        match self {
            Class::Generation | Class::Closure => "H",
            Class::Representation => "V",
            Class::Evidence => "E",
            Class::Control => "B",
        }
    }
}

/// Direction: operators that write `H` use it to tell 'add (↑)' from 'filter (↓)'.
/// The paper says 'the type fixes the class'; `wr` alone has only four values, so direction must be added to get five.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    None,
}

/// State: candidate family `H`, vocabulary `V`, evidence `E`, budget `B`.
#[derive(Clone, Debug)]
pub struct State {
    pub n: usize,
    pub vocab: Vec<Mask>,
    /// Binary relation: `rels[r][x]` is the successor set of `x` under relation `r`.
    /// Relations do not enter signatures directly; they enter the vocabulary only after being projected to a unary predicate via [`State::image`] / [`State::preimage`].
    pub rels: Vec<Vec<Mask>>,
    pub hyps: Vec<Mask>,
    pub pos: Mask,
    pub neg: Mask,
    /// Objects that were asked directly. Class labels 'inherit' to members of the same class, so members never asked directly are unverified.
    pub asked: Mask,
    pub budget: u32,
}

impl State {
    pub fn new(n: usize) -> Self {
        assert!(n <= 64, "Mask is u64, so at most 64 objects");
        Self {
            n,
            vocab: Vec::new(),
            rels: Vec::new(),
            hyps: Vec::new(),
            pos: 0,
            neg: 0,
            asked: 0,
            budget: u32::MAX,
        }
    }

    /// Unverified members: objects of a multi-member equivalence class that were never asked directly.
    /// Their labels are 'inherited' from class members and may be wrong.
    pub fn unprobed(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for c in self.classes() {
            if c.count_ones() < 2 {
                continue;
            }
            for x in 0..self.n {
                if c & bit(x) != 0 && self.asked & bit(x) == 0 {
                    out.push(x);
                }
            }
        }
        out
    }

    /// Objects reachable in one step from set `s` under relation `r`: `{y | ∃x∈s, r(x,y)}`.
    pub fn image(&self, r: usize, s: Mask) -> Mask {
        let mut out = 0u64;
        for x in 0..self.n {
            if s & bit(x) != 0 {
                out |= self.rels[r][x];
            }
        }
        out
    }

    /// Objects that reach `s` in one step under relation `r`: `{x | ∃y∈s, r(x,y)}`.
    /// This is the shortest path for projecting a binary relation to a unary predicate (a lift in ILP).
    pub fn preimage(&self, r: usize, s: Mask) -> Mask {
        let mut out = 0u64;
        for x in 0..self.n {
            if self.rels[r][x] & s != 0 {
                out |= bit(x);
            }
        }
        out
    }

    pub fn full(n: usize) -> Mask {
        if n >= 64 {
            u64::MAX
        } else {
            (1u64 << n) - 1
        }
    }

    /// Candidate `h` is consistent with the evidence: it contains every positive and no negative.
    pub fn consistent(&self, h: Mask) -> bool {
        self.pos & !h == 0 && h & self.neg == 0
    }

    /// Live set `L(s)`.
    pub fn live(&self) -> Vec<Mask> {
        self.hyps.iter().copied().filter(|h| self.consistent(*h)).collect()
    }

    /// Potential `Φ(s)`.
    pub fn phi(&self) -> usize {
        self.live().len()
    }

    /// Signature of `x` under `V`.
    pub fn signature(&self, x: usize) -> Vec<bool> {
        self.vocab.iter().map(|p| p & bit(x) != 0).collect()
    }

    /// Group by signature (equivalence classes).
    pub fn classes(&self) -> Vec<Mask> {
        self.classes_with(0)
    }

    /// Equivalence classes after adding `extra` to the vocabulary. With `extra = 0` this is the partition of the current vocabulary.
    pub fn classes_with(&self, extra: Mask) -> Vec<Mask> {
        let mut keys: Vec<Vec<bool>> = Vec::new();
        let mut masks: Vec<Mask> = Vec::new();
        for x in 0..self.n {
            let mut s = self.signature(x);
            s.push(extra & bit(x) != 0);
            match keys.iter().position(|k| *k == s) {
                Some(i) => masks[i] |= bit(x),
                None => {
                    keys.push(s);
                    masks.push(bit(x));
                }
            }
        }
        masks
    }

    /// Minimum number of literals needed to describe `h` as a conjunction over the current vocabulary (each predicate may be positive or negative),
    /// enumerating at most 3 literals; returns `None` beyond that. This is an engineering approximation of MDL: model terms only, no residual.
    pub fn mdl(&self, h: Mask) -> Option<usize> {
        let m = self.vocab.len();
        if m > 12 {
            return None;
        }
        let full = State::full(self.n);
        for k in 1..=3usize.min(m) {
            for combo in combos(m, k) {
                for signs in 0..(1u32 << k) {
                    let mut mask = full;
                    for (i, &idx) in combo.iter().enumerate() {
                        let lit = if signs >> i & 1 == 0 {
                            self.vocab[idx]
                        } else {
                            full & !self.vocab[idx]
                        };
                        mask &= lit;
                    }
                    if mask == h {
                        return Some(k);
                    }
                }
            }
        }
        None
    }

    /// Pairs of objects that are indistinguishable under the current vocabulary but fall in the positives and the negatives respectively (the paper's colliding pairs).
    pub fn colliding_pairs(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for x in 0..self.n {
            if self.pos & bit(x) == 0 {
                continue;
            }
            for y in 0..self.n {
                if self.neg & bit(y) != 0 && self.signature(x) == self.signature(y) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    /// Whether `h` is `V`-definable: constant on every equivalence class (paper T1).
    pub fn definable(&self, h: Mask) -> bool {
        self.definable_with(0, h)
    }

    /// Whether `h` is definable under `V ∪ {extra}`. The warrant of `Represent` uses it:
    /// the new atom must really make these candidates expressible, otherwise the step only records the unexpressed as expressed.
    pub fn definable_with(&self, extra: Mask, h: Mask) -> bool {
        for c in self.classes_with(extra) {
            let inside = h & c;
            if inside != 0 && inside != c {
                return false;
            }
        }
        true
    }

    /// All `V`-definable sets under the current vocabulary (unions of equivalence classes).
    /// The demo domain is kept small (at most 20 classes); otherwise returns empty.
    pub fn enumerate_definable(&self) -> Vec<Mask> {
        let cs = self.classes();
        if cs.len() > 20 {
            return Vec::new();
        }
        (0u32..(1u32 << cs.len()))
            .map(|m| {
                let mut out = 0u64;
                for (i, c) in cs.iter().enumerate() {
                    if m & (1 << i) != 0 {
                        out |= c;
                    }
                }
                out
            })
            .collect()
    }
}

/// All combinations of `k` out of `m` indices (`m` is tiny, enough for the demo domain).
fn combos(m: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    fn go(start: usize, m: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..m {
            cur.push(i);
            go(i + 1, m, k, cur, out);
            cur.pop();
        }
    }
    go(0, m, k, &mut cur, &mut out);
    out
}

/// Pick the candidate atom with the largest coverage from the pool (the executable version of T7).
/// If a single atom alone makes the target definable, it must separate all colliding pairs, so the choice is not a heuristic in that case.
/// Boolean recombinations (`V`-definable candidates) do not take part — those are what T11 says cannot break a collision.
pub fn best_cover(s: &State, pool: &[Mask]) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, &a) in pool.iter().enumerate() {
        if s.definable(a) {
            continue;
        }
        let cov = s
            .colliding_pairs()
            .iter()
            .filter(|(x, y)| ((a >> *x) & 1) != ((a >> *y) & 1))
            .count();
        match best {
            None => best = Some((i, cov)),
            Some((_, bc)) => {
                if cov > bc {
                    best = Some((i, cov));
                }
            }
        }
    }
    best.map(|(i, _)| i)
}

/// How many atoms each of three orders needs when two atoms are required. `k` is the number of 'directions' (that many atoms are needed).
pub struct ComposeStats {
    pub trials: u64,
    pub fixed: f64,
    pub random: f64,
    pub greedy: f64,
    pub fixed_ok: f64,
    pub random_ok: f64,
    pub greedy_ok: f64,
}

/// When several atoms are needed: greedy cover vs random order vs fixed order (the fixed order puts whole-class atoms first; they provide no new split).
pub fn compose_bench(trials: u64, k: usize, budget: usize) -> ComposeStats {
    assert!(k >= 1 && k <= 4, "two-bit class encoding requires k <= 4");
    let mut seed = 0x1234_5678u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let build = || {
        let n = 2 * k;
        let mut s = State::new(n);
        let block = |pred: fn(usize) -> bool| -> Mask {
            (0..k)
                .filter(|i| pred(*i))
                .map(|i| bit(2 * i) | bit(2 * i + 1))
                .fold(0, |a, b| a | b)
        };
        s.vocab = vec![block(|i| i % 2 == 1), block(|i| i / 2 % 2 == 1)];
        s.pos = (0..k).map(|i| bit(2 * i)).fold(0, |a, b| a | b);
        s.neg = (0..k).map(|i| bit(2 * i + 1)).fold(0, |a, b| a | b);
        s.hyps = s.enumerate_definable();
        s
    };
    let good: Vec<Mask> = (0..k).map(|i| bit(2 * i)).collect();
    let bad: Vec<Mask> = (0..k).map(|i| bit(2 * i) | bit(2 * i + 1)).collect();
    let fixed_order: Vec<Mask> = bad.iter().chain(good.iter()).copied().collect();

    let (mut f, mut r, mut g, mut fo, mut ro, mut go) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for _ in 0..trials {
        let s0 = build();
        let target = s0.pos;

        let (fs, fok) = steps_to_define(&s0, &fixed_order, target, budget);
        f += fs as u64;
        fo += u64::from(fok);

        let mut order = fixed_order.clone();
        for i in (1..order.len()).rev() {
            let j = (rnd() as usize) % (i + 1);
            order.swap(i, j);
        }
        let (rs, rok) = steps_to_define(&s0, &order, target, budget);
        r += rs as u64;
        ro += u64::from(rok);

        let greedy = greedy_order(&s0, &[good.clone(), bad.clone()].concat());
        let (gs, gok) = steps_to_define(&s0, &greedy, target, budget);
        g += gs as u64;
        go += u64::from(gok);
    }
    let d = trials.max(1) as f64;
    ComposeStats {
        trials,
        fixed: f as f64 / d,
        random: r as f64 / d,
        greedy: g as f64 / d,
        fixed_ok: fo as f64 / d,
        random_ok: ro as f64 / d,
        greedy_ok: go as f64 / d,
    }
}

/// Order the atoms greedily by coverage (skipping `V`-definable recombinations).
fn greedy_order(s0: &State, pool: &[Mask]) -> Vec<Mask> {
    let mut s = s0.clone();
    let mut remaining: Vec<Mask> = pool.to_vec();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let Some(i) = best_cover(&s, &remaining) else { break };
        let a = remaining.remove(i);
        s.vocab.push(a);
        order.push(a);
    }
    order
}

/// Add atoms one by one in the given order until the target is definable; returns (steps, success). Failure is recorded as `budget + 1`.
fn steps_to_define(s0: &State, order: &[Mask], target: Mask, budget: usize) -> (usize, bool) {
    let mut s = s0.clone();
    if s.definable(target) {
        return (0, true);
    }
    for (i, &a) in order.iter().enumerate() {
        if i >= budget {
            break;
        }
        s.vocab.push(a);
        if s.definable(target) {
            return (i + 1, true);
        }
    }
    (budget + 1, false)
}

/// Same-budget comparison: for the same batch of queries, how many rounds fixed order / random order / stepwise selection each need.
pub fn selection_bench(trials: u64) -> (u64, f64, f64, f64) {
    let mut seed = 0x9e37_79b9u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let (mut fixed, mut random, mut selected, mut done) = (0u64, 0u64, 0u64, 0u64);
    for _ in 0..trials {
        let n = 6;
        let mut s = State::new(n);
        s.pos = bit(0);
        s.hyps = (0..8)
            .map(|_| {
                let mut h = bit(0);
                for x in 1..n {
                    if rnd() % 2 == 0 {
                        h |= bit(x);
                    }
                }
                h
            })
            .collect();
        s.hyps.sort_unstable();
        s.hyps.dedup();
        if s.phi() < 2 {
            continue;
        }
        let truth = s.hyps[(rnd() as usize) % s.hyps.len()];
        let ops: Vec<Op> = (1..n)
            .map(|x| Op::Evidence { x, member: truth & bit(x) != 0, cost: 1 })
            .collect();

        let mut a = s.clone();
        fixed += run_order(&mut a, &ops).cost as u64;

        let mut b = s.clone();
        let mut order = ops.clone();
        for i in (1..order.len()).rev() {
            let j = (rnd() as usize) % (i + 1);
            order.swap(i, j);
        }
        random += run_order(&mut b, &order).cost as u64;

        let mut c = s.clone();
        selected += run_selected(&mut c, &ops).expect("selected run").cost as u64;
        done += 1;
    }
    let d = done.max(1) as f64;
    (done, fixed as f64 / d, random as f64 / d, selected as f64 / d)
}

/// Run in the given order until a decision (`Φ = 1`) (the fixed/random orders used for comparison).
fn run_order(s: &mut State, ops: &[Op]) -> Outcome {
    let mut cost = 0u32;
    let mut ran = 0usize;
    for op in ops {
        if s.phi() == 1 {
            break;
        }
        if op.is_redundant(s) {
            continue;
        }
        if op.apply(s).is_err() {
            break;
        }
        cost += op.cost();
        ran += 1;
    }
    Outcome { phi: s.phi(), cost, ran, skipped: 0, skipped_cost: 0 }
}

/// The step was hit by a falsifier, or its precondition does not hold.
///
/// `Recombination` is the direct implementation of paper T11; the other three are engineering-level preconditions (contradicts the evidence /
/// contradicts an already confirmed answer / not enough budget), on which the paper makes no claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falsified {
    /// The predicate in a representation step is a Boolean combination of the current vocabulary: T11, recombination cannot break a collision.
    Recombination,
    /// The new candidate contradicts the evidence.
    Inconsistent,
    /// The candidate is still not definable under `V ∪ {new atom}`: the step recorded the unexpressed as expressed.
    NotExpressible,
    /// The probe target was already asked directly.
    AlreadyAsked,
    /// The probe target's equivalence class has only one member: there is no 'inheritance' to verify.
    NotInherited,
    /// The evidence contradicts an already confirmed answer.
    Contradiction,
    /// Not enough budget.
    OutOfBudget,
}

/// Bit flags of the state components, corresponding to `rd` / `wr` in paper Def 5.
pub mod comp {
    pub const H: u8 = 1;
    pub const V: u8 = 2;
    pub const E: u8 = 4;
    pub const B: u8 = 8;
}

/// The five classes of steps. `cost` uses one unit (query rounds / proposal overhead); otherwise costs are neither additive nor schedulable.
#[derive(Clone, Debug)]
pub enum Op {
    /// Generation: merge candidates into `H`. The warrant is coverage, the falsifier is inconsistency.
    Generate { cands: Vec<Mask>, cost: u32 },
    /// Closure: filter `H` to agree with `E`. The warrant is derivation; there is no falsifier (everything removed can be checked).
    Close { cost: u32 },
    /// Representation: add a new atom `pred` and let candidates using it enter `H`.
    Represent { pred: Mask, cands: Vec<Mask>, cost: u32 },
    /// Evidence: ask the oracle about one object. The warrant is the oracle's answer.
    Evidence { x: usize, member: bool, cost: u32 },
    /// Probe: ask a member whose 'label was inherited from its class', to check whether the inheritance holds.
    /// It differs from `Evidence` only in the warrant: the object must never have been asked directly, and its class must have more than one member.
    Probe { x: usize, member: bool, cost: u32 },
}

impl Op {
    /// Direction. The two classes that write `H` are told apart by it.
    pub fn dir(&self) -> Dir {
        match self {
            Op::Generate { .. } => Dir::Up,
            Op::Close { .. } => Dir::Down,
            Op::Represent { .. } | Op::Evidence { .. } | Op::Probe { .. } => Dir::None,
        }
    }

    /// Which components are read (paper Def 5's `rd`).
    pub fn reads(&self) -> u8 {
        match self {
            Op::Generate { .. } | Op::Close { .. } | Op::Evidence { .. } | Op::Probe { .. } => {
                comp::H | comp::E
            }
            Op::Represent { .. } => comp::V | comp::H | comp::E,
        }
    }

    /// Which components are written (paper Def 5's `wr`). `Represent` writes `V` and therefore `H` (a new atom lets new candidates in).
    pub fn writes(&self) -> u8 {
        match self {
            Op::Generate { .. } | Op::Close { .. } => comp::H,
            Op::Represent { .. } => comp::V | comp::H,
            Op::Evidence { .. } | Op::Probe { .. } => comp::E,
        }
    }

    pub fn class(&self) -> Class {
        match self {
            Op::Generate { .. } => Class::Generation,
            Op::Close { .. } => Class::Closure,
            Op::Represent { .. } => Class::Representation,
            Op::Evidence { .. } | Op::Probe { .. } => Class::Evidence,
        }
    }

    pub fn cost(&self) -> u32 {
        match self {
            Op::Generate { cost, .. }
            | Op::Close { cost }
            | Op::Represent { cost, .. }
            | Op::Evidence { cost, .. }
            | Op::Probe { cost, .. } => *cost,
        }
    }

    /// Warranty: why this step may be accepted. T11 is implemented here.
    pub fn warrant(&self, s: &State) -> Result<(), Falsified> {
        match self {
            Op::Generate { .. } => {
                // generation needs no warrant: if an added candidate contradicts the evidence, closure will remove it
                // (T8 only says Φ does not drop, not whether a candidate is good — goodness is measured by Φ and cost).
                Ok(())
            }
            Op::Close { .. } => Ok(()),
            Op::Represent { pred, cands, .. } => {
                if s.definable(*pred) {
                    return Err(Falsified::Recombination);
                }
                // every candidate must be definable under the new vocabulary V ∪ {pred}, otherwise it is only 'recorded' as expressed.
                if cands.iter().any(|c| !s.definable_with(*pred, *c)) {
                    return Err(Falsified::NotExpressible);
                }
                if cands.iter().any(|c| !s.consistent(*c)) {
                    return Err(Falsified::Inconsistent);
                }
                Ok(())
            }
            Op::Evidence { x, member, .. } => {
                let b = bit(*x);
                if (*member && s.neg & b != 0) || (!*member && s.pos & b != 0) {
                    return Err(Falsified::Contradiction);
                }
                Ok(())
            }
            Op::Probe { x, member, .. } => {
                let b = bit(*x);
                if s.asked & b != 0 {
                    return Err(Falsified::AlreadyAsked);
                }
                let cls = s.classes().into_iter().find(|c| c & b != 0).unwrap_or(0);
                if cls.count_ones() < 2 {
                    return Err(Falsified::NotInherited);
                }
                if (*member && s.neg & b != 0) || (!*member && s.pos & b != 0) {
                    return Err(Falsified::Contradiction);
                }
                Ok(())
            }
        }
    }

    /// How much this step is expected to remove: generation/closure count candidates, representation looks at coverage, evidence at whether it splits.
    pub fn gain(&self, s: &State) -> usize {
        match self {
            Op::Generate { cands, .. } => cands
                .iter()
                .filter(|c| !s.hyps.contains(*c) && s.consistent(**c))
                .count(),
            Op::Close { .. } => s.hyps.iter().filter(|h| !s.consistent(**h)).count(),
            Op::Represent { .. } => self.coverage(s),
            Op::Evidence { .. } | Op::Probe { .. } => usize::from(self.is_discriminating(s)),
        }
    }

    /// Redundancy: nothing new can be written (the state is unchanged bit for bit). The control class is always redundant.
    ///
    /// Note that 'redundant' and 'non-discriminating' are not the same thing: an evidence step that answers the same on every live candidate
    /// **does not lower Φ**, yet it still writes `E` (the candidates after the constraint), so it is not called redundant. See [`Op::is_discriminating`].
    pub fn is_redundant(&self, s: &State) -> bool {
        match self {
            Op::Generate { cands, .. } => cands
                .iter()
                .all(|c| s.hyps.contains(c) || !s.consistent(*c)),
            Op::Close { .. } => s.hyps.iter().all(|h| s.consistent(*h)),
            Op::Represent { pred, cands, .. } => {
                s.vocab.contains(pred) && cands.iter().all(|c| s.hyps.contains(c))
            }
            Op::Evidence { x, .. } => {
                let b = bit(*x);
                s.pos & b != 0 || s.neg & b != 0 // the answer is already known; asking again writes nothing
            }
            Op::Probe { x, .. } => s.asked & bit(*x) != 0 || s.pos & bit(*x) != 0 || s.neg & bit(*x) != 0,
        }
    }

    /// Coverage score: how many currently colliding positive/negative pairs this step separates.
    /// It is only a scheduling signal, not a warrant — an atom that is only useful later has coverage 0 and is still legal.
    pub fn coverage(&self, s: &State) -> usize {
        match self {
            Op::Generate { .. } | Op::Close { .. } | Op::Evidence { .. } | Op::Probe { .. } => 0,
            Op::Represent { pred, .. } => s
                .colliding_pairs()
                .iter()
                .filter(|(x, y)| ((pred >> *x) & 1) != ((pred >> *y) & 1))
                .count(),
        }
    }

    /// Discriminating: whether this step really splits the live set (and therefore strictly lowers `Φ`).
    /// Non-discriminating evidence does not lower `Φ`, it only adds constraints; this is the executable version of the paper's 'every question is worth the same bit'.
    pub fn is_discriminating(&self, s: &State) -> bool {
        match self {
            Op::Generate { cands, .. } => {
                cands.iter().any(|c| !s.hyps.contains(c) && s.consistent(*c))
            }
            Op::Close { .. } => s.hyps.iter().any(|h| !s.consistent(*h)),
            Op::Represent { pred, .. } => !s.definable(*pred),
            Op::Evidence { x, member, .. } | Op::Probe { x, member, .. } => {
                let b = bit(*x);
                s.live().iter().any(|h| (h & b != 0) != *member)
            }
        }
    }

    /// Execute one step. Pass the warrant first, then write the state.
    pub fn apply(&self, s: &mut State) -> Result<(), Falsified> {
        self.warrant(s)?;
        match self {
            Op::Generate { cands, .. } => {
                for c in cands {
                    if !s.hyps.contains(c) {
                        s.hyps.push(*c);
                    }
                }
                Ok(())
            }
            Op::Close { .. } => {
                let live = s.live();
                s.hyps = live;
                Ok(())
            }
            Op::Represent { pred, cands, .. } => {
                s.vocab.push(*pred);
                for c in cands {
                    if !s.hyps.contains(c) {
                        s.hyps.push(*c);
                    }
                }
                Ok(())
            }
            Op::Evidence { x, member, .. } | Op::Probe { x, member, .. } => {
                if *member {
                    s.pos |= bit(*x);
                } else {
                    s.neg |= bit(*x);
                }
                s.asked |= bit(*x);
                Ok(())
            }
        }
    }
}

/// The two relations of paper Def 5: whether `o₂` is informed by `o₁`, and whether `o₂` is redundant after `o₁`.
pub fn informed_by(prev: &Op, next: &Op) -> bool {
    next.reads() & (prev.reads() | prev.writes()) != 0
}

/// `next` writes no new component after `prev` (the redundancy of paper Def 5).
pub fn redundant_after(prev: &Op, next: &Op) -> bool {
    next.writes() & !prev.writes() == 0
}

/// Pick one of the candidate steps by `gain / cost` (ties go to the earlier one).
/// Steps that fail the warrant or are redundant are skipped outright — that is the entire job of the control class.
pub fn select(s: &State, ops: &[Op]) -> Option<usize> {
    let mut best: Option<(usize, u64, u64)> = None;
    for (i, op) in ops.iter().enumerate() {
        if op.is_redundant(s) || op.warrant(s).is_err() {
            continue;
        }
        let gain = op.gain(s) as u64;
        let cost = op.cost().max(1) as u64;
        match best {
            None => best = Some((i, gain, cost)),
            Some((_, bg, bc)) => {
                if gain * bc > bg * cost {
                    best = Some((i, gain, cost));
                }
            }
        }
    }
    best.map(|(i, _, _)| i)
}

/// Reselect the currently best step every time. The stopping condition is **decided and with no inherited labels**:
/// `Φ = 1` is not enough, `unprobed` must be empty as well; otherwise schedule a probe first.
pub fn run_selected(s: &mut State, ops: &[Op]) -> Result<Outcome, Falsified> {
    let mut remaining: Vec<Op> = ops.to_vec();
    let mut cost = 0u32;
    let mut ran = 0usize;
    loop {
        if s.phi() == 1 && s.unprobed().is_empty() {
            break;
        }
        let idx = if s.phi() == 1 {
            // already decided, but inherited labels remain: accept only a probe that can verify them.
            let Some(x) = s.unprobed().into_iter().next() else { break };
            match remaining
                .iter()
                .position(|o| matches!(o, Op::Probe { x: px, .. } if *px == x))
            {
                Some(i) => i,
                None => break,
            }
        } else {
            match select(s, &remaining) {
                Some(i) => i,
                None => break,
            }
        };
        let op = remaining.remove(idx);
        if cost.saturating_add(op.cost()) > s.budget {
            return Err(Falsified::OutOfBudget);
        }
        op.apply(s)?;
        cost += op.cost();
        ran += 1;
    }
    Ok(Outcome {
        phi: s.phi(),
        cost,
        ran,
        skipped: ops.len() - ran,
        skipped_cost: remaining.iter().map(|o| o.cost()).sum(),
    })
}

/// The result of one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub phi: usize,
    pub cost: u32,
    pub ran: usize,
    pub skipped: usize,
    /// Sum of the costs of the skipped steps (everything skipped leaves the state unchanged).
    pub skipped_cost: u32,
}

/// Run a sequence of operators in the given order. When `skip_redundant` is true, the control class really does save the redundant steps.
pub fn run(ops: &[Op], s: &mut State, skip_redundant: bool) -> Result<Outcome, Falsified> {
    let mut cost = 0u32;
    let mut ran = 0usize;
    let mut skipped = 0usize;
    let mut skipped_cost = 0u32;
    for op in ops {
        if skip_redundant && op.is_redundant(s) {
            skipped += 1;
            skipped_cost += op.cost();
            continue;
        }
        if cost.saturating_add(op.cost()) > s.budget {
            return Err(Falsified::OutOfBudget);
        }
        op.apply(s)?;
        cost += op.cost();
        ran += 1;
    }
    Ok(Outcome { phi: s.phi(), cost, ran, skipped, skipped_cost })
}

/// A set of demos: the same step sequence, fixed order vs skipping redundancy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Demo {
    pub name: &'static str,
    pub fixed_cost: u32,
    pub scheduled_cost: u32,
    pub phi_before: usize,
    pub phi_after: usize,
    pub skipped: usize,
    /// Sum of the costs of the skipped steps; should equal `fixed_cost - scheduled_cost`.
    pub skipped_cost: u32,
}

/// Three demo scenarios. Each uses only quantities from the paper (Φ, cost, redundant steps).
pub fn demo() -> Vec<Demo> {
    let mut out = Vec::new();

    // A: with no redundancy, scheduling saves nothing (paper: control is worth only the redundant part).
    {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        // three live candidates, one question settles it.
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![Op::Evidence { x: 1, member: true, cost: 1 }];
        out.push(run_pair("no-redundancy", &s, &ops));
    }

    // B: the fixed order contains repeated closures and non-discriminating questions; scheduling saves exactly their cost.
    {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: false, cost: 1 },
            Op::Evidence { x: 3, member: false, cost: 1 },
        ];
        out.push(run_pair("repeated-closure", &s, &ops));
    }

    // C: the state is exhausted (Φ=0). Generation cannot save it, only a new atom can; the fixed order also pays two extra redundancies.
    {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1); // 0 and 1 are in the same class → T2 collision
        s.hyps = s.enumerate_definable();
        let ops = vec![
            Op::Generate { cands: vec![bit(2) | bit(3)], cost: 2 },
            Op::Close { cost: 1 },
            Op::Represent { pred: bit(0), cands: vec![bit(0), bit(0) | bit(2) | bit(3)], cost: 10 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: false, cost: 1 },
        ];
        out.push(run_pair("exhausted-needs-atom", &s, &ops));
    }

    out
}

/// Run the same step sequence twice: without skipping / with skipping. Returns the row where both reach the same state.
fn run_pair(name: &'static str, s: &State, ops: &[Op]) -> Demo {
    let before = s.phi();
    let fixed = run(ops, &mut s.clone(), false).expect("fixed run");
    let sched = run(ops, &mut s.clone(), true).expect("scheduled run");
    assert_eq!(fixed.phi, sched.phi, "both schedules must reach the same state");
    assert_eq!(fixed.cost - sched.cost, sched.skipped_cost, "the difference must equal the cost of the skipped steps");
    Demo {
        name,
        phi_before: before,
        phi_after: fixed.phi,
        fixed_cost: fixed.cost,
        scheduled_cost: sched.cost,
        skipped: sched.skipped,
        skipped_cost: sched.skipped_cost,
    }
}

/// Random self-check: generate `trials` random schedules and verify two things —
/// skipping redundancy never changes the final `Φ`; the money saved equals exactly the cost of the skipped steps.
///
/// Returns `(trials, violations)`. The binary runs it on every `ops`, so the paper's §4 claim
/// 'control does not move the potential' is checked at runtime as well, not only in tests.
pub fn audit(trials: u64) -> (u64, Vec<String>) {
    let mut seed = 0x5eed_1234u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let n = 6;
    let mut violations = Vec::new();
    for t in 0..trials {
        let mut s = State::new(n);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.hyps = s.enumerate_definable();
        let mut answered = s.pos | s.neg;
        let mut ops = Vec::new();
        for _ in 0..8 {
            match rnd() % 3 {
                0 => ops.push(Op::Close { cost: 1 }),
                1 => {
                    let m = rnd() & State::full(n);
                    ops.push(Op::Generate { cands: vec![m], cost: 2 });
                }
                _ => {
                    let free: Vec<usize> =
                        (0..n).filter(|&x| answered & bit(x) == 0).collect();
                    if free.is_empty() {
                        ops.push(Op::Close { cost: 1 });
                    } else {
                        let x = free[(rnd() as usize) % free.len()];
                        answered |= bit(x);
                        ops.push(Op::Evidence { x, member: rnd() % 2 == 0, cost: 1 });
                    }
                }
            }
        }
        match (run(&ops, &mut s.clone(), false), run(&ops, &mut s.clone(), true)) {
            (Ok(a), Ok(b)) => {
                if a.phi != b.phi {
                    violations.push(format!("trial {t}: phi {} != {}", a.phi, b.phi));
                }
                if a.cost - b.cost != b.skipped_cost {
                    violations.push(format!("trial {t}: cost identity broken"));
                }
            }
            (Err(_), Err(_)) => {}
            _ => violations.push(format!("trial {t}: one schedule errored, the other did not")),
        }
    }
    (trials, violations)
}

/// The 'identified' extension under the current vocabulary: the union of all equivalence classes that contain a positive.
pub fn implied_extension(s: &State) -> Mask {
    let mut out = 0u64;
    for c in s.classes() {
        if c & s.pos != 0 {
            out |= c;
        }
    }
    out
}

/// Demo of inherited labels: an early stop yields a wrong extension, and one more representation step after the probes makes it right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeDemo {
    pub early_phi: usize,
    pub early_queries: u32,
    pub early_correct: bool,
    pub verified_queries: u32,
    pub verified_cost: u32,
    pub verified_correct: bool,
}

/// The truth is `{0}` (4 objects, not definable under the empty vocabulary).
pub fn probe_demo() -> ProbeDemo {
    let truth = bit(0);

    // early stop: ask only 0, Φ=1, but the extension is the whole class → wrong.
    let mut early = State::new(4);
    early.hyps = early.enumerate_definable();
    Op::Evidence { x: 0, member: true, cost: 1 }.apply(&mut early).expect("ask 0");

    // verified version: probe → the state falls back to insufficient → add an atom → keep probing until no inherited labels remain.
    let mut verified = early.clone();
    let ops = vec![
        Op::Probe { x: 1, member: false, cost: 1 },
        Op::Represent { pred: bit(0), cands: vec![bit(0)], cost: 10 },
        Op::Probe { x: 2, member: false, cost: 1 },
        Op::Probe { x: 3, member: false, cost: 1 },
    ];
    let out = run_selected(&mut verified, &ops).expect("verified run");

    ProbeDemo {
        early_phi: early.phi(),
        early_queries: 1,
        early_correct: implied_extension(&early) == truth,
        verified_queries: 1 + out.ran as u32,
        verified_cost: 1 + out.cost,
        verified_correct: implied_extension(&verified) == truth,
    }
}

/// JSON of the demo results (consistent with the other subcommands: machine-readable).
pub fn demo_json() -> String {
    let rows: Vec<String> = demo()
        .iter()
        .map(|d| {
            format!(
                "{{\"name\":\"{}\",\"phi_before\":{},\"phi_after\":{},\"fixed_cost\":{},\"scheduled_cost\":{},\"skipped\":{},\"skipped_cost\":{}}}",
                d.name, d.phi_before, d.phi_after, d.fixed_cost, d.scheduled_cost, d.skipped, d.skipped_cost
            )
        })
        .collect();
    let (trials, violations) = audit(200);
    let (sel_n, fixed, random, selected) = selection_bench(200);
    let c = compose_bench(200, 2, 2);
    let p = probe_demo();
    format!(
        "{{\"claim\":\"scheduling saves exactly the redundant spend\",\"audit\":{{\"trials\":{},\"violations\":{}}},\"selection\":{{\"trials\":{},\"fixed_cost\":{:.2},\"random_cost\":{:.2},\"selected_cost\":{:.2}}},\"compose\":{{\"k\":2,\"budget\":2,\"greedy\":{:.2},\"random\":{:.2},\"fixed\":{:.2},\"greedy_ok\":{:.2},\"random_ok\":{:.2},\"fixed_ok\":{:.2}}},\"probe\":{{\"early_phi\":{},\"early_queries\":{},\"early_correct\":{},\"verified_queries\":{},\"verified_cost\":{},\"verified_correct\":{}}},\"scenarios\":[{}]}}",
        trials,
        violations.len(),
        sel_n,
        fixed,
        random,
        selected,
        c.greedy,
        c.random,
        c.fixed,
        c.greedy_ok,
        c.random_ok,
        c.fixed_ok,
        p.early_phi,
        p.early_queries,
        p.early_correct,
        p.verified_queries,
        p.verified_cost,
        p.verified_correct,
        rows.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state3() -> State {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        s
    }

    /// T8: generation only adds candidates, so Φ never drops.
    #[test]
    fn t8_generation_never_reduces_phi() {
        let s = state3();
        let before = s.phi();
        let mut t = s.clone();
        Op::Generate { cands: vec![bit(0) | bit(3)], cost: 2 }.apply(&mut t).unwrap();
        assert!(t.phi() >= before);
        // add an inconsistent candidate: it is dead on arrival, and Φ still does not drop.
        let mut u = s.clone();
        Op::Generate { cands: vec![0], cost: 1 }.apply(&mut u).unwrap();
        assert_eq!(u.phi(), before);
    }

    /// T9: closing twice = closing once; the second time is a redundant step.
    #[test]
    fn t9_closure_is_idempotent() {
        let mut s = state3();
        // stuff an inconsistent candidate in (directly, simulating a broken generation operator).
        s.hyps.push(bit(2) | bit(3));
        let mut once = s.clone();
        Op::Close { cost: 1 }.apply(&mut once).unwrap();
        let phi_once = once.phi();
        Op::Close { cost: 1 }.apply(&mut once).unwrap();
        assert_eq!(once.phi(), phi_once);
        assert!(Op::Close { cost: 1 }.is_redundant(&once));
    }

    /// T10: a question that splits the live set strictly lowers Φ; one that does not lowers nothing (but it still writes E, so it is not redundant).
    #[test]
    fn t10_evidence_is_the_only_strict_reducer() {
        let s = state3();
        let before = s.phi();
        let mut t = s.clone();
        Op::Evidence { x: 1, member: true, cost: 1 }.apply(&mut t).unwrap();
        assert!(t.phi() < before);

        // every live candidate contains 1 and 1 has not been asked yet: asking 1 does not change Φ, but it is new evidence (writes E), not a redundant step.
        let mut s2 = State::new(4);
        s2.pos = bit(0);
        s2.hyps = vec![
            bit(0) | bit(1),
            bit(0) | bit(1) | bit(2),
            bit(0) | bit(1) | bit(3),
        ];
        let q = Op::Evidence { x: 1, member: true, cost: 1 };
        assert!(!q.is_discriminating(&s2));
        assert!(!q.is_redundant(&s2));
        let mut u = s2.clone();
        q.apply(&mut u).unwrap();
        assert_eq!(u.phi(), s2.phi());
        // asking the same question a second time is what makes it redundant.
        assert!(q.is_redundant(&u));
    }

    /// T11: a predicate built by Boolean recombination of the vocabulary is rejected outright by the warrant.
    #[test]
    fn t11_recombination_is_rejected() {
        let s = State::new(4);
        // the vocabulary has one predicate p = {0,1}; not(p) is still a Boolean combination of it.
        let mut v = s.clone();
        v.vocab = vec![bit(0) | bit(1)];
        let not_p = State::full(4) & !(bit(0) | bit(1));
        let op = Op::Represent { pred: not_p, cands: vec![not_p], cost: 5 };
        assert_eq!(op.warrant(&v), Err(Falsified::Recombination));
        // a new atom (outside the Boolean closure) passes.
        let atom = bit(0) | bit(2);
        assert!(!v.definable(atom));
        assert_eq!(Op::Represent { pred: atom, cands: vec![atom], cost: 5 }.warrant(&v), Ok(()));
    }

    /// First half of the corollary: at Φ=0, generation/closure/evidence cannot help.
    #[test]
    fn exhausted_state_survives_without_a_new_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        s.hyps = s.enumerate_definable();
        assert_eq!(s.phi(), 0);
        let ops = vec![
            Op::Generate { cands: vec![bit(0) | bit(1) | bit(2)], cost: 1 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: true, cost: 1 },
        ];
        let out = run(&ops, &mut s, false).unwrap();
        assert_eq!(out.phi, 0);
    }

    /// Second half of the corollary: only after a new atom arrives can Φ exceed 0.
    #[test]
    fn a_new_atom_can_raise_phi() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        s.hyps = s.enumerate_definable();
        let before = s.phi();
        Op::Represent { pred: bit(0), cands: vec![bit(0)], cost: 5 }.apply(&mut s).unwrap();
        assert!(s.phi() > before);
    }

    /// An atom that is only useful later (coverage 0 now) is still legal: coverage is only a scheduling score.
    #[test]
    fn represent_allows_a_future_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        assert_eq!(s.colliding_pairs(), vec![(0, 1)]);
        // {2} splits the class {2,3} (not definable), but does not separate the colliding (0,1).
        let op = Op::Represent { pred: bit(2), cands: vec![], cost: 5 };
        assert_eq!(op.warrant(&s), Ok(()));
        assert_eq!(op.coverage(&s), 0, "zero coverage now, but it is still a legal step");
    }

    /// Every candidate must be definable under the new vocabulary V ∪ {pred}; otherwise Φ=1 is a fake 'decided'.
    #[test]
    fn represent_rejects_candidates_outside_new_vocabulary() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0) | bit(2);
        s.neg = bit(1) | bit(3);
        // the candidate {0,2} still splits {2,3} under V' = {V, {0}} → not definable.
        let bad = Op::Represent { pred: bit(0), cands: vec![bit(0) | bit(2)], cost: 5 };
        assert_eq!(bad.warrant(&s), Err(Falsified::NotExpressible));

        // with the same atom, if the evidence only requires 0 and excludes 1, then both {0} and {0,2,3} are legal.
        let mut t = State::new(4);
        t.vocab = vec![bit(0) | bit(1)];
        t.pos = bit(0);
        t.neg = bit(1);
        let ok = Op::Represent {
            pred: bit(0),
            cands: vec![bit(0), bit(0) | bit(2) | bit(3)],
            cost: 5,
        };
        assert_eq!(ok.warrant(&t), Ok(()));
    }

    /// `(wr, dir)` gives the four classes four distinct values; with `wr` alone, generation and closure collide.
    /// This is the machine version of 'why 4 and not 3, not 5': counting the classes requires counting the direction bit.
    #[test]
    fn type_with_direction_separates_the_classes() {
        let ops = [
            Op::Generate { cands: vec![], cost: 1 },
            Op::Close { cost: 1 },
            Op::Represent { pred: 0, cands: vec![], cost: 1 },
            Op::Evidence { x: 0, member: true, cost: 1 },
        ];
        // wr alone: generation and closure share a signature.
        assert_eq!(ops[0].writes(), ops[1].writes());
        // add direction: the four signatures are pairwise distinct.
        let mut seen: Vec<(u8, Dir)> = Vec::new();
        for op in &ops {
            let sig = (op.writes(), op.dir());
            assert!(!seen.contains(&sig), "{:?} duplicates an existing signature", op.class());
            seen.push(sig);
        }
        assert_eq!(seen.len(), 4);
    }

    /// A relation projects out a unary predicate, which then enters the vocabulary and separates the colliding pair.
    #[test]
    fn relations_enter_through_lifts() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        // relation r: 0→2, 1→3.
        s.rels = vec![vec![bit(2), bit(3), 0, 0]];
        assert_eq!(s.image(0, bit(0)), bit(2));
        assert_eq!(s.preimage(0, bit(2)), bit(0));
        // the preimage {0} is not definable under the old vocabulary, yet it separates (0,1) exactly.
        let lift = s.preimage(0, bit(2));
        assert_eq!(lift, bit(0));
        assert!(!s.definable(lift));
        let op = Op::Represent { pred: lift, cands: vec![lift], cost: 5 };
        assert_eq!(op.warrant(&s), Ok(()));
        assert_eq!(op.coverage(&s), 1);
    }

    /// Images of a relation compose (two steps of reachability).
    #[test]
    fn relational_images_compose() {
        let mut s = State::new(4);
        s.rels = vec![vec![bit(1), bit(2), bit(3), 0]];
        assert_eq!(s.image(0, bit(0)), bit(1));
        assert_eq!(s.image(0, s.image(0, bit(0))), bit(2));
        assert_eq!(s.preimage(0, bit(2)), bit(1));
        assert_eq!(s.preimage(0, bit(0)), 0);
    }

    /// MDL: how many literals it takes to describe a set as a conjunction over the current vocabulary.
    #[test]
    fn mdl_counts_literals() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1), bit(0)];
        assert_eq!(s.mdl(bit(0)), Some(1));
        assert_eq!(s.mdl(bit(1)), Some(2));
        assert_eq!(s.mdl(bit(2) | bit(3)), Some(1));
        assert_eq!(s.mdl(bit(0) | bit(2)), None);
    }

    /// Selection: a question that can split is preferred over one that cannot.
    #[test]
    fn selection_prefers_a_discriminating_question() {
        let mut s = State::new(5);
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![
            Op::Evidence { x: 4, member: false, cost: 1 }, // no live candidate contains 4: does not split
            Op::Evidence { x: 1, member: true, cost: 1 },  // splits
        ];
        assert_eq!(select(&s, &ops), Some(1));
    }

    /// Selection: an atom that covers more colliding pairs is preferred.
    #[test]
    fn selection_prefers_higher_coverage_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0) | bit(2);
        s.neg = bit(1) | bit(3);
        let ops = vec![
            Op::Represent { pred: bit(0), cands: vec![], cost: 5 },
            Op::Represent { pred: bit(0) | bit(2), cands: vec![], cost: 5 },
        ];
        assert_eq!(ops[1].coverage(&s), 2);
        assert_eq!(select(&s, &ops), Some(1));
    }

    /// Stepwise reselection reaches a decision and never runs a step twice.
    #[test]
    fn run_selected_reaches_a_decision() {
        let mut s = State::new(5);
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![
            Op::Evidence { x: 4, member: false, cost: 1 },
            Op::Evidence { x: 1, member: true, cost: 1 },
        ];
        let out = run_selected(&mut s, &ops).expect("selected run");
        assert_eq!(out.phi, 1);
        assert_eq!(out.ran, 1, "the first question is enough to decide");
        assert_eq!(out.cost, 1);
    }

    /// When a single atom suffices, the one with the largest coverage is the sufficient one (T7): it must separate all colliding pairs.
    #[test]
    fn best_cover_picks_the_sufficient_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0) | bit(2);
        s.neg = bit(1) | bit(3);
        // colliding pairs (0,1) and (2,3); the target is S = {0,2}.
        let decoy = bit(0);
        let good = bit(0) | bit(2);
        let pool = vec![decoy, good];
        assert_eq!(best_cover(&s, &pool), Some(1));
        // use it for one representation step; Φ goes straight to 1.
        let op = Op::Represent { pred: good, cands: vec![good], cost: 5 };
        assert_eq!(op.warrant(&s), Ok(()));
        op.apply(&mut s).expect("sufficient atom");
        assert_eq!(s.phi(), 1);
    }

    /// Same-budget comparison: stepwise selection should not cost more than a fixed order (synthesized 8-candidate state, deterministic seed).
    #[test]
    fn selection_bench_beats_fixed_order() {
        let (n, fixed, random, selected) = selection_bench(50);
        assert!(n > 0);
        assert!(selected <= fixed, "selected {selected} vs fixed {fixed}");
        assert!(selected <= random + 0.5, "selected {selected} vs random {random}");
    }

    /// When two atoms are needed: greedy covering needs two, while the fixed order (whole-class atoms first) fails within the budget.
    #[test]
    fn compose_bench_greedy_composes() {
        let st = compose_bench(50, 2, 2);
        assert_eq!(st.greedy, 2.0);
        assert_eq!(st.greedy_ok, 1.0);
        assert!(st.greedy < st.random, "greedy {} vs random {}", st.greedy, st.random);
        assert_eq!(st.fixed_ok, 0.0);
    }

    /// Probe: an unverified member can be asked; after asking, the state may fall back from 'decided' to 'insufficient'.
    #[test]
    fn probe_can_reopen_a_decided_state() {
        let mut s = State::new(3);
        s.pos = bit(0);
        s.asked = bit(0); // 0 was asked directly
        s.hyps = vec![bit(0) | bit(1) | bit(2)];
        assert_eq!(s.phi(), 1);
        assert_eq!(s.unprobed(), vec![1, 2]);
        let op = Op::Probe { x: 1, member: false, cost: 1 };
        assert_eq!(op.warrant(&s), Ok(()));
        op.apply(&mut s).expect("probe");
        assert_eq!(s.phi(), 0, "the inherited label was refuted; the state falls back to insufficient");
        assert_eq!(s.asked, bit(0) | bit(1));
    }

    /// The probe warrant: never asked directly, and the class has more than one member.
    #[test]
    fn probe_requires_an_unasked_member_of_a_nontrivial_class() {
        let mut s = State::new(3);
        s.vocab = vec![bit(0)]; // classes {0} and {1,2}
        s.pos = bit(0);
        Op::Evidence { x: 0, member: true, cost: 1 }.apply(&mut s).unwrap();
        assert_eq!(Op::Probe { x: 0, member: true, cost: 1 }.warrant(&s), Err(Falsified::AlreadyAsked));
        assert_eq!(Op::Probe { x: 1, member: true, cost: 1 }.warrant(&s), Ok(()));

        let mut t = State::new(3);
        t.vocab = vec![bit(0)];
        assert_eq!(Op::Probe { x: 0, member: true, cost: 1 }.warrant(&t), Err(Falsified::NotInherited));
    }

    /// Ordinary evidence also records 'asked directly'; singleton classes are not counted in the unverified set.
    #[test]
    fn evidence_marks_asked_and_unprobed_skips_singletons() {
        let mut s = State::new(3);
        s.vocab = vec![bit(0)];
        s.pos = bit(0);
        assert_eq!(s.unprobed(), vec![1, 2]);
        Op::Evidence { x: 2, member: true, cost: 1 }.apply(&mut s).unwrap();
        assert_eq!(s.asked, bit(2));
        assert_eq!(s.unprobed(), vec![1]);
    }

    /// Stopping rule: when Φ=1 but inherited labels remain, probe first; a refuted probe falls back to insufficient.
    #[test]
    fn run_selected_probes_before_stopping() {
        let make = || {
            let mut s = State::new(2);
            s.pos = bit(0);
            s.asked = bit(0);
            s.hyps = vec![bit(0) | bit(1)];
            s
        };
        let mut s = make();
        assert_eq!(s.phi(), 1);
        assert_eq!(s.unprobed(), vec![1]);

        let out = run_selected(&mut s, &[Op::Probe { x: 1, member: false, cost: 1 }]).unwrap();
        assert_eq!(out.ran, 1);
        assert_eq!(out.phi, 0, "the probe refuted the inherited label");

        let mut t = make();
        let out = run_selected(&mut t, &[Op::Probe { x: 1, member: true, cost: 1 }]).unwrap();
        assert_eq!(out.ran, 1);
        assert_eq!(out.phi, 1);
        assert!(t.unprobed().is_empty(), "no inherited labels remain after probing");

        // no probe operator is available: stop at Φ=1, but unverified members remain (the caller must know).
        let mut u = make();
        let out = run_selected(&mut u, &[]).unwrap();
        assert_eq!(out.ran, 0);
        assert_eq!(u.unprobed(), vec![1]);
    }

    /// Demo: at Φ=1 the early stop yields a wrong extension; after probing it is correct.
    #[test]
    fn probe_demo_shows_the_gap() {
        let p = probe_demo();
        assert_eq!(p.early_phi, 1);
        assert!(!p.early_correct, "the early-stopped extension should be wrong");
        assert!(p.verified_correct, "it should be right after probing");
        assert!(p.verified_cost > 1);
    }

    /// Control is not a step but the skip mode of [`run`]: it writes `B` and does not move `Φ`.
    #[test]
    fn control_is_the_scheduler_not_a_step() {
        assert_eq!(Class::Control.writes(), "B");
        let s = state3();
        let ops = vec![Op::Close { cost: 1 }, Op::Close { cost: 1 }];
        let fixed = run(&ops, &mut s.clone(), false).unwrap();
        let sched = run(&ops, &mut s.clone(), true).unwrap();
        assert_eq!(fixed.phi, sched.phi);
    }

    /// The two relations of paper Def 5: informed-by / redundant.
    #[test]
    fn type_relations_follow_definition() {
        let close = Op::Close { cost: 1 };
        let repr = Op::Represent { pred: bit(0), cands: vec![], cost: 5 };
        let ev = Op::Evidence { x: 1, member: true, cost: 1 };
        let gen = Op::Generate { cands: vec![bit(0)], cost: 2 };
        assert!(informed_by(&close, &repr), "closure writes H; representation reads H");
        assert!(!redundant_after(&close, &repr), "representation writes the new component V");
        assert!(redundant_after(&close, &close), "closure cannot write what closure never wrote");
        assert!(!redundant_after(&gen, &ev), "evidence writes E; generation never wrote E");
        assert!(informed_by(&ev, &gen), "evidence writes E; generation reads E");
        assert_eq!(repr.writes(), comp::V | comp::H);
        assert_eq!(ev.writes(), comp::E);
    }

    /// `is_discriminating` must be equivalent to 'Φ strictly drops after applying'.
    #[test]
    fn discriminating_iff_phi_drops() {
        let mut seed = 7u64;
        let mut rnd = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            seed >> 33
        };
        for _ in 0..200 {
            let n = 6;
            let mut s = State::new(n);
            s.vocab = vec![bit(0) | bit(1)];
            s.pos = bit(0);
            s.neg = bit(2);
            s.hyps = s.enumerate_definable();
            for _ in 0..3 {
                let m = rnd() & State::full(n);
                if !s.hyps.contains(&m) {
                    s.hyps.push(m);
                }
            }
            let free: Vec<usize> =
                (0..n).filter(|&x| s.pos & bit(x) == 0 && s.neg & bit(x) == 0).collect();
            if free.is_empty() {
                continue;
            }
            let x = free[(rnd() as usize) % free.len()];
            let op = Op::Evidence { x, member: rnd() % 2 == 0, cost: 1 };
            let before = s.phi();
            let mut t = s.clone();
            op.apply(&mut t).unwrap();
            assert_eq!(op.is_discriminating(&s), t.phi() < before, "x={x}");
        }
    }

    /// The money saved by scheduling must equal exactly the cost of the skipped steps (the paper's 'control is worth only redundancy').
    #[test]
    fn scheduling_saves_exactly_the_redundant_spend() {
        let s = state3();
        // three closures + two evidence steps, of which two closures are redundant.
        let ops = vec![
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: false, cost: 1 },
            Op::Evidence { x: 3, member: false, cost: 1 },
        ];
        let fixed = run(&ops, &mut s.clone(), false).unwrap();
        let sched = run(&ops, &mut s.clone(), true).unwrap();
        assert_eq!(fixed.phi, sched.phi, "both orders must reach the same state");
        assert_eq!(fixed.cost - sched.cost, sched.skipped as u32);
    }

    /// Return the falsifier when the budget is exhausted, instead of silently continuing.
    #[test]
    fn budget_is_enforced() {
        let mut s = state3();
        s.budget = 1;
        let ops = vec![Op::Evidence { x: 1, member: true, cost: 2 }];
        assert_eq!(run(&ops, &mut s, false), Err(Falsified::OutOfBudget));
    }

    /// Every demo row must satisfy: fixed cost − scheduled cost = cost of the skipped steps.
    #[test]
    fn demo_cost_identity_holds() {
        let rows = demo();
        assert_eq!(rows.len(), 3);
        for d in rows {
            assert_eq!(d.fixed_cost - d.scheduled_cost, d.skipped_cost, "{}", d.name);
            assert!(d.scheduled_cost <= d.fixed_cost, "{}", d.name);
        }
    }

    /// Random schedules: skipping redundancy never changes the final state, and the money saved equals exactly the cost of the skipped steps.
    /// It runs the same [`audit`] as the binary's `ops`.
    #[test]
    fn random_schedules_preserve_state_and_accounting() {
        let (trials, violations) = audit(200);
        assert_eq!(trials, 200);
        assert!(violations.is_empty(), "{violations:?}");
    }
}
