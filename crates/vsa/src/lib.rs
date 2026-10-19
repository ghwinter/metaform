//! Vector symbolic architecture for metaform.
//!
//! Adapted from the `hypervector` crate (<https://github.com/rishikanthc/hypervector>),
//! MIT OR Apache-2.0, Copyright (c) 2025 Rishikanth Chandrasekaran. See `NOTICE`.
//!
//! What changed relative to the original:
//!   - `once_cell` removed; the global RNG uses `std::sync::OnceLock`;
//!   - `serde` derives removed, so the component has a single dependency (`rand`);
//!   - `encoder`, `ssp` and `fhrr` are not carried over yet: the first component needs
//!     bipolar MBAT vectors, binding, bundling, similarity and negation;
//!   - `negate` added to the `VSA` trait, because a query here is a contrast between two
//!     objects rather than a single vector.
//!
//! Everything else (the `VSA` trait shape, MBAT semantics, tie-breaking) is the original.

use rand::{rngs::StdRng, Rng, SeedableRng};
use std::ops::{Add, Mul};
use std::sync::{Mutex, OnceLock};

/// Process-wide RNG for operations that do not take one.
fn global_rng() -> &'static Mutex<StdRng> {
    static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();
    RNG.get_or_init(|| Mutex::new(StdRng::from_entropy()))
}

/// Seeds the global RNG, which makes every operation that uses it reproducible.
pub fn set_global_seed(seed: u64) {
    let mut rng = global_rng().lock().unwrap();
    *rng = StdRng::seed_from_u64(seed);
}

/// How bundling resolves an element-wise tie (sum equal to zero).
#[derive(Debug, Clone, Copy)]
pub enum TieBreaker {
    AlwaysPositive,
    AlwaysNegative,
    Random,
}

/// A vector symbolic architecture.
pub trait VSA: Sized + Clone {
    /// The type of one element.
    type Elem: Copy + std::fmt::Debug + PartialEq + Into<f32>;

    /// A random vector of the given dimension.
    fn generate(dim: usize, rng: &mut impl Rng) -> Self;

    /// Superposition: keeps what the operands have in common.
    fn bundle(&self, other: &Self, tie_breaker: TieBreaker, rng: &mut impl Rng) -> Self;

    /// Binding: associates two vectors, and is (nearly) its own inverse.
    fn bind(&self, other: &Self) -> Self;

    /// Element-wise negation, used to build a contrast between two vectors.
    fn negate(&self) -> Self;

    fn cosine_similarity(&self, other: &Self) -> f32;

    fn hamming_distance(&self, other: &Self) -> f32;

    fn to_vec(&self) -> Vec<f32>;

    fn from_vec(v: Vec<f32>) -> Self;

    /// Bundles a slice, folding from the left. Panics on an empty slice.
    fn bundle_many(vectors: &[Self], tie_breaker: TieBreaker, rng: &mut impl Rng) -> Self {
        assert!(!vectors.is_empty(), "cannot bundle an empty slice");
        let mut result = vectors[0].clone();
        for v in &vectors[1..] {
            result = result.bundle(v, tie_breaker, rng);
        }
        result
    }

    /// Binds a slice, folding from the left. Panics on an empty slice.
    fn bind_many(vectors: &[Self]) -> Self {
        assert!(!vectors.is_empty(), "cannot bind an empty slice");
        let mut result = vectors[0].clone();
        for v in &vectors[1..] {
            result = result.bind(v);
        }
        result
    }
}

/// A hypervector over some architecture.
#[derive(Clone, Debug, PartialEq)]
pub struct Hypervector<V: VSA> {
    pub inner: V,
}

impl<V: VSA> Hypervector<V> {
    /// A random vector from the global RNG.
    pub fn generate(dim: usize) -> Self {
        let mut rng = global_rng().lock().unwrap();
        Self { inner: V::generate(dim, &mut *rng) }
    }

    /// A random vector from a caller-supplied RNG, so a component can be reproducible without
    /// touching process-wide state.
    pub fn generate_with(dim: usize, rng: &mut impl Rng) -> Self {
        Self { inner: V::generate(dim, rng) }
    }

    /// `count` random vectors from a caller-supplied RNG.
    pub fn generate_many_with(dim: usize, count: usize, rng: &mut impl Rng) -> Vec<Self> {
        (0..count).map(|_| Self { inner: V::generate(dim, rng) }).collect()
    }

    /// `count` random vectors of the given dimension.
    pub fn generate_many(dim: usize, count: usize) -> Vec<Self> {
        let mut rng = global_rng().lock().unwrap();
        (0..count).map(|_| Self { inner: V::generate(dim, &mut *rng) }).collect()
    }

    pub fn bundle(&self, other: &Self, tie_breaker: TieBreaker) -> Self {
        let mut rng = global_rng().lock().unwrap();
        Self { inner: self.inner.bundle(&other.inner, tie_breaker, &mut *rng) }
    }

    pub fn bind(&self, other: &Self) -> Self {
        Self { inner: self.inner.bind(&other.inner) }
    }

    /// The element-wise negation of this vector.
    pub fn negate(&self) -> Self {
        Self { inner: self.inner.negate() }
    }

    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        self.inner.cosine_similarity(&other.inner)
    }

    pub fn hamming_distance(&self, other: &Self) -> f32 {
        self.inner.hamming_distance(&other.inner)
    }

    pub fn to_vec(&self) -> Vec<f32> {
        self.inner.to_vec()
    }

    pub fn from_vec(v: Vec<f32>) -> Self {
        Self { inner: V::from_vec(v) }
    }

    pub fn bundle_many(vectors: &[Self], tie_breaker: TieBreaker) -> Self {
        let mut rng = global_rng().lock().unwrap();
        let inners: Vec<V> = vectors.iter().map(|hv| hv.inner.clone()).collect();
        Self { inner: V::bundle_many(&inners, tie_breaker, &mut *rng) }
    }

    pub fn bind_many(vectors: &[Self]) -> Self {
        let inners: Vec<V> = vectors.iter().map(|hv| hv.inner.clone()).collect();
        Self { inner: V::bind_many(&inners) }
    }
}

impl<V: VSA> Add for Hypervector<V> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut rng = global_rng().lock().unwrap();
        Self { inner: self.inner.bundle(&rhs.inner, TieBreaker::Random, &mut *rng) }
    }
}

impl<V: VSA> Mul for Hypervector<V> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self { inner: self.inner.bind(&rhs.inner) }
    }
}

use crate::mbat::MBAT;

impl Hypervector<MBAT> {
    /// Bundles many vectors by summing first and taking the sign once: the query over several
    /// attributes needs this, and it is unbiased, unlike folding pairwise with a tie-breaker.
    pub fn bundle_sum(vectors: &[Self]) -> Self {
        let inners: Vec<MBAT> = vectors.iter().map(|v| v.inner.clone()).collect();
        Self { inner: MBAT::bundle_sum(&inners) }
    }
}

pub mod mbat;
pub mod retrieval;

#[cfg(test)]
mod tests {
    use super::mbat::MBAT;
    use super::*;

    type HV = Hypervector<MBAT>;

    #[test]
    fn random_vectors_are_nearly_orthogonal() {
        set_global_seed(7);
        let a = HV::generate(10_000);
        let b = HV::generate(10_000);
        assert!(a.cosine_similarity(&b).abs() < 0.1);
    }

    #[test]
    fn bundling_keeps_half_the_similarity() {
        set_global_seed(7);
        let a = HV::generate(10_000);
        let b = HV::generate(10_000);
        let bundled = a.bundle(&b, TieBreaker::Random);
        assert!((bundled.cosine_similarity(&a) - 0.5).abs() < 0.1);
        assert!((bundled.cosine_similarity(&b) - 0.5).abs() < 0.1);
    }

    #[test]
    fn binding_is_orthogonal_to_its_operands() {
        set_global_seed(7);
        let a = HV::generate(10_000);
        let b = HV::generate(10_000);
        let bound = a.bind(&b);
        assert!(a.cosine_similarity(&bound).abs() < 0.1);
        assert!(b.cosine_similarity(&bound).abs() < 0.1);
    }

    #[test]
    fn binding_unbinds() {
        set_global_seed(7);
        let a = HV::generate(10_000);
        let b = HV::generate(10_000);
        let recovered = a.bind(&b).bind(&b);
        assert!(recovered.cosine_similarity(&a) > 0.99);
    }

    #[test]
    fn negate_flips_every_element() {
        let v = HV::from_vec(vec![1.0, -1.0, 1.0]);
        assert_eq!(v.negate().to_vec(), vec![-1.0, 1.0, -1.0]);
    }
}
