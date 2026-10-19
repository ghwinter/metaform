//! Golden test against SME4's dehydrated geometry run
//! Source: SME4's dehydrated corpus, geometry run 20140922134623-18 (read locally, not tracked).
//!
//! Base and target are transcribed in corpus expression order, so base statement `i` is
//! expression `i` of `the-base` and target statement `i` is expression `i` of `the-target`.
//! The corpus's best mapping pairs expression `i` with expression `i`:
//!
//! ```text
//! (:mapping 70 0.11400001 ... (:match Edge-20 Edge-27 0.036000002)
//!                           ... (:match Edge-22 Edge-29 0.036000002)
//!                           ... (:match Edge-21 Edge-28 0.032))
//! ```
//!
//! The corpus file itself therefore scores the mapped entities as
//! Edge-20 = 0.036, Edge-21 = 0.032, Edge-22 = 0.036.  The `0.028` for Edge-22 appears in the
//! file only as the entity match hypothesis `Edge-22 -> Edge-27` (`0.028000003`) and in the
//! second-best mapping (where Edge-22 maps to Edge-27), not in the best mapping.

use metaform_sme::{evidence, map, System, Term};
use std::collections::BTreeSet;

// Base entity order from `(Edge-22 Edge-20 Edge-21)`.
const EDGE_22: u32 = 0;
const EDGE_20: u32 = 1;
const EDGE_21: u32 = 2;

// Target entity order from `(Edge-29 Edge-27 Edge-28)`.
const EDGE_29: u32 = 0;
const EDGE_27: u32 = 1;
const EDGE_28: u32 = 2;

fn unary(system: &mut System, relation: &str, entity: u32) -> u32 {
    system.push(relation, vec![Term::Entity(entity)])
}

fn base_system() -> System {
    let mut s = System::new(3);
    unary(&mut s, "StraightEdge", EDGE_22);
    unary(&mut s, "PerceptualEdge", EDGE_22);
    unary(&mut s, "axisAligned", EDGE_22);
    unary(&mut s, "HorizontalEdge", EDGE_22);
    unary(&mut s, "StraightEdge", EDGE_21);
    unary(&mut s, "PerceptualEdge", EDGE_21);
    unary(&mut s, "ObliqueEdge-Upward", EDGE_21);
    unary(&mut s, "StraightEdge", EDGE_20);
    unary(&mut s, "PerceptualEdge", EDGE_20);
    unary(&mut s, "axisAligned", EDGE_20);
    unary(&mut s, "HorizontalEdge", EDGE_20);
    unary(&mut s, "lengthLong", EDGE_21);
    unary(&mut s, "lengthMedium", EDGE_22);
    unary(&mut s, "lengthMedium", EDGE_20);
    s.push("above", vec![Term::Entity(EDGE_22), Term::Entity(EDGE_21)]);
    s.push_symmetric(
        "elementsConnected",
        vec![Term::Entity(EDGE_22), Term::Entity(EDGE_21)],
    );
    s.push("above", vec![Term::Entity(EDGE_22), Term::Entity(EDGE_20)]);
    s.push_symmetric(
        "edgesParallel",
        vec![Term::Entity(EDGE_22), Term::Entity(EDGE_20)],
    );
    s.push("above", vec![Term::Entity(EDGE_21), Term::Entity(EDGE_20)]);
    s.push_symmetric(
        "elementsConnected",
        vec![Term::Entity(EDGE_21), Term::Entity(EDGE_20)],
    );
    s
}

fn target_system() -> System {
    let mut s = System::new(3);
    unary(&mut s, "StraightEdge", EDGE_29);
    unary(&mut s, "PerceptualEdge", EDGE_29);
    unary(&mut s, "axisAligned", EDGE_29);
    unary(&mut s, "HorizontalEdge", EDGE_29);
    unary(&mut s, "StraightEdge", EDGE_28);
    unary(&mut s, "PerceptualEdge", EDGE_28);
    unary(&mut s, "ObliqueEdge-Upward", EDGE_28);
    unary(&mut s, "StraightEdge", EDGE_27);
    unary(&mut s, "PerceptualEdge", EDGE_27);
    unary(&mut s, "axisAligned", EDGE_27);
    unary(&mut s, "HorizontalEdge", EDGE_27);
    unary(&mut s, "lengthLong", EDGE_28);
    unary(&mut s, "lengthMedium", EDGE_29);
    unary(&mut s, "lengthMedium", EDGE_27);
    s.push("above", vec![Term::Entity(EDGE_29), Term::Entity(EDGE_28)]);
    s.push_symmetric(
        "elementsConnected",
        vec![Term::Entity(EDGE_29), Term::Entity(EDGE_28)],
    );
    s.push("above", vec![Term::Entity(EDGE_29), Term::Entity(EDGE_27)]);
    s.push_symmetric(
        "edgesParallel",
        vec![Term::Entity(EDGE_29), Term::Entity(EDGE_27)],
    );
    s.push("above", vec![Term::Entity(EDGE_28), Term::Entity(EDGE_27)]);
    s.push_symmetric(
        "elementsConnected",
        vec![Term::Entity(EDGE_28), Term::Entity(EDGE_27)],
    );
    s
}

fn assert_best_mapping_covers_every_expression(
    base: &System,
    target: &System,
) -> metaform_sme::Mapping {
    let mappings = map(base, target, 3);
    let m = mappings
        .first()
        .expect("SME must report at least one alignment");
    assert_eq!(
        m.score.matched, 20,
        "best mapping must match all 20 base expressions"
    );
    assert_eq!(
        m.statements.len(),
        20,
        "one correspondence per base expression"
    );
    let matched_base: BTreeSet<u32> = m.statements.iter().map(|(b, _)| *b).collect();
    assert_eq!(
        matched_base,
        (0..20).collect(),
        "every base expression id 0..=19 must appear exactly once"
    );
    let matched_target: BTreeSet<u32> = m.statements.iter().map(|(_, t)| *t).collect();
    assert_eq!(
        matched_target,
        (0..20).collect(),
        "every target expression id 0..=19 must appear exactly once"
    );
    assert_eq!(m.target_of(EDGE_22), Some(EDGE_29), "Edge-22 -> Edge-29");
    assert_eq!(m.target_of(EDGE_20), Some(EDGE_27), "Edge-20 -> Edge-27");
    assert_eq!(m.target_of(EDGE_21), Some(EDGE_28), "Edge-21 -> Edge-28");
    m.clone()
}

/// The corpus's best mapping, reproduced.  This is the pass/fail gate for "comparable to SME4".
#[test]
fn corpus_best_mapping_is_reproduced() {
    let base = base_system();
    let target = target_system();
    let m = assert_best_mapping_covers_every_expression(&base, &target);
    let e = evidence(&base, &target, &m);

    assert!(
        (e.expressions - 0.010).abs() < 1e-9,
        "20 matched expressions x 5e-4 = 0.010, got {}",
        e.expressions
    );
    // Corpus values: 0.036000002, 0.036000002, 0.032 (the last printed without rounding).
    assert!(
        (e.entities[&EDGE_22] - 0.036).abs() < 1e-9,
        "Edge-22 evidence: corpus 0.036, measured {}",
        e.entities[&EDGE_22]
    );
    assert!(
        (e.entities[&EDGE_20] - 0.036).abs() < 1e-9,
        "Edge-20 evidence: corpus 0.036, measured {}",
        e.entities[&EDGE_20]
    );
    assert!(
        (e.entities[&EDGE_21] - 0.032).abs() < 1e-9,
        "Edge-21 evidence: corpus 0.032, measured {}",
        e.entities[&EDGE_21]
    );
    // Corpus total is 0.11400001 (six significant figures; exact arithmetic gives 0.114).
    assert!(
        (e.total - 0.11400001).abs() < 1e-6,
        "total evidence: corpus 0.11400001, measured {}",
        e.total
    );
}

/// The second-best mapping pairs the three edges rotated: Edge-22 -> Edge-27, Edge-20 -> Edge-29,
/// Edge-21 -> Edge-28.  Its three binary matches are twin matches, so the target arguments are
/// reversed relative to the base's; entity evidence must follow the correspondences and credit
/// them anyway.  The mapping matches 17 of the 20 base expressions (the three `above` statements
/// stay out), so its entity scores are 7/7/6 matched expressions x 8 x 5e-4 =
/// 0.028/0.028/0.024 and its total is 17 x 5e-4 expressions + 0.080 entity evidence = 0.0885.
#[test]
fn corpus_second_mapping_credits_twin_matched_entities() {
    let base = base_system();
    let target = target_system();
    let mappings = map(&base, &target, 3);
    let m = mappings.get(1).expect("the geometry run reports a second mapping");

    assert_eq!(m.statements.len(), 17, "second mapping matches 17 base expressions");
    assert_eq!(m.score.matched, 17);
    assert_eq!(m.target_of(EDGE_22), Some(EDGE_27), "Edge-22 -> Edge-27");
    assert_eq!(m.target_of(EDGE_20), Some(EDGE_29), "Edge-20 -> Edge-29");
    assert_eq!(m.target_of(EDGE_21), Some(EDGE_28), "Edge-21 -> Edge-28");

    let e = evidence(&base, &target, m);
    assert!(
        (e.expressions - 0.0085).abs() < 1e-9,
        "17 matched expressions x 5e-4 = 0.0085, got {}",
        e.expressions
    );
    assert!(
        (e.entities[&EDGE_22] - 0.028).abs() < 1e-9,
        "Edge-22 evidence: expected 0.028, measured {}",
        e.entities[&EDGE_22]
    );
    assert!(
        (e.entities[&EDGE_20] - 0.028).abs() < 1e-9,
        "Edge-20 evidence: expected 0.028, measured {}",
        e.entities[&EDGE_20]
    );
    assert!(
        (e.entities[&EDGE_21] - 0.024).abs() < 1e-9,
        "Edge-21 evidence: expected 0.024, measured {}",
        e.entities[&EDGE_21]
    );
    assert!(
        (e.total - 0.0885).abs() < 1e-9,
        "total evidence: expected 0.0885, measured {}",
        e.total
    );
}

/// The three entity scores named in the task brief.  Edge-20 and Edge-21 pass; Edge-22 does not:
/// the matcher measures 0.036 (0.008 above the requested 0.028), and 0.036 is what the corpus's
/// best mapping reports for `Edge-22 -> Edge-29` (`(:match Edge-22 Edge-29 0.036000002)`, line 90).
/// The brief's 0.028 is the corpus's non-mapping candidate `Edge-22 -> Edge-27`
/// (`(:match Edge-22 Edge-27 0.028000003)`, line 94) and the Edge-22 score in the second-best
/// mapping, which pairs Edge-22 with Edge-27.  No single SME4 mapping in this run has
/// (Edge-20 = 0.036, Edge-21 = 0.032, Edge-22 = 0.028): 0.028 implies 7 matched expressions over
/// Edge-22, but full expression coverage gives 9.  Ignored so the discrepancy is recorded without
/// turning a correct corpus match into a red test.
#[test]
#[ignore = "brief's Edge-22 = 0.028 conflicts with the corpus best mapping, which scores Edge-22 -> Edge-29 at 0.036"]
fn brief_expected_scores() {
    let base = base_system();
    let target = target_system();
    let m = assert_best_mapping_covers_every_expression(&base, &target);
    let e = evidence(&base, &target, &m);

    assert!(
        (e.entities[&EDGE_20] - 0.036).abs() < 1e-9,
        "Edge-20 evidence: expected 0.036, measured {}",
        e.entities[&EDGE_20]
    );
    assert!(
        (e.entities[&EDGE_21] - 0.032).abs() < 1e-9,
        "Edge-21 evidence: expected 0.032, measured {}",
        e.entities[&EDGE_21]
    );
    // Measured: 0.036.  Expected here per the brief: 0.028.  Gap: +0.008.
    assert!(
        (e.entities[&EDGE_22] - 0.028).abs() < 1e-9,
        "Edge-22 evidence: expected 0.028, measured {} (gap {:+})",
        e.entities[&EDGE_22],
        e.entities[&EDGE_22] - 0.028
    );
}
