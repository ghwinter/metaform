//! The controller with the real components: VSA retrieval and SME analogy propose, the kernel
//! validates, and the state leaves insufficiency.
//!
//! The kernel's own tests use an in-test operator, so this is where the composition is exercised.

use metaform::{bit, Predicate, Status, Vocabulary};
use metaform_components::{scenarios, SmeAnalogizer, VsaRetriever};
use metaform_runtime::run::run;
use metaform_runtime::{colliding_pairs, context, label, status_with, Context, Operator};

fn collision() -> Context {
    // Four objects; the vocabulary holds a, so 0/1 (positive) and 2 (negative) collide.
    let a = bit(0) | bit(1) | bit(2);
    let b = bit(0) | bit(1);
    let c = bit(3);
    let mut ctx = context(
        4,
        Vocabulary::with(vec![Predicate::new("a", a)]),
        vec![
            ("a".to_string(), a),
            ("b".to_string(), b),
            ("c".to_string(), c),
        ],
    );
    label(&mut ctx, 0, true);
    label(&mut ctx, 1, true);
    label(&mut ctx, 2, false);
    label(&mut ctx, 3, false);
    ctx
}

#[test]
fn the_controller_with_the_real_components_stops_being_insufficient() {
    let mut ctx = collision();
    assert_eq!(status_with(&ctx, &[]), Status::Insufficient);

    let systems = scenarios::water_and_heat();
    let operators: Vec<Box<dyn Operator>> = vec![
        Box::new(SmeAnalogizer {
            base: systems.0,
            target: systems.1,
            readings: vec![("liquid".to_string(), bit(0) | bit(1))],
        }),
        Box::new(VsaRetriever { dim: 2048, seed: 7, max_arity: 2 }),
    ];

    let steps = run(&mut ctx, &operators, 4, 4);
    assert!(!steps.is_empty(), "the controller should take at least one step");
    assert!(steps[0].adopted().is_some(), "the first step should adopt an offer");
    assert_ne!(status_with(&ctx, &[]), Status::Insufficient);
    assert!(colliding_pairs(&ctx).is_empty(), "the collision should be gone");
}
