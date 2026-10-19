//! MBAT: bipolar vectors in {-1, +1}, binding by element-wise product.
//!
//! Adapted from the `hypervector` crate (<https://github.com/rishikanthc/hypervector>),
//! MIT OR Apache-2.0, Copyright (c) 2025 Rishikanth Chandrasekaran. See `NOTICE`.
//! The `serde` derives were removed and `negate` was added; the semantics are unchanged.

use crate::{TieBreaker, VSA};
use rand::Rng;

/// Matrix Binding of Additive Terms, stored as a bipolar `Vec<i8>`.
#[derive(Clone, Debug, PartialEq)]
pub struct MBAT {
    pub data: Vec<i8>,
}

impl VSA for MBAT {
    type Elem = i8;

    fn generate(dim: usize, rng: &mut impl Rng) -> Self {
        let data = (0..dim).map(|_| if rng.gen_bool(0.5) { 1 } else { -1 }).collect();
        MBAT { data }
    }

    fn bundle(&self, other: &Self, tie_breaker: TieBreaker, rng: &mut impl Rng) -> Self {
        assert_eq!(self.data.len(), other.data.len(), "dimension mismatch in bundling");
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| match a + b {
                s if s > 0 => 1,
                s if s < 0 => -1,
                _ => match tie_breaker {
                    TieBreaker::AlwaysPositive => 1,
                    TieBreaker::AlwaysNegative => -1,
                    TieBreaker::Random => {
                        if rng.gen_bool(0.5) {
                            1
                        } else {
                            -1
                        }
                    }
                },
            })
            .collect();
        MBAT { data }
    }

    fn bind(&self, other: &Self) -> Self {
        assert_eq!(self.data.len(), other.data.len(), "dimension mismatch in binding");
        MBAT { data: self.data.iter().zip(other.data.iter()).map(|(&a, &b)| a * b).collect() }
    }

    fn negate(&self) -> Self {
        MBAT { data: self.data.iter().map(|&x| -x).collect() }
    }

    fn cosine_similarity(&self, other: &Self) -> f32 {
        assert_eq!(self.data.len(), other.data.len(), "dimension mismatch in similarity");
        let dot: i32 = self.data.iter().zip(other.data.iter()).map(|(&a, &b)| (a as i32) * (b as i32)).sum();
        dot as f32 / self.data.len() as f32
    }

    fn hamming_distance(&self, other: &Self) -> f32 {
        assert_eq!(self.data.len(), other.data.len(), "dimension mismatch in distance");
        let mismatches = self.data.iter().zip(other.data.iter()).filter(|(a, b)| a != b).count();
        mismatches as f32 / self.data.len() as f32
    }

    fn to_vec(&self) -> Vec<f32> {
        self.data.iter().map(|&x| x as f32).collect()
    }

    fn from_vec(v: Vec<f32>) -> Self {
        MBAT { data: v.into_iter().map(|x| if x >= 0.0 { 1 } else { -1 }).collect() }
    }
}

impl MBAT {
    /// Bundles many vectors by summing them first and taking the sign once.
    ///
    /// The upstream code folds pairwise, which forces every intermediate result back to +-1 and
    /// biases long bundles toward the tie-breaker. Summing first is the standard MAP/BSC
    /// superposition and is what a query over several attributes needs. Ties are broken
    /// deterministically and without bias: even positions take +1, odd positions take -1.
    pub fn bundle_sum(vectors: &[MBAT]) -> MBAT {
        assert!(!vectors.is_empty(), "cannot bundle an empty slice");
        let dim = vectors[0].data.len();
        let mut sums = vec![0i32; dim];
        for v in vectors {
            assert_eq!(v.data.len(), dim, "dimension mismatch in bundle_sum");
            for (s, &x) in sums.iter_mut().zip(v.data.iter()) {
                *s += x as i32;
            }
        }
        let data = sums
            .into_iter()
            .enumerate()
            .map(|(i, s)| match s {
                0 => {
                    if i % 2 == 0 {
                        1
                    } else {
                        -1
                    }
                }
                s if s > 0 => 1,
                _ => -1,
            })
            .collect();
        MBAT { data }
    }
}