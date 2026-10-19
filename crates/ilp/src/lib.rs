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

/// Helper: a ground atom over constants.
pub fn ground(pred: &str, args: &[u32]) -> Atom {
    Atom { pred: pred.to_string(), args: args.iter().map(|&c| Term::Const(c)).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
