//! Item and associative memory, the two memories a hyperdimensional computer needs.
//!
//! An *item memory* is a codebook: named random vectors, plus a cleanup operation that maps a
//! noisy vector back to the nearest stored item. An *associative memory* stores a set of
//! key-value pairs as the superposition of their bindings, and recalls a value from a key by
//! unbinding and cleaning up. Both are the standard construction (Kanerva 2009); the point of
//! keeping them here, rather than in a caller, is that capacity and noise behaviour become
//! measurable properties of the component instead of folklore.

use crate::mbat::MBAT;
use crate::Hypervector;
use crate::VSA;
use rand::rngs::StdRng;
use rand::SeedableRng;

/// A named codebook of random vectors.
#[derive(Clone, Debug)]
pub struct ItemMemory {
    dim: usize,
    names: Vec<String>,
    vectors: Vec<MBAT>,
}

impl ItemMemory {
    pub fn new(dim: usize) -> Self {
        assert!(dim > 0, "an item memory needs a positive dimension");
        ItemMemory { dim, names: Vec::new(), vectors: Vec::new() }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Adds a named random vector, or returns the index of an existing name.
    pub fn add(&mut self, name: &str) -> usize {
        if let Some(i) = self.names.iter().position(|n| n == name) {
            return i;
        }
        let v = Hypervector::<MBAT>::generate_with(self.dim, &mut StdRng::seed_from_u64(
            derive_seed(self.dim as u64, name),
        ));
        self.names.push(name.to_string());
        self.vectors.push(v.inner);
        self.names.len() - 1
    }

    /// The vector stored under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&MBAT> {
        self.names.iter().position(|n| n == name).map(|i| &self.vectors[i])
    }

    /// The nearest stored item and its similarity. Ties keep the earlier index, so the result is
    /// deterministic.
    pub fn cleanup(&self, query: &Hypervector<MBAT>) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, v) in self.vectors.iter().enumerate() {
            let s = v.cosine_similarity(&query.inner);
            match best {
                Some((_, bs)) if s <= bs => {}
                _ => best = Some((i, s)),
            }
        }
        best
    }

    /// The `k` nearest items, best first. Ties keep the earlier index.
    pub fn cleanup_k(&self, query: &Hypervector<MBAT>, k: usize) -> Vec<(usize, f32)> {
        let mut scored: Vec<(usize, f32)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v.cosine_similarity(&query.inner)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
        scored.truncate(k);
        scored
    }
}

/// A superposition of bound key-value pairs.
#[derive(Clone, Debug)]
pub struct AssociativeMemory {
    dim: usize,
    pairs: usize,
    trace: Vec<i32>,
}

impl AssociativeMemory {
    pub fn new(dim: usize) -> Self {
        assert!(dim > 0, "an associative memory needs a positive dimension");
        AssociativeMemory { dim, pairs: 0, trace: vec![0; dim] }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.pairs
    }

    pub fn is_empty(&self) -> bool {
        self.pairs == 0
    }

    /// Stores `key * value` in the trace. The key should come from a deterministic source (an
    /// item memory or an encoder), otherwise it cannot be recalled.
    pub fn store(&mut self, key: &MBAT, value: &MBAT) {
        assert_eq!(key.data.len(), self.dim, "key dimension mismatch");
        assert_eq!(value.data.len(), self.dim, "value dimension mismatch");
        for (t, (&k, &v)) in self.trace.iter_mut().zip(key.data.iter().zip(value.data.iter())) {
            *t += (k as i32) * (v as i32);
        }
        self.pairs += 1;
    }

    /// The trace as a bundle: the sign of each accumulated sum.
    pub fn trace(&self) -> Hypervector<MBAT> {
        assert!(self.pairs > 0, "an empty associative memory has no trace");
        let data = self
            .trace
            .iter()
            .enumerate()
            .map(|(i, &s)| match s {
                0 => if i % 2 == 0 { 1 } else { -1 },
                s if s > 0 => 1,
                _ => -1,
            })
            .collect();
        Hypervector { inner: MBAT { data } }
    }

    /// Recalls the value stored under `key` by unbinding the trace and cleaning up against the
    /// item memory. For a bipolar memory unbinding is binding again, so this is one pass.
    pub fn recall(
        &self,
        key: &MBAT,
        values: &ItemMemory,
    ) -> Option<(usize, f32)> {
        if self.pairs == 0 {
            return None;
        }
        let probe = Hypervector::<MBAT> { inner: key.clone() }.bind(&self.trace());
        values.cleanup(&probe)
    }

    /// Recalls without re-quantising the trace: the unbinding leaves an integer accumulator, and
    /// the score against a stored value is its normalised inner product.
    ///
    /// The quantised trace throws away the magnitude that distinguishes "one pair says this" from
    /// "the sum is near zero", which is exactly the information that decides a recall near
    /// capacity. Keeping the integer accumulator in the hot path is the difference between a
    /// usable memory and a demonstration.
    pub fn recall_raw(&self, key: &MBAT, values: &ItemMemory) -> Option<(usize, f32)> {
        if self.pairs == 0 {
            return None;
        }
        let dim = self.dim as f32;
        let mut best: Option<(usize, f32)> = None;
        for (i, v) in values.vectors.iter().enumerate() {
            let mut dot: f64 = 0.0;
            for (t, (&k, &x)) in self.trace.iter().zip(key.data.iter().zip(v.data.iter())) {
                dot += (*t as f64) * (k as f64) * (x as f64);
            }
            let s = (dot / (dim as f64 * dim as f64)) as f32;
            match best {
                Some((_, bs)) if s <= bs => {}
                _ => best = Some((i, s)),
            }
        }
        best
    }
}

/// A 64-bit seed derived from a name, so a codebook is reproducible without storing it.
pub fn derive_seed(dim: u64, name: &str) -> u64 {
    // FNV-1a: stable across runs and platforms, unlike the standard hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^ (dim << 32) ^ 0x9e37_79b9_7f4a_7c15
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn cleanup_recovers_a_clean_vector() {
        let mut mem = ItemMemory::new(2048);
        for i in 0..100 {
            mem.add(&format!("item-{i}"));
        }
        let q = Hypervector::<MBAT>::from_vec(mem.get("item-42").unwrap().to_vec());
        let (idx, sim) = mem.cleanup(&q).unwrap();
        assert_eq!(mem.names()[idx], "item-42");
        assert!(sim > 0.99);
    }

    #[test]
    fn cleanup_survives_noise() {
        let mut mem = ItemMemory::new(4096);
        for i in 0..200 {
            mem.add(&format!("item-{i}"));
        }
        let mut rng = StdRng::seed_from_u64(11);
        let base = mem.get("item-7").unwrap().clone();
        let noisy: Vec<i8> = base
            .data
            .iter()
            .map(|&x| if rng.gen_bool(0.1) { -x } else { x })
            .collect();
        let (idx, _) = mem.cleanup(&Hypervector { inner: MBAT { data: noisy } }).unwrap();
        assert_eq!(mem.names()[idx], "item-7");
    }

    #[test]
    fn associative_memory_recalls_a_value() {
        // 32 pairs at 8192 bits: the recall signal is about 1/sqrt(32), so the test checks the
        // ranking, not the magnitude.
        let mut keys = ItemMemory::new(8192);
        let mut values = ItemMemory::new(8192);
        let mut assoc = AssociativeMemory::new(8192);
        for i in 0..32 {
            let k = keys.get(&format!("k{i}")).cloned().unwrap_or_else(|| {
                keys.add(&format!("k{i}"));
                keys.get(&format!("k{i}")).unwrap().clone()
            });
            let v = values.get(&format!("v{i}")).cloned().unwrap_or_else(|| {
                values.add(&format!("v{i}"));
                values.get(&format!("v{i}")).unwrap().clone()
            });
            assoc.store(&k, &v);
        }
        let key = keys.get("k17").cloned().unwrap();
        let (idx, sim) = assoc.recall(&key, &values).unwrap();
        assert_eq!(values.names()[idx], "v17");
        assert!(sim > 0.05, "recall similarity {sim} is too low");
    }

    #[test]
    fn derived_seeds_are_stable() {
        assert_eq!(derive_seed(2048, "role:color"), derive_seed(2048, "role:color"));
        assert_ne!(derive_seed(2048, "role:color"), derive_seed(2048, "role:size"));
        assert_ne!(derive_seed(1024, "role:color"), derive_seed(2048, "role:color"));
    }

    #[test]
    fn associative_recall_holds_at_capacity() {
        // The capacity claim, as a test: 128 pairs, 4096 bits, a 10% corrupted key. This is the
        // row of the benchmark table the library promises; if the encoding regresses, it goes red.
        let dim = 4096;
        let n = 128;
        let mut keys = ItemMemory::new(dim);
        let mut values = ItemMemory::new(dim);
        let mut assoc = AssociativeMemory::new(dim);
        for i in 0..n {
            keys.add(&format!("k{i}"));
            values.add(&format!("v{i}"));
        }
        for i in 0..n {
            let k = keys.get(&format!("k{i}")).unwrap().clone();
            let v = values.get(&format!("v{i}")).unwrap().clone();
            assoc.store(&k, &v);
        }
        let mut rng = StdRng::seed_from_u64(3);
        let mut hits = 0;
        for i in 0..16 {
            let key = keys.get(&format!("k{}", i * 7)).unwrap().clone();
            let noisy: Vec<i8> = key.data.iter().map(|&x| if rng.gen_bool(0.1) { -x } else { x }).collect();
            let (idx, _) = assoc.recall_raw(&MBAT { data: noisy }, &values).unwrap();
            if values.names()[idx] == format!("v{}", i * 7) {
                hits += 1;
            }
        }
        assert!(hits >= 15, "recall at capacity dropped: {hits}/16");
    }
}
