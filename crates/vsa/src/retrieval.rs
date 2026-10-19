//! Candidate retrieval by vector similarity.
//!
//! This is the first component of the runtime that is not the exact kernel. It answers one
//! narrow question: given a contrast between a positive and a negative object, can a
//! hyperdimensional encoding rank a candidate predicate that separates them better than chance?
//!
//! The encoding follows Plate's holographic reduced representations as implemented for bipolar
//! vectors (MBAT): an attribute is a random vector, an object is the bundle of its attributes, a
//! conjunction is the binding of its attributes, and the query is the contrast between the two
//! objects. Ranking uses only the vectors; the extension of a candidate is used afterwards, to
//! decide whether the retrieved candidate was right.
//!
//! A retrieved candidate is a *proposal*. It enters the state only after the exact kernel checks
//! it, which is the same boundary the model proposer goes through.

use crate::mbat::MBAT;
use crate::Hypervector;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// One measurement configuration.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub dim: usize,
    pub objects: usize,
    pub attrs: usize,
    pub target: usize,
    pub trials: usize,
    pub seed: u64,
    /// Proposal cost used by the cost law `r/q + rounds`.
    pub proposal_cost: f32,
    /// Query rounds the law adds, measured elsewhere.
    pub query_rounds: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            dim: 2048,
            objects: 12,
            attrs: 8,
            target: 2,
            trials: 200,
            seed: 7,
            proposal_cost: 20.0,
            query_rounds: 7.6,
        }
    }
}

/// What one arm of the experiment scored.
#[derive(Debug, Clone, Copy)]
pub struct Report {
    pub trials: usize,
    pub vsa_hits: usize,
    pub random_hits: usize,
    pub proposal_cost: f32,
    pub query_rounds: f32,
}

impl Report {
    pub fn vsa_hit_rate(&self) -> f32 {
        self.vsa_hits as f32 / self.trials as f32
    }

    pub fn random_hit_rate(&self) -> f32 {
        self.random_hits as f32 / self.trials as f32
    }

    /// The cost law of the paper, in query units.
    pub fn cost_law(&self, hit_rate: f32) -> f32 {
        if hit_rate <= 0.0 {
            f32::INFINITY
        } else {
            self.proposal_cost / hit_rate + self.query_rounds
        }
    }
}

/// A candidate predicate: the attributes it conjoins, and its vector.
struct Candidate {
    attrs: Vec<usize>,
    vector: Hypervector<MBAT>,
}

fn object_vector(attrs_of_object: &[usize], attr_vectors: &[Hypervector<MBAT>]) -> Hypervector<MBAT> {
    if attrs_of_object.is_empty() {
        // No attribute to bundle: fall back to a fixed vector rather than an empty bundle.
        return Hypervector::from_vec(vec![1.0; attr_vectors[0].to_vec().len()]);
    }
    let parts: Vec<Hypervector<MBAT>> =
        attrs_of_object.iter().map(|&a| attr_vectors[a].clone()).collect();
    Hypervector::bundle_sum(&parts)
}

/// Runs the experiment and returns the two arms.
pub fn run(cfg: Config) -> Report {
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let mut vsa_hits = 0;
    let mut random_hits = 0;
    let mut counted = 0;

    for _ in 0..cfg.trials {
        // A random world: every object is a subset of the attributes.
        let mut world = vec![vec![false; cfg.attrs]; cfg.objects];
        for row in world.iter_mut() {
            for cell in row.iter_mut() {
                *cell = rng.gen_bool(0.5);
            }
        }
        // The hidden concept is a conjunction of `target` attributes.
        let mut target_attrs: Vec<usize> = Vec::with_capacity(cfg.target);
        while target_attrs.len() < cfg.target {
            let a = rng.gen_range(0..cfg.attrs);
            if !target_attrs.contains(&a) {
                target_attrs.push(a);
            }
        }
        let labels: Vec<bool> = world
            .iter()
            .map(|row| target_attrs.iter().all(|&a| row[a]))
            .collect();
        let positives: Vec<usize> = (0..cfg.objects).filter(|&i| labels[i]).collect();
        let negatives: Vec<usize> = (0..cfg.objects).filter(|&i| !labels[i]).collect();
        if positives.is_empty() || negatives.is_empty() {
            continue;
        }
        counted += 1;

        // Attribute vectors, object vectors, candidate predicates.
        let attr_vectors = Hypervector::<MBAT>::generate_many_with(cfg.dim, cfg.attrs, &mut rng);
        let object_vectors: Vec<Hypervector<MBAT>> = (0..cfg.objects)
            .map(|i| {
                let present: Vec<usize> = (0..cfg.attrs).filter(|&a| world[i][a]).collect();
                object_vector(&present, &attr_vectors)
            })
            .collect();

        let mut candidates: Vec<Candidate> = Vec::new();
        for a in 0..cfg.attrs {
            candidates.push(Candidate { attrs: vec![a], vector: attr_vectors[a].clone() });
        }
        for a in 0..cfg.attrs {
            for b in (a + 1)..cfg.attrs {
                candidates.push(Candidate {
                    attrs: vec![a, b],
                    vector: attr_vectors[a].bind(&attr_vectors[b]),
                });
            }
        }

        let p = positives[rng.gen_range(0..positives.len())];
        let n = negatives[rng.gen_range(0..negatives.len())];

        // The query is the contrast between the two objects: keep what the negative has and the
        // positive does not.
        let query = Hypervector::bundle_sum(&[object_vectors[p].negate(), object_vectors[n].clone()]);

        let splits = |c: &Candidate| {
            let holds = |obj: usize| c.attrs.iter().all(|&a| world[obj][a]);
            holds(p) != holds(n)
        };

        let best = candidates
            .iter()
            .enumerate()
            .max_by(|(_, x), (_, y)| {
                x.vector
                    .cosine_similarity(&query)
                    .partial_cmp(&y.vector.cosine_similarity(&query))
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap();
        if splits(&candidates[best]) {
            vsa_hits += 1;
        }

        let pick = rng.gen_range(0..candidates.len());
        if splits(&candidates[pick]) {
            random_hits += 1;
        }
    }

    Report {
        trials: counted,
        vsa_hits,
        random_hits,
        proposal_cost: cfg.proposal_cost,
        query_rounds: cfg.query_rounds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_covers_every_conjunction_up_to_the_arity() {
        let attrs: Vec<(String, u64)> = ["a", "b", "c", "d"]
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), 1u64 << i))
            .collect();
        let one = rank_for_pair(4, &attrs, 0, 3, 128, 7, 1);
        let two = rank_for_pair(4, &attrs, 0, 3, 128, 7, 2);
        let three = rank_for_pair(4, &attrs, 0, 3, 128, 7, 3);
        assert_eq!(one.len(), 4);
        assert_eq!(two.len(), 4 + 6);
        assert_eq!(three.len(), 4 + 6 + 4);
        let names: std::collections::BTreeSet<&String> = three.iter().map(|r| &r.name).collect();
        assert_eq!(names.len(), three.len(), "names must be distinct");
    }

    fn small() -> Config {
        Config { dim: 2048, objects: 12, attrs: 8, target: 2, trials: 60, seed: 7, ..Default::default() }
    }

    #[test]
    fn retrieval_beats_a_random_candidate() {
        let r = run(small());
        assert!(
            r.vsa_hit_rate() > r.random_hit_rate(),
            "vsa {} vs random {}",
            r.vsa_hit_rate(),
            r.random_hit_rate()
        );
    }

    #[test]
    fn retrieval_is_deterministic_for_a_seed() {
        let a = run(small());
        let b = run(small());
        assert_eq!(a.vsa_hits, b.vsa_hits);
        assert_eq!(a.random_hits, b.random_hits);
    }

    #[test]
    fn the_cost_law_falls_when_the_hit_rate_rises() {
        let r = run(small());
        assert!(r.cost_law(0.8) < r.cost_law(0.5));
    }
}

/// One ranked candidate, with the vector similarity kept separate from its extension.
#[derive(Clone, Debug)]
pub struct Ranked {
    pub name: String,
    pub extension: u64,
    pub similarity: f32,
}

/// Ranks single and paired attributes by similarity to the contrast between two objects, for one
/// concrete pair. This is what a caller inside the runtime uses; the experiment above is the
/// offline measurement of the same code path.
pub fn rank_for_pair(
    objects: usize,
    attributes: &[(String, u64)],
    positive: usize,
    negative: usize,
    dim: usize,
    seed: u64,
    max_arity: usize,
) -> Vec<Ranked> {
    use crate::mbat::MBAT;
    let mut rng = StdRng::seed_from_u64(seed);
    let attr_vectors: Vec<Hypervector<MBAT>> =
        Hypervector::<MBAT>::generate_many_with(dim, attributes.len(), &mut rng);

    // An object is the bundle of the attributes that hold for it.
    let mut object_vectors: Vec<Hypervector<MBAT>> = Vec::with_capacity(objects);
    for i in 0..objects {
        let present: Vec<Hypervector<MBAT>> = attributes
            .iter()
            .enumerate()
            .filter(|(_, (_, ext))| (ext >> i) & 1 == 1)
            .map(|(a, _)| attr_vectors[a].clone())
            .collect();
        object_vectors.push(if present.is_empty() {
            Hypervector::from_vec(vec![1.0; dim])
        } else {
            Hypervector::bundle_sum(&present)
        });
    }

    let query = Hypervector::bundle_sum(&[
        object_vectors[positive].negate(),
        object_vectors[negative].clone(),
    ]);

    // Every conjunction of at most `max_arity` attributes is a candidate.
    let mut ranked: Vec<Ranked> = Vec::new();
    let n = attributes.len();
    let arity = max_arity.clamp(1, n);
    for size in 1..=arity {
        let mut idx: Vec<usize> = (0..size).collect();
        loop {
            let mut name = attributes[idx[0]].0.clone();
            let mut extension = attributes[idx[0]].1;
            let mut vector = attr_vectors[idx[0]].clone();
            for &i in &idx[1..] {
                name.push('&');
                name.push_str(&attributes[i].0);
                extension &= attributes[i].1;
                vector = vector.bind(&attr_vectors[i]);
            }
            ranked.push(Ranked { name, extension, similarity: vector.cosine_similarity(&query) });
            // next combination
            let mut i = size;
            loop {
                if i == 0 {
                    break;
                }
                i -= 1;
                if idx[i] != i + n - size {
                    idx[i] += 1;
                    for j in (i + 1)..size {
                        idx[j] = idx[j - 1] + 1;
                    }
                    break;
                }
                if i == 0 {
                    idx.clear();
                    break;
                }
            }
            if idx.is_empty() {
                break;
            }
        }
    }
    ranked.sort_by(|x, y| y.similarity.partial_cmp(&x.similarity).unwrap());
    ranked

}
