//! Coarsening: merge objects by vector similarity, then let the kernel judge the merge.
//!
//! This is the first production caller of `Translation::quotient`. Before it, the quotient existed,
//! was tested, and was reached only by `metaform translate` when a spec omitted `to` -- that is, by
//! whoever remembered to write such a spec. The operator that would naturally use it (a component
//! saying "these two objects are the same thing") did not exist.
//!
//! The split is the crate's usual one. `coarsen` proposes a map on objects from the vector encoding.
//! The kernel then answers three questions about that map, and none of them is "is the merge good":
//!
//!   - **lossy** -- the map puts two objects in one fibre that an atom separates, and the witness
//!     names the atom and the pair. This is a fact, not a preference.
//!   - **wording_only** -- every definable set survives, and the coarser side says nothing the fine
//!     side could not. The merge is free.
//!   - **genuine** -- the coarser side can say something new. A union of fibres need not be definable
//!     from the fine vocabulary, so a quotient is not always a subset of what it quotients: merging
//!     can *add* a distinction ("I no longer care which of these two").
//!
//! **Why not the SME mapping, which also maps objects.** A structure mapping is one object to one
//! object, so `map` is injective, so `merged()` can never fire and the answer is always "faithful".
//! A judgment that cannot come out negative is not a judgment. The VSA neighbourhood is the right
//! partner because its whole purpose is to say two different things are similar.

use metaform::morphism::{self, Kind, Lost, Theory, Translation};
use metaform::{propose::json_string, Mask, Vocabulary};
use metaform_vsa::retrieval;

/// A merge proposal: which object each one lands on, and which pairs it put together.
#[derive(Clone, Debug)]
pub struct Coarsening {
    /// `map[x]` is the cluster `x` belongs to, numbered `0..clusters`.
    pub map: Vec<usize>,
    /// Every pair the threshold put in one cluster, in ascending order.
    pub merged: Vec<(usize, usize)>,
}

impl Coarsening {
    pub fn clusters(&self) -> usize {
        self.map.iter().copied().max().map_or(0, |m| m + 1)
    }
}

/// Merge objects whose vectors are at least `threshold` similar, transitively.
///
/// Similarity is compared with a tolerance of `1e-9`, because `cosine_similarity(v, v)` is a quotient
/// of square roots and comes out a few ulps below 1 for a vector against itself. Without the
/// tolerance, `--threshold 1.0` would merge nothing at all, and the one threshold a reader can predict
/// would be the one that does not work.
pub fn coarsen(
    objects: usize,
    attributes: &[(String, u64)],
    dim: usize,
    seed: u64,
    threshold: f32,
) -> Coarsening {
    let vectors = retrieval::object_vectors(objects, attributes, dim, seed);

    let mut parent: Vec<usize> = (0..objects).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    let mut merged = Vec::new();
    for a in 0..objects {
        for b in (a + 1)..objects {
            let similarity = vectors[a].cosine_similarity(&vectors[b]);
            if similarity + 1e-9 >= threshold {
                merged.push((a, b));
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                if ra != rb {
                    parent[ra] = rb;
                }
            }
        }
    }

    // Renumber the roots so the map is the coarsest quotient along a total map, and cluster ids are
    // dense: `Translation::quotient` derives the far side's object count from `map.max() + 1`.
    let mut label = vec![usize::MAX; objects];
    let mut next = 0;
    let map = (0..objects)
        .map(|i| {
            let root = find(&mut parent, i);
            if label[root] == usize::MAX {
                label[root] = next;
                next += 1;
            }
            label[root]
        })
        .collect();

    Coarsening { map, merged }
}

/// Coarsen, then let the kernel judge it. One JSON line, fixed field order.
#[allow(clippy::too_many_arguments)]
pub fn judge(
    objects: usize,
    attributes: &[(String, u64)],
    vocabulary: &Vocabulary,
    pos: Mask,
    neg: Mask,
    dim: usize,
    seed: u64,
    threshold: f32,
) -> String {
    let coarsening = coarsen(objects, attributes, dim, seed, threshold);
    let theory = Theory::new(
        objects,
        vocabulary.preds.iter().map(|p| (p.name.clone(), p.ext)).collect(),
    );
    let translation = Translation::quotient(&theory, coarsening.map.clone());
    let analysis = translation.analyse(morphism::MAX_CLASSES);

    let count = |value: Option<usize>| match value {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    };
    let pairs: Vec<String> = coarsening
        .merged
        .iter()
        .map(|(a, b)| format!("[{a},{b}]"))
        .collect();

    let mut out = format!(
        "{{\"kind\":{},\"new_sets\":{},\"sets\":{},\"fibres\":{},\"clusters\":{},\"merged_pairs\":[{}]",
        json_string(analysis.kind.as_str()),
        count(analysis.new_sets),
        count(analysis.sets),
        analysis.fibres,
        coarsening.clusters(),
        pairs.join(",")
    );

    if let Kind::Lossy(lost) = &analysis.kind {
        out.push_str(",\"witness\":");
        out.push_str(&match lost {
            Lost::Merged { atom, a, b } => format!(
                "{{\"kind\":\"merged\",\"atom\":{},\"a\":{a},\"b\":{b}}}",
                json_string(atom)
            ),
            Lost::Unsayable { atom } => {
                format!("{{\"kind\":\"unsayable\",\"atom\":{}}}", json_string(atom))
            }
        });
    }
    // The loss an atom check cannot see: a fibre that holds one positive and one negative. That is
    // the collision which proved the vocabulary insufficient, erased by a merge that kept every
    // definable set.
    out.push_str(",\"collapsed\":");
    out.push_str(&match translation.collapses(pos, neg) {
        Some((a, b)) => format!("[{a},{b}]"),
        None => "null".to_string(),
    });
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use metaform::bit;

    /// Four objects, three attributes; objects 0 and 1 hold exactly the same ones.
    fn attributes() -> Vec<(String, u64)> {
        vec![
            ("x0".to_string(), bit(0) | bit(1)),
            ("x1".to_string(), bit(0) | bit(1) | bit(2)),
            ("x2".to_string(), bit(2) | bit(3)),
        ]
    }

    #[test]
    fn a_threshold_of_one_merges_only_indistinguishable_objects() {
        let c = coarsen(4, &attributes(), 256, 7, 1.0);
        assert_eq!(c.map[0], c.map[1], "0 and 1 hold the same attributes");
        assert!(c.merged.contains(&(0, 1)), "{:?}", c.merged);
    }

    #[test]
    fn the_map_is_dense_because_the_quotient_derives_its_object_count_from_it() {
        let c = coarsen(4, &attributes(), 256, 7, 1.0);
        assert_eq!(c.map.iter().copied().max(), Some(c.clusters() - 1));
    }

    #[test]
    fn a_merge_the_vocabulary_cannot_see_costs_nothing() {
        // `p` puts 0 and 1 on the same side, so merging them keeps every definable set.
        let vocabulary = Vocabulary::with(vec![metaform::Predicate::new("p", bit(0) | bit(1))]);
        let line = judge(4, &attributes(), &vocabulary, 0, 0, 256, 7, 1.0);
        assert!(line.contains("\"kind\":\"wording_only\""), "{line}");
        assert!(line.contains("\"new_sets\":0"), "{line}");
    }

    #[test]
    fn a_merge_the_vocabulary_separates_is_reported_with_its_witness() {
        // `p` holds for 0 and not for 1: the merge erases exactly the distinction `p` draws, and the
        // witness must name a pair, not merely a count.
        let vocabulary = Vocabulary::with(vec![metaform::Predicate::new("p", bit(0))]);
        let line = judge(4, &attributes(), &vocabulary, 0, 0, 256, 7, 1.0);
        assert!(line.contains("\"kind\":\"lossy\""), "{line}");
        assert!(line.contains("\"witness\":{\"kind\":\"merged\",\"atom\":\"p\",\"a\":0,\"b\":1}"), "{line}");
    }

    #[test]
    fn a_merge_that_erases_a_labelled_collision_is_named_even_when_no_atom_sees_it() {
        // `p` agrees with 0 and 1, so no atom is merged; the labels disagree, which is the other loss.
        let vocabulary = Vocabulary::with(vec![metaform::Predicate::new("p", bit(0) | bit(1))]);
        let line = judge(4, &attributes(), &vocabulary, bit(0), bit(1), 256, 7, 1.0);
        assert!(line.contains("\"kind\":\"wording_only\""), "{line}");
        assert!(line.contains("\"collapsed\":[0,1]"), "{line}");
    }
}
