//! Learns a clause for `grandparent` and writes it as a proposal for the kernel.
//!
//! The objects the kernel sees are ordered pairs of people (a * 6 + b). The vocabulary in force has
//! `parent` and `same_city` over pairs, which cannot separate a positive pair from a negative one.
//! The learned clause is projected onto the pairs and written as a `metaform.propose/1` response,
//! so the kernel -- not this program -- decides whether it is any good.
//!
//! Usage: ilp_demo [--out <response.json>]

use metaform_ilp::{extension, ground, learn, Background, Signature};

const PEOPLE: u32 = 6;

fn family() -> Background {
    let mut bg = Background::new(vec![
        Signature { name: "parent".to_string(), arity: 2 },
        Signature { name: "same_city".to_string(), arity: 2 },
    ]);
    for (a, b) in [(0, 1), (1, 2), (2, 3), (4, 5)] {
        bg.fact("parent", &[a, b]);
    }
    for (a, b) in [(0, 1), (1, 2), (4, 5)] {
        bg.fact("same_city", &[a, b]);
    }
    bg
}

fn pair_id(a: u32, b: u32) -> u64 {
    1u64 << (a * PEOPLE + b)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let bg = family();
    let positives = [ground("grandparent", &[0, 2]), ground("grandparent", &[1, 3])];
    let negatives = [ground("grandparent", &[0, 3]), ground("grandparent", &[4, 5])];

    let clause = learn("grandparent", 2, &positives, &negatives, &bg, 2, 3)
        .expect("a clause for grandparent");
    println!("clause: {}", clause.to_string());

    let domain: Vec<u32> = (0..PEOPLE).collect();
    let ext = extension(&clause, &bg, &domain);
    let mut mask = 0u64;
    for tuple in &ext {
        mask |= pair_id(tuple[0], tuple[1]);
    }
    println!("extension: {} pair(s) out of {}", ext.len(), PEOPLE * PEOPLE);

    if let Some(path) = out {
        let ids: Vec<String> = (0..(PEOPLE * PEOPLE))
            .filter(|i| (mask >> i) & 1 == 1)
            .map(|i| i.to_string())
            .collect();
        let body = format!(
            "{{\"protocol\":\"metaform.propose/1\",\"request_id\":\"ilp\",\"candidates\":[{{\"name\":\"grandparent_pair\",\"ids\":[{}],\"warrant\":\"learned clause\",\"falsifier\":\"a pair that satisfies the body but is not a grandparent\"}}],\"refused\":null}}\n",
            ids.join(",")
        );
        std::fs::write(&path, body).expect("write the response");
        println!("response written to {}", path);
    }
}
