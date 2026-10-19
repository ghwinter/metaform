//! Runs the structure mapper on a small analogy and prints the alignment and the inferences.
//!
//! Usage: sme_demo

use metaform_sme::{candidate_inferences, map, System, Term};

/// Water flows from a full pipe into an empty beaker: the greater the pipe level compared with the
/// beaker level, the greater the flow. Entities: 0 pipe, 1 beaker.
fn water_flow() -> System {
    let mut s = System::new(2);
    let greater = s.push("greater", vec![Term::Entity(0), Term::Entity(1)]);
    let flow = s.push("flow", vec![Term::Entity(0), Term::Entity(1)]);
    s.push("cause", vec![Term::Statement(greater), Term::Statement(flow)]);
    // One statement the target has no counterpart for: the pipe holds liquid.
    s.push("liquid", vec![Term::Entity(0)]);
    s
}

/// Heat flows from warm coffee into a cold bar. Entities: 0 coffee, 1 bar.
fn heat_flow() -> System {
    let mut s = System::new(2);
    let greater = s.push("greater", vec![Term::Entity(0), Term::Entity(1)]);
    let flow = s.push("flow", vec![Term::Entity(0), Term::Entity(1)]);
    s.push("cause", vec![Term::Statement(greater), Term::Statement(flow)]);
    s
}

fn main() {
    let base = water_flow();
    let target = heat_flow();
    let mappings = map(&base, &target, 3);
    println!("alignments: {}", mappings.len());
    for (i, m) in mappings.iter().enumerate() {
        println!(
            "  [{}] matched={} systematicity={} components={}",
            i, m.score.matched, m.score.systematicity, m.score.components
        );
        for (b, t) in &m.entities {
            println!("      entity {} -> {}", b, t);
        }
        for (b, t) in &m.statements {
            println!(
                "      {} -> {}",
                base.statements[*b as usize].relation, target.statements[*t as usize].relation
            );
        }
    }
    if let Some(best) = mappings.first() {
        let inferences = candidate_inferences(&base, &target, best);
        println!("candidate inferences: {}", inferences.len());
        for s in inferences {
            let args: Vec<String> = s
                .args
                .iter()
                .map(|a| match a {
                    Term::Entity(e) => format!("entity {}", e),
                    Term::Statement(x) => format!("statement {}", x),
                })
                .collect();
            println!("      {} [{}]", s.relation, args.join(", "));
            println!("      (a proposal, not a conclusion: the kernel has to check it)");
        }
    }
}
