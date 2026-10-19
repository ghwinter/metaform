//! Encoding of structures into hypervectors.
//!
//! The recipe is the standard one (Plate 1995; Kanerva 2009): a role is a random vector, a filler
//! is a random vector, a role-filler pair is their binding, and a structure is the superposition
//! of its pairs. Order is carried by permutation: position `i` of a sequence is bound to the
//! `i`-th permutation of its item.
//!
//! Everything here is deterministic given the dimension and the names, so a structure has the same
//! vector in every process. That matters more than it sounds: a component that proposes from
//! vectors must be reproducible from its inputs alone, or its proposals cannot be replayed.

use crate::mbat::MBAT;
use crate::memory::derive_seed;
use crate::Hypervector;
use crate::VSA;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;

/// A deterministic role/filler codebook and the encodings built from it.
#[derive(Clone, Debug)]
pub struct Encoder {
    dim: usize,
    roles: HashMap<String, MBAT>,
    fillers: HashMap<String, MBAT>,
}

impl Encoder {
    pub fn new(dim: usize) -> Self {
        assert!(dim > 0, "an encoder needs a positive dimension");
        Encoder { dim, roles: HashMap::new(), fillers: HashMap::new() }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    /// The vector for a role, created deterministically on first use.
    pub fn role(&mut self, name: &str) -> MBAT {
        if let Some(v) = self.roles.get(name) {
            return v.clone();
        }
        let mut rng = StdRng::seed_from_u64(derive_seed(self.dim as u64, &format!("role:{name}")));
        let v = MBAT::generate(self.dim, &mut rng);
        self.roles.insert(name.to_string(), v.clone());
        v
    }

    /// The vector for a filler, created deterministically on first use.
    pub fn filler(&mut self, name: &str) -> MBAT {
        if let Some(v) = self.fillers.get(name) {
            return v.clone();
        }
        let mut rng = StdRng::seed_from_u64(derive_seed(self.dim as u64, &format!("filler:{name}")));
        let v = MBAT::generate(self.dim, &mut rng);
        self.fillers.insert(name.to_string(), v.clone());
        v
    }

    /// One role-filler pair: `role * filler`.
    pub fn pair(&mut self, role: &str, filler: &str) -> Hypervector<MBAT> {
        let r = self.role(role);
        let f = self.filler(filler);
        Hypervector { inner: r.bind(&f) }
    }

    /// A structure: the bundle of its pairs, in any order.
    pub fn structure(&mut self, pairs: &[(&str, &str)]) -> Hypervector<MBAT> {
        assert!(!pairs.is_empty(), "a structure needs at least one pair");
        let parts: Vec<MBAT> = pairs.iter().map(|(r, f)| self.pair(r, f).inner).collect();
        Hypervector { inner: MBAT::bundle_sum(&parts) }
    }

    /// A sequence: position `i` holds the `i`-th permutation of its filler.
    pub fn sequence(&mut self, fillers: &[&str]) -> Hypervector<MBAT> {
        assert!(!fillers.is_empty(), "a sequence needs at least one item");
        let parts: Vec<MBAT> = fillers
            .iter()
            .enumerate()
            .map(|(i, f)| self.filler(f).permute(i))
            .collect();
        Hypervector { inner: MBAT::bundle_sum(&parts) }
    }

    /// Recovers a filler from a structure when the rest of the pairs are known: unbind the known
    /// pairs, then clean up. This is what makes a structure partially readable rather than opaque.
    pub fn read(
        &mut self,
        structure: &Hypervector<MBAT>,
        known: &[(&str, &str)],
    ) -> Hypervector<MBAT> {
        let mut probe = structure.clone();
        for (r, f) in known {
            let pair = self.pair(r, f);
            probe = probe.bind(&pair);
        }
        probe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbinding_recovers_the_right_filler() {
        // Binary bundling is lossy: the unbundled signal is about `1/sqrt(pairs)` of the original,
        // so the test asks that the right filler wins against a distractor, not that the vector is
        // restored. That is the property a cleanup memory needs.
        let mut enc = Encoder::new(8192);
        let s = enc.structure(&[("color", "red"), ("shape", "round")]);
        let role = enc.role("color");
        let recovered = Hypervector { inner: s.inner.bind(&role) };
        let red = Hypervector { inner: enc.filler("red") };
        let blue = Hypervector { inner: enc.filler("blue") };
        assert!(recovered.cosine_similarity(&red) > recovered.cosine_similarity(&blue));
        assert!(recovered.cosine_similarity(&red) > 0.2, "signal is {} ", recovered.cosine_similarity(&red));
    }

    #[test]
    fn different_structures_are_dissimilar() {
        let mut enc = Encoder::new(4096);
        let a = enc.structure(&[("color", "red"), ("shape", "round")]);
        let b = enc.structure(&[("color", "blue"), ("shape", "square")]);
        assert!(a.cosine_similarity(&b).abs() < 0.3);
    }

    #[test]
    fn structures_that_share_a_pair_are_closer() {
        let mut enc = Encoder::new(4096);
        let a = enc.structure(&[("color", "red"), ("shape", "round")]);
        let b = enc.structure(&[("color", "red"), ("shape", "square")]);
        let c = enc.structure(&[("color", "blue"), ("shape", "square")]);
        assert!(a.cosine_similarity(&b) > a.cosine_similarity(&c));
    }

    #[test]
    fn order_is_carried_by_permutation() {
        let mut enc = Encoder::new(4096);
        let ab = enc.sequence(&["a", "b", "c"]);
        let ba = enc.sequence(&["b", "a", "c"]);
        assert!(ab.cosine_similarity(&ba).abs() < 0.3);
    }

    #[test]
    fn encoding_is_deterministic_across_encoders() {
        let mut a = Encoder::new(2048);
        let mut b = Encoder::new(2048);
        let sa = a.structure(&[("x", "1"), ("y", "2")]);
        let sb = b.structure(&[("x", "1"), ("y", "2")]);
        assert!(sa.cosine_similarity(&sb) > 0.999);
    }
}
