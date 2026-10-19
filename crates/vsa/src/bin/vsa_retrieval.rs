//! Measures the retrieval arm of the VSA component.
//!
//! Usage: vsa_retrieval [--trials N] [--dim N] [--objects N] [--attrs N] [--target N] [--seed N]

use metaform_vsa::retrieval::{run, Config};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut cfg = Config::default();
    if let Some(v) = arg(&args, "--trials") {
        cfg.trials = v.parse().expect("--trials");
    }
    if let Some(v) = arg(&args, "--dim") {
        cfg.dim = v.parse().expect("--dim");
    }
    if let Some(v) = arg(&args, "--objects") {
        cfg.objects = v.parse().expect("--objects");
    }
    if let Some(v) = arg(&args, "--attrs") {
        cfg.attrs = v.parse().expect("--attrs");
    }
    if let Some(v) = arg(&args, "--target") {
        cfg.target = v.parse().expect("--target");
    }
    if let Some(v) = arg(&args, "--seed") {
        cfg.seed = v.parse().expect("--seed");
    }

    let report = run(cfg);
    println!(
        "trials={} dim={} objects={} attrs={} target={}",
        report.trials, cfg.dim, cfg.objects, cfg.attrs, cfg.target
    );
    println!("vsa top-1 splits the pair: {:.3}", report.vsa_hit_rate());
    println!("random candidate splits:    {:.3}", report.random_hit_rate());
    println!(
        "cost at r={}: vsa q={:.3} -> {:.1}",
        report.proposal_cost,
        report.vsa_hit_rate(),
        report.cost_law(report.vsa_hit_rate())
    );
    println!(
        "cost at r={}: random q={:.3} -> {:.1}",
        report.proposal_cost,
        report.random_hit_rate(),
        report.cost_law(report.random_hit_rate())
    );
}
