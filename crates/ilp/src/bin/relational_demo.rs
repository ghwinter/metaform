//! The relational necessity criterion, on two small domains.
//!
//! `trees()` gives four objects: red 0 and 1, a link 0 -> green 2, and a link 1 -> blue 3. Unary
//! predicates cannot separate 0 from 1, and no amount of unary vocabulary will: they are the same
//! in every unary respect. The pattern `link(V0,V1), green(V1)` separates them, so the missing word
//! is a relation, not an attribute.
//!
//! `bisimilar()` gives two objects that no tree-shaped pattern can separate. The program prints
//! that verdict rather than inventing a pattern, because "no separating pattern in this language"
//! is the answer the kernel needs.
//!
//! Usage: relational_demo [--out <response.json>]

use metaform_ilp::{extension, separating_pattern, Background, Signature};

fn sig(names: &[(&str, usize)]) -> Vec<Signature> {
    names.iter().map(|(n, a)| Signature { name: n.to_string(), arity: *a }).collect()
}

fn trees() -> Background {
    let mut bg = Background::new(sig(&[("red", 1), ("blue", 1), ("green", 1), ("link", 2)]));
    bg.fact("red", &[0]);
    bg.fact("red", &[1]);
    bg.fact("link", &[0, 2]);
    bg.fact("link", &[1, 3]);
    bg.fact("green", &[2]);
    bg.fact("blue", &[3]);
    bg
}

fn bisimilar() -> Background {
    let mut bg = Background::new(sig(&[("red", 1), ("green", 1), ("link", 2)]));
    bg.fact("red", &[0]);
    bg.fact("red", &[1]);
    bg.fact("link", &[0, 2]);
    bg.fact("link", &[1, 3]);
    bg.fact("green", &[2]);
    bg.fact("green", &[3]);
    bg
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let domain: Vec<u32> = (0..4).collect();

    let bg = trees();
    let pattern = separating_pattern(&bg, &domain, 0, 1).expect("a relational pattern");
    println!("unary: 0 and 1 collide (both red)");
    println!("pattern: {}", pattern.to_string());
    let ext = extension(&pattern, &bg, &domain);
    println!("extension: {:?}", ext);

    let bi = bisimilar();
    let none = separating_pattern(&bi, &domain, 0, 1);
    println!(
        "bisimilar: {}",
        if none.is_none() { "no separating pattern" } else { "pattern found" }
    );

    if let Some(path) = out {
        let ids: Vec<String> = extension(&pattern, &bg, &domain)
            .iter()
            .map(|t| t[0].to_string())
            .collect();
        let body = format!(
            "{{\"protocol\":\"metaform.propose/1\",\"request_id\":\"relational\",\"candidates\":[{{\"name\":\"linked_to_green\",\"ids\":[{}],\"warrant\":\"a tree-shaped pattern that separates 0 from 1\",\"falsifier\":\"a link to a green object that is not a counterexample\"}}],\"refused\":null}}\n",
            ids.join(",")
        );
        std::fs::write(&path, body).expect("write the response");
        println!("response written to {}", path);
    }
}
