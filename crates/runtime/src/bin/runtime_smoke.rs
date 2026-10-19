//! One validation run: both components propose, the kernel decides.
//!
//! The state is four objects, all labelled, and a vocabulary of one attribute. A positive and a
//! negative object share its signature, so by Theorem 2 the vocabulary is insufficient: no further
//! labelling can repair it, only a new predicate can.
//!
//! Two components then propose one, each in its own way:
//!   - the analogy component maps a relational system onto another, projects the unmatched
//!     statement, and hands the kernel an extension for it;
//!   - the retrieval component ranks attribute combinations by vector similarity to the contrast
//!     between the colliding objects.
//!
//! Neither decides anything. Both proposals go through the same validation, and only adoption
//! changes the state.
//!
//! Usage: runtime_smoke

use metaform::{bit, Mask, Predicate, Status, Vocabulary};
use metaform_runtime::{
    adopt, colliding_pairs, context, label, scenarios, splits, status_with, validate, Candidate,
    Operator, SmeAnalogizer, VsaRetriever,
};

fn name_of(status: Status) -> &'static str {
    match status {
        Status::Insufficient => "INSUFFICIENT",
        Status::Identified => "IDENTIFIED",
        Status::Undetermined => "UNDETERMINED",
    }
}

fn main() {
    // Four objects, all labelled. The target is a&b; the vocabulary holds only a, so the positive
    // object 0 and the negative object 2 share a signature.
    let a: Mask = bit(0) | bit(1) | bit(2);
    let b: Mask = bit(0) | bit(1);
    let c: Mask = bit(3);
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

    println!("state: {} objects, all labelled, vocabulary of {} predicate", ctx.objects, ctx.vocabulary.len());
    println!("colliding pairs: {:?}", colliding_pairs(&ctx));
    println!("status: {}", name_of(status_with(&ctx, &[])));
    println!();

    let systems = scenarios::water_and_heat();
    let operators: Vec<Box<dyn Operator>> = vec![
        Box::new(SmeAnalogizer {
            base: systems.0,
            target: systems.1,
            readings: vec![("liquid".to_string(), b)],
        }),
        Box::new(VsaRetriever { dim: 2048, seed: 7, max_arity: 2 }),
    ];

    let snapshot = ctx.clone();
    let mut accepted: Vec<Candidate> = Vec::new();

    for op in &operators {
        println!("== {} ==", op.name());
        if !op.applicable(&snapshot) {
            let result = op.execute(&snapshot, 4);
            println!("  not applicable: {:?}", result.unresolved);
            println!();
            continue;
        }
        let result = op.execute(&snapshot, 4);
        println!("  status {:?}, cost {:.3}", result.status, result.cost);
        for line in &result.evidence {
            println!("  evidence: {}", line);
        }
        for line in &result.assumptions {
            println!("  assumption: {}", line);
        }
        for line in &result.unresolved {
            println!("  unresolved: {}", line);
        }

        let verdict = validate(&snapshot, &result.candidates);
        println!(
            "  proposals: {} accepted, {} rejected",
            verdict.accepted.len(),
            verdict.rejected.len()
        );
        for c in &verdict.accepted {
            println!(
                "    accepted {}: {} (splits {} colliding pair(s))",
                c.name,
                c.detail,
                splits(&snapshot, c.extension)
            );
            // The controller's policy, not the validator's: a proposal is adopted only if it
            // breaks at least one colliding pair. Validation said it was well-formed; that is a
            // different question from whether it helps.
            if splits(&snapshot, c.extension) > 0 {
                if !accepted.iter().any(|x| x.extension == c.extension) {
                    accepted.push(c.clone());
                }
            } else {
                println!("    kept out of the state: {} breaks no colliding pair", c.name);
            }
        }
        for (c, why) in &verdict.rejected {
            println!("    rejected {}: {}", c.name, why);
        }
        println!();
    }

    // Adoption is a separate step, and it is the only thing that changes the state.
    for candidate in &accepted {
        adopt(&mut ctx, candidate);
        println!(
            "adopted {}: vocabulary {}, status {}",
            candidate.name,
            ctx.vocabulary.len(),
            name_of(status_with(&ctx, &[]))
        );
    }
    println!();
    println!(
        "final: {} predicates, colliding pairs {:?}, status {}",
        ctx.vocabulary.len(),
        colliding_pairs(&ctx),
        name_of(status_with(&ctx, &[]))
    );
}
