//! Translation: a change of vocabulary as a morphism, and the judgments that make it checkable.
//!
//! Everything in the kernel is stated over a *fixed* object set: [`Explorer`](crate::Explorer) and
//! [`State`](crate::operators::State) both hold a vocabulary and never change the objects it is
//! stated over. Adding an atom is therefore written as an in-place push, which makes it look like
//! an operation of the structure. It is not: it changes the signature, and a signature change is a
//! move to a different theory, not a function inside one.
//!
//! This module states the general case — a theory on each side, and a map on the objects between
//! them — so the in-place case becomes the one where the object map is the identity:
//!
//! | case | objects | vocabulary | what it can fail at |
//! |---|---|---|---|
//! | extend | identity | grows | nothing, in this fragment |
//! | quotient | merges | projected | an old atom is cut by a fibre |
//! | re-axiomatise | identity | changes | an old atom is no longer definable |
//!
//! Three judgments, each cheap and each carrying a witness:
//!
//! - **merged**: the map puts two objects in one fibre that an old atom separates. The witness is
//!   the atom and the pair, so a rejection names what was lost instead of saying 'illegal'.
//! - **unsayable**: no set definable in the new vocabulary pulls back to an old atom's extension.
//!   The old language cannot be re-stated — the failure mode of a re-axiomatisation.
//! - **gain**: how many new-definable sets pull back to sets the old vocabulary could not define.
//!   Zero means the translation is a *change of wording*: faithful, and worth nothing. This is the
//!   conservative extension of reverse mathematics, made countable.
//!
//! ## What this module deliberately cannot see
//!
//! **The labels.** Fidelity is relative to the vocabulary; a collision is relative to the labels.
//! Merging two objects that already share a signature loses no definable set — and destroys the
//! only proof that the vocabulary was insufficient. That is [`Translation::collapses`], and it is
//! why a quotient needs `pos` / `neg` as well as an atom list. A check that only reads the atoms
//! will call such a merge harmless.
//!
//! ## What is not here
//!
//! The gain counts *sets*, not usefulness: an atom that adds many definable sets and separates
//! nothing is legitimate and worthless, and the count does not say so. Ranking atoms is a separate
//! judgment (coverage, net utility) and must not be folded into this one.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;

use crate::{bit, Mask};

/// How many classes may be enumerated before a judgment gives up. `2^20` unions is the ceiling the
/// rest of the kernel uses; above it the gain is reported as undecided rather than guessed.
pub const MAX_CLASSES: usize = 20;

/// A vocabulary together with the object set it is stated over.
///
/// The kernel keeps masks without names (`State::vocab`) or names without an object count
/// (`Vocabulary`); a theory needs both, because a translation can change either.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theory {
    pub objects: usize,
    pub atoms: Vec<(String, Mask)>,
}

impl Theory {
    pub fn new(objects: usize, atoms: Vec<(String, Mask)>) -> Self {
        assert!(objects <= 64, "Mask is u64, so at most 64 objects");
        Self { objects, atoms }
    }

    /// A theory from bare masks, the shape the kernel's state already has.
    pub fn of(objects: usize, masks: &[Mask]) -> Self {
        Self::new(
            objects,
            masks.iter().enumerate().map(|(i, m)| (format!("p{i}"), *m)).collect(),
        )
    }

    pub fn signature(&self, x: usize) -> Vec<bool> {
        let b = bit(x);
        self.atoms.iter().map(|(_, m)| m & b != 0).collect()
    }

    /// Equivalence classes in the order of their first object. Deterministic, unlike
    /// `Explorer::classes`, which folds through a `HashMap`.
    pub fn classes(&self) -> Vec<Mask> {
        let mut keys: Vec<Vec<bool>> = Vec::new();
        let mut out: Vec<Mask> = Vec::new();
        for x in 0..self.objects {
            let s = self.signature(x);
            match keys.iter().position(|k| *k == s) {
                Some(i) => out[i] |= bit(x),
                None => {
                    keys.push(s);
                    out.push(bit(x));
                }
            }
        }
        out
    }

    /// Constant on every class: T1 in the form a translation needs it.
    pub fn definable(&self, s: Mask) -> bool {
        self.classes().into_iter().all(|c| {
            let inside = s & c;
            inside == 0 || inside == c
        })
    }

    /// Every union of classes, when the class count is small enough to enumerate.
    pub fn definable_sets(&self, max_classes: usize) -> Option<Vec<Mask>> {
        let cs = self.classes();
        if cs.len() > max_classes {
            return None;
        }
        Some(
            (0u32..(1u32 << cs.len()))
                .map(|picked| {
                    let mut out = 0u64;
                    for (i, c) in cs.iter().enumerate() {
                        if picked & (1 << i) != 0 {
                            out |= c;
                        }
                    }
                    out
                })
                .collect(),
        )
    }
}

/// A witnessed loss: the reason a translation is rejected, named rather than gestured at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lost {
    /// One fibre holds two objects that `atom` separates.
    Merged { atom: String, a: usize, b: usize },
    /// No new-definable set pulls back to `atom`'s extension.
    Unsayable { atom: String },
}

/// The verdict on a translation. `Lossy` carries its witness; `Undecided` carries the class count
/// that stopped the enumeration, so a caller can raise `max_classes` instead of guessing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Lossy(Lost),
    /// Faithful, and every new definable set is already old-definable.
    WordingOnly,
    /// Faithful, and the new vocabulary can say something about the old objects it could not before.
    Genuine,
    Undecided,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Lossy(_) => "lossy",
            Kind::WordingOnly => "wording_only",
            Kind::Genuine => "genuine",
            Kind::Undecided => "undecided",
        }
    }
}

/// One measurement of a translation. The counts are reported separately from the verdict on
/// purpose: a number is not a score, and `new_sets` must not be added to a coverage count.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub kind: Kind,
    /// New-definable sets whose pullback is not old-definable; `None` when undecided.
    pub new_sets: Option<usize>,
    /// How many new-definable sets were enumerated; `None` when undecided.
    pub sets: Option<usize>,
    pub from_classes: usize,
    pub to_classes: usize,
    pub fibres: usize,
}

/// A change of theory: a total map on objects plus the vocabulary on the far side.
#[derive(Clone, Debug)]
pub struct Translation {
    pub from: Theory,
    pub to: Theory,
    /// `map[x]` is the object of `to` that `x` of `from` lands on.
    pub map: Vec<usize>,
}

impl Translation {
    /// The identity on objects: the case `Op::Represent` implements in place.
    pub fn identity(t: Theory) -> Self {
        let n = t.objects;
        Self { from: t.clone(), to: t, map: (0..n).collect() }
    }

    /// Add one atom and change nothing else. This is the morphism the kernel's representation step
    /// performs; writing it here makes the step a special case rather than a primitive.
    pub fn extend(from: Theory, name: &str, ext: Mask) -> Self {
        let n = from.objects;
        assert!(n >= 64 || ext >> n == 0, "the atom names objects outside the theory");
        let mut to = from.clone();
        to.atoms.push((name.to_string(), ext));
        Self { from, to, map: (0..n).collect() }
    }

    /// The coarsest quotient along `map`: every old atom is projected, so the quotient loses a
    /// distinction only when `map` itself merges one. A caller that wants a quotient to *drop*
    /// information on purpose writes `to` explicitly instead.
    pub fn quotient(from: &Theory, map: Vec<usize>) -> Self {
        assert_eq!(map.len(), from.objects, "the object map must be total");
        let to_objects = map.iter().copied().max().map_or(0, |m| m + 1);
        let atoms = from
            .atoms
            .iter()
            .map(|(name, ext)| {
                let mut projected = 0u64;
                for x in 0..from.objects {
                    if ext & bit(x) != 0 {
                        projected |= bit(map[x]);
                    }
                }
                (name.clone(), projected)
            })
            .collect();
        Self {
            from: from.clone(),
            to: Theory::new(to_objects, atoms),
            map,
        }
    }

    /// The first object pair an old atom separates that the map puts in one fibre.
    pub fn merged(&self) -> Option<Lost> {
        for (name, ext) in &self.from.atoms {
            for a in 0..self.from.objects {
                for b in (a + 1)..self.from.objects {
                    if self.map[a] == self.map[b]
                        && ((ext >> a) & 1) != ((ext >> b) & 1)
                    {
                        return Some(Lost::Merged { atom: name.clone(), a, b });
                    }
                }
            }
        }
        None
    }

    /// A set of old objects pulled back from a set of new ones.
    pub fn pullback(&self, d: Mask) -> Mask {
        let mut out = 0u64;
        for x in 0..self.from.objects {
            if (d >> self.map[x]) & 1 != 0 {
                out |= bit(x);
            }
        }
        out
    }

    /// The object pairs one fibre holds with *different* labels. This is the loss a fidelity check
    /// on atoms cannot see: the merge keeps every definable set and erases the collision that
    /// proved the vocabulary insufficient.
    pub fn collapses(&self, pos: Mask, neg: Mask) -> Option<(usize, usize)> {
        for a in 0..self.from.objects {
            for b in 0..self.from.objects {
                if self.map[a] != self.map[b] {
                    continue;
                }
                let mixed = (pos & bit(a) != 0 && neg & bit(b) != 0)
                    || (neg & bit(a) != 0 && pos & bit(b) != 0);
                if mixed {
                    return Some((a, b));
                }
            }
        }
        None
    }

    pub fn fibres(&self) -> usize {
        let mut seen: Vec<usize> = Vec::new();
        for &y in &self.map {
            if !seen.contains(&y) {
                seen.push(y);
            }
        }
        seen.len()
    }

    /// The three judgments, in the order that makes the witness the most concrete: a merged pair is
    /// a fact about two objects, an unsayable atom is a fact about one, and a gain is a count.
    pub fn analyse(&self, max_classes: usize) -> Analysis {
        let from_classes = self.from.classes().len();
        let to_classes = self.to.classes().len();
        let mut out = Analysis {
            kind: Kind::Undecided,
            new_sets: None,
            sets: None,
            from_classes,
            to_classes,
            fibres: self.fibres(),
        };
        if let Some(lost) = self.merged() {
            out.kind = Kind::Lossy(lost);
            return out;
        }
        let Some(sets) = self.to.definable_sets(max_classes) else {
            return out;
        };
        for (name, ext) in &self.from.atoms {
            if !sets.iter().any(|d| self.pullback(*d) == *ext) {
                out.kind = Kind::Lossy(Lost::Unsayable { atom: name.clone() });
                return out;
            }
        }
        let new_sets = sets.iter().filter(|d| !self.from.definable(self.pullback(**d))).count();
        out.sets = Some(sets.len());
        out.new_sets = Some(new_sets);
        out.kind = if new_sets == 0 {
            Kind::WordingOnly
        } else {
            Kind::Genuine
        };
        out
    }
}

/// The input shape of `metaform translate`. Names are sorted by `BTreeMap`, matching the kernel's
/// `canonical_names` discipline, so two files that differ only in atom order give the same answer.
#[derive(Debug, Deserialize)]
struct Side {
    objects: usize,
    #[serde(default)]
    atoms: BTreeMap<String, Vec<usize>>,
}

impl Side {
    fn theory(&self) -> Result<Theory, String> {
        let mut atoms = Vec::new();
        for (name, ids) in &self.atoms {
            let mut ext = 0u64;
            for &i in ids {
                if i >= self.objects {
                    return Err(format!(
                        "atom `{name}` names object {i}, but the theory has {} objects",
                        self.objects
                    ));
                }
                ext |= bit(i);
            }
            atoms.push((name.clone(), ext));
        }
        Ok(Theory::new(self.objects, atoms))
    }
}

#[derive(Debug, Deserialize)]
struct Spec {
    from: Side,
    /// Omitted: derived as the coarsest quotient along `map`.
    #[serde(default)]
    to: Option<Side>,
    /// Omitted: the identity, which is the `extend` / `re-axiomatise` case.
    #[serde(default)]
    map: Option<Vec<usize>>,
    #[serde(default)]
    pos: Vec<usize>,
    #[serde(default)]
    neg: Vec<usize>,
}

/// `metaform translate --input <spec.json|-> [--max-classes N]`: one JSON line, no file written.
///
/// The spec names the two theories and the object map; `to` or `map` may be omitted, and omitting
/// `map` is what the kernel's representation step does. `pos` / `neg` are optional and turn on the
/// label check, which is the only judgment that catches a merge of two objects that already shared
/// a signature.
pub fn run(args: &[String]) -> Result<String, String> {
    let path = match crate::flag_value(args, "--input")? {
        Some(p) => p,
        None => return Err("usage: metaform translate --input <spec.json|-> [--max-classes N]".into()),
    };
    let max_classes = match crate::flag_value(args, "--max-classes")? {
        Some(v) => v.parse::<usize>().map_err(|_| format!("--max-classes expects a number, got `{v}`"))?,
        None => MAX_CLASSES,
    };
    let text = if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("cannot read stdin: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read `{path}`: {e}"))?
    };
    let spec: Spec = serde_json::from_str(&text).map_err(|e| format!("invalid spec: {e}"))?;

    let from = spec.from.theory()?;
    let n = from.objects;
    let map = match &spec.map {
        Some(m) => {
            if m.len() != n {
                return Err(format!("the map has {} entries, the theory has {n} objects", m.len()));
            }
            m.clone()
        }
        None => (0..n).collect(),
    };
    let translation = match spec.to {
        Some(side) => Translation { from, to: side.theory()?, map },
        None => Translation::quotient(&from, map),
    };

    let a = translation.analyse(max_classes);
    let mut report = json!({
        "kind": a.kind.as_str(),
        "new_sets": a.new_sets,
        "sets": a.sets,
        "from": { "objects": translation.from.objects, "atoms": translation.from.atoms.len(), "classes": a.from_classes },
        "to": { "objects": translation.to.objects, "atoms": translation.to.atoms.len(), "classes": a.to_classes },
        "fibres": a.fibres,
    });
    if let Kind::Lossy(lost) = &a.kind {
        report["witness"] = match lost {
            Lost::Merged { atom, a, b } => json!({ "kind": "merged", "atom": atom, "a": a, "b": b }),
            Lost::Unsayable { atom } => json!({ "kind": "unsayable", "atom": atom }),
        };
    }
    if !spec.pos.is_empty() || !spec.neg.is_empty() {
        let mask = |ids: &[usize]| {
            ids.iter()
                .filter(|&&i| i < n)
                .fold(0u64, |acc, &i| acc | bit(i))
        };
        report["collapsed"] = match translation.collapses(mask(&spec.pos), mask(&spec.neg)) {
            Some((a, b)) => json!({ "a": a, "b": b }),
            None => json!(null),
        };
    }
    Ok(report.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operators::State;

    /// Six objects, one atom: a = {0,1,3}. Classes {0,1,3} and {2,4,5}.
    fn six() -> Theory {
        Theory::new(6, vec![("a".into(), bit(0) | bit(1) | bit(3))])
    }

    fn kind(t: &Translation) -> Kind {
        t.analyse(MAX_CLASSES).kind
    }

    #[test]
    fn the_identity_buys_nothing() {
        let t = Translation::identity(six());
        assert_eq!(kind(&t), Kind::WordingOnly);
        assert_eq!(t.analyse(MAX_CLASSES).new_sets, Some(0));
    }

    /// Adding an atom that already is a Boolean combination of the vocabulary is T11 as a
    /// measurement rather than as a rule: the gain is zero, so the step is a change of wording.
    #[test]
    fn a_recombination_is_a_change_of_wording() {
        let complement = bit(2) | bit(4) | bit(5);
        let t = Translation::extend(six(), "not_a", complement);
        assert_eq!(kind(&t), Kind::WordingOnly);

        let intersection = bit(0);
        let t = Translation::extend(six(), "a_and_b", intersection);
        // {0} is *not* a-definable, so this one does buy something.
        assert_eq!(kind(&t), Kind::Genuine);
        assert!(t.analyse(MAX_CLASSES).new_sets.unwrap() > 0);
    }

    /// The kernel's representation warrant is the gain test, not a separate rule. Exhaustive over
    /// every atom on a four-object theory, so a future edit to either side breaks this test.
    #[test]
    fn the_represent_warrant_is_the_gain_test() {
        let theory = Theory::new(4, vec![("a".into(), bit(0) | bit(1))]);
        let mut state = State::new(4);
        state.vocab = theory.atoms.iter().map(|(_, m)| *m).collect();
        for ext in 0u64..(1 << 4) {
            let definable = state.definable(ext);
            let wording_only = kind(&Translation::extend(theory.clone(), "x", ext)) == Kind::WordingOnly;
            assert_eq!(definable, wording_only, "atom {ext:04b}");
        }
    }

    /// Merging two objects an atom separates is the failure the kernel cannot currently express.
    #[test]
    fn a_merge_of_a_separated_pair_is_lossy() {
        // 0 and 3 are both in a, 2 is not: merge 0 (in a) with 2 (not in a).
        let map = vec![0, 1, 0, 3, 4, 5];
        let t = Translation::quotient(&six(), map);
        assert_eq!(
            kind(&t),
            Kind::Lossy(Lost::Merged { atom: "a".into(), a: 0, b: 2 })
        );
    }

    /// Merging two objects that already share a signature loses no definable set; in this fragment
    /// it is a change of wording, not a loss.
    #[test]
    fn a_merge_of_an_inseparable_pair_is_not_lossy() {
        // 2 and 4 are both outside a: same signature.
        let map = vec![0, 1, 2, 3, 2, 5];
        let t = Translation::quotient(&six(), map);
        assert_eq!(kind(&t), Kind::WordingOnly);
        assert_eq!(t.fibres(), 5);
    }

    /// The judgment the atom list cannot make: that same harmless merge erases a collision when the
    /// two merged objects carry different labels.
    #[test]
    fn a_safe_merge_still_collapses_a_collision() {
        let map = vec![0, 1, 2, 3, 2, 5];
        let t = Translation::quotient(&six(), map);
        assert_eq!(kind(&t), Kind::WordingOnly);
        assert_eq!(t.collapses(bit(2), bit(4)), Some((2, 4)));
        assert_eq!(t.collapses(bit(0), bit(3)), None);
    }

    /// Re-stating the vocabulary in a language that cannot say what it said.
    #[test]
    fn an_unsayable_old_atom_is_lossy() {
        // Same objects, and `to` drops `a`: nothing definable there pulls back to `a`.
        let from = six();
        let to = Theory::new(6, vec![]);
        let t = Translation { from, to, map: (0..6).collect() };
        assert_eq!(kind(&t), Kind::Lossy(Lost::Unsayable { atom: "a".into() }));
    }

    /// `extend` is the identity map plus one atom, so the general machinery must agree with the
    /// special case on the thing the special case can decide.
    #[test]
    fn extend_is_the_identity_map_with_one_more_atom() {
        let t = Translation::extend(six(), "b", bit(1) | bit(2) | bit(5));
        assert_eq!(t.map, (0..6).collect::<Vec<_>>());
        assert_eq!(t.fibres(), 6);
        assert_eq!(t.from.atoms.len() + 1, t.to.atoms.len());
        assert_eq!(kind(&t), Kind::Genuine);
    }

    #[test]
    fn a_spec_without_a_map_is_the_extend_case() {
        let spec: Spec = serde_json::from_str(
            r#"{"from":{"objects":3,"atoms":{"a":[0,1]}},"to":{"objects":3,"atoms":{"a":[0,1],"b":[1]}}}"#,
        )
        .expect("spec parses");
        let from = spec.from.theory().expect("from");
        let to = spec.to.expect("to").theory().expect("to");
        let t = Translation { from, to, map: spec.map.unwrap_or_else(|| (0..3).collect()) };
        assert_eq!(kind(&t), Kind::Genuine);
    }
}
