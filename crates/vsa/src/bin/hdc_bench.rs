//! Capacity and noise of the two memories, measured rather than assumed.
//!
//! Run with `cargo run --release -p metaform-vsa --bin hdc_bench`. The table answers the two
//! questions a user of the library actually has: how many items fit at a given dimension, and how
//! much noise the recall survives. The numbers are properties of bipolar superposition, so they
//! should follow the `1/sqrt(N)` signal law; a row that does not is a bug.

use metaform_vsa::mbat::MBAT;
use metaform_vsa::memory::{AssociativeMemory, ItemMemory};
use metaform_vsa::{Hypervector, VSA};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn corrupt(v: &MBAT, flips: f64, rng: &mut StdRng) -> MBAT {
    if flips <= 0.0 {
        return v.clone();
    }
    let data = v.data.iter().map(|&x| if rng.gen_bool(flips) { -x } else { x }).collect();
    MBAT { data }
}

fn probe_accuracy(dim: usize, n: usize, flips: f64, probes: usize, seed: u64) -> (f64, f64, f64) {
    let mut items = ItemMemory::new(dim);
    let mut values = ItemMemory::new(dim);
    let mut assoc = AssociativeMemory::new(dim);
    for i in 0..n {
        items.add(&format!("k{i}"));
        values.add(&format!("v{i}"));
        let k = items.get(&format!("k{i}")).unwrap().clone();
        let v = values.get(&format!("v{i}")).unwrap().clone();
        assoc.store(&k, &v);
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut item_hits = 0;
    let mut assoc_hits = 0;
    let mut raw_hits = 0;
    for i in 0..probes {
        let idx = (i * 7919) % n;
        let key = items.get(&format!("k{idx}")).unwrap().clone();
        let noisy = Hypervector { inner: corrupt(&key, flips, &mut rng) };
        if let Some((hit, _)) = items.cleanup(&noisy) {
            if items.names()[hit] == format!("k{idx}") {
                item_hits += 1;
            }
        }
        let probe = corrupt(&key, flips, &mut rng);
        if let Some((hit, _)) = assoc.recall(&probe, &values) {
            if values.names()[hit] == format!("v{idx}") {
                assoc_hits += 1;
            }
        }
        if let Some((hit, _)) = assoc.recall_raw(&probe, &values) {
            if values.names()[hit] == format!("v{idx}") {
                raw_hits += 1;
            }
        }
    }
    (
        item_hits as f64 / probes as f64,
        assoc_hits as f64 / probes as f64,
        raw_hits as f64 / probes as f64,
    )
}

fn main() {
    println!("bipolar MBAT, 64 probes per cell, value = top-1 accuracy");
    for dim in [2048usize, 4096] {
        println!("\n### dimension {dim}\n");
        println!("| items | noise | item memory | assoc (quantised) | assoc (raw trace) |");
        println!("|---|---|---|---|---|");
        for n in [16usize, 64, 256, 1024] {
            for flips in [0.0, 0.05, 0.10, 0.20] {
                let (item, assoc, raw) = probe_accuracy(dim, n, flips, 64, 7);
                println!("| {n} | {:.0}% | {:.3} | {:.3} | {:.3} |", flips * 100.0, item, assoc, raw);
            }
        }
    }

    // The signal law: at a fixed noise, accuracy should fall with 1/sqrt(N).
    println!("\n### signal law (dimension 4096, clean probe, raw trace)\n");
    for n in [16usize, 64, 256, 1024] {
        let mut sum = 0.0;
        for seed in 0..8u64 {
            let (_, _, raw) = probe_accuracy(4096, n, 0.0, 32, seed);
            sum += raw;
        }
        println!("items {n:5}: associative recall {:.3}", sum / 8.0);
    }
}
