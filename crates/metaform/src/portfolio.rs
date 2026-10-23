//! Portfolio identification: several vocabularies, of which exactly one is sufficient.
//!
//! The setup is the executable counterpart of the paper's colliding-pair certificate (Theorem 2):
//! a vocabulary is **eliminated** when a positive and a negative object have the same signature,
//! so no conjunction over that vocabulary can cover the positive and exclude the negative.
//!
//! | Paper | Here |
//! |---|---|
//! | Signature / equivalence classes | `concepts` + a member's consistent family |
//! | A colliding pair certifies insufficiency (T2) | `run_arm` marks a member eliminated when its family is empty |
//! | Sufficient vocabulary | a member whose family is non-empty |
//! | Decided vocabulary | a member whose family has exactly one extension |
//!
//! One trial builds a domain and a candidate set: one target member (base plus the target conjunction)
//! and `K-1` decoys (base plus a random pair). The portfolio arm uses all candidates; the baseline arm
//! draws one candidate uniformly, so the target member is drawn only with probability `1/K`.
//!
//! The portfolio decision rule is deliberately not "the first member with a singleton family":
//! - if two decided members disagree, the trial is a **conflict** and stops;
//! - if every decided member agrees and every non-eliminated member is decided, the portfolio decides;
//! - otherwise it keeps querying until `--max-queries` labels have been used.
//!
//! Queries use shared evidence, so all members see the same labels. The primary rule, [`Rule::Disagreement`],
//! asks the unlabelled object with the largest entropy of the members' votes (a member votes 0.5 when
//! its family disagrees on that object, otherwise 0 or 1). [`Rule::Random`] asks uniformly among the
//! unlabelled objects with the seeded generator.
//!
//! ## Report
//!
//! Rows are emitted as `rule, arm, k`; for every `(rule, k)` the baseline arm comes first. The counts
//! are means over trials, so `success` and `conflict` are rates. `queries_per_decoy` is the mean of
//! `(queries when the last decoy was eliminated) / (k-1)` over trials that eliminated every decoy;
//! it is 0 for the baseline and for `k = 1`.
//!
//! With the default budget (12 labels over 12 objects) every trial reaches the full labelling, so a
//! non-conflicting portfolio always succeeds; the conflict rate is then the only source of portfolio
//! failure. The single-member baseline can also fail by drawing an eliminated or still-open decoy.

use crate::{bit, Mask};

const DEFAULT_OBJECTS: usize = 12;
const DEFAULT_ATTRS: usize = 8;
const DEFAULT_TARGET: usize = 2;
const DEFAULT_TRIALS: u64 = 200;
const DEFAULT_MAX_QUERIES: usize = 12;
const DEFAULT_CONCEPT_SIZE: usize = 3;
const DEFAULT_SEED: u64 = 713;
const DEFAULT_KS: [usize; 4] = [1, 3, 5, 10];

/// Independent streams inside one trial, so that the candidate set does not perturb the query order.
const SALT_DATA: u64 = 0x11;
const SALT_DECOY: u64 = 0x22;
const SALT_QUERY: u64 = 0x33;
const SALT_DRAW: u64 = 0x44;

/// Query rule. Both rules are reported by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Ask the unlabelled object with the largest entropy of the members' predictions.
    Disagreement,
    /// Ask an unlabelled object uniformly with the seeded generator.
    Random,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Rule::Disagreement => "disagreement",
            Rule::Random => "random",
        }
    }
}

/// Portfolio arm: a single uniformly drawn member, or the whole candidate set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm {
    Baseline,
    Portfolio,
}

impl Arm {
    pub fn name(self) -> &'static str {
        match self {
            Arm::Baseline => "baseline",
            Arm::Portfolio => "portfolio",
        }
    }
}

/// Experiment configuration. Defaults follow the command line: 12 objects, 8 attributes, target 2.
#[derive(Clone, Debug)]
pub struct Config {
    pub objects: usize,
    pub attrs: usize,
    pub target: usize,
    pub trials: u64,
    pub max_queries: usize,
    pub concept_size: usize,
    pub seed: u64,
    pub ks: Vec<usize>,
    pub rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            objects: DEFAULT_OBJECTS,
            attrs: DEFAULT_ATTRS,
            target: DEFAULT_TARGET,
            trials: DEFAULT_TRIALS,
            max_queries: DEFAULT_MAX_QUERIES,
            concept_size: DEFAULT_CONCEPT_SIZE,
            seed: DEFAULT_SEED,
            ks: DEFAULT_KS.to_vec(),
            rules: vec![Rule::Disagreement, Rule::Random],
        }
    }
}

impl Config {
    /// Parse `--flag value` pairs. Values are validated by [`Config::validate`].
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut cfg = Config::default();
        let mut i = 0usize;
        while i < args.len() {
            let flag = args[i].as_str();
            let value = args
                .get(i + 1)
                .ok_or_else(|| format!("{flag} needs a value"))?;
            match flag {
                "--objects" => cfg.objects = parse_usize(flag, value)?,
                "--attrs" => cfg.attrs = parse_usize(flag, value)?,
                "--target" => cfg.target = parse_usize(flag, value)?,
                "--trials" => cfg.trials = parse_u64(flag, value)?,
                "--max-queries" => cfg.max_queries = parse_usize(flag, value)?,
                "--concept-size" => cfg.concept_size = parse_usize(flag, value)?,
                "--seed" => cfg.seed = parse_u64(flag, value)?,
                "--k" => cfg.ks = parse_ks(value)?,
                "--rule" => cfg.rules = parse_rules(value)?,
                other => return Err(format!("unknown option: {other}")),
            }
            i += 2;
        }
        Ok(cfg)
    }

    /// Check the limits imposed by the model (at most 64 objects because extensions are `u64`).
    pub fn validate(&self) -> Result<(), String> {
        if self.objects == 0 || self.objects > 64 {
            return Err("--objects must be between 1 and 64".into());
        }
        if self.attrs == 0 {
            return Err("--attrs must be at least 1".into());
        }
        if self.target == 0 || self.target > self.attrs {
            return Err("--target must be between 1 and --attrs".into());
        }
        if self.trials == 0 {
            return Err("--trials must be at least 1".into());
        }
        if self.concept_size == 0 {
            return Err("--concept-size must be at least 1".into());
        }
        if self.ks.is_empty() || self.ks.contains(&0) {
            return Err("--k must contain positive integers".into());
        }
        if self.ks.iter().any(|&k| k > 1) && self.attrs < 2 {
            return Err("--attrs must be at least 2 when a portfolio has more than one member".into());
        }
        if self.ks.iter().any(|&k| k > 1) && self.target >= 2 && self.attrs < 3 {
            return Err(
                "--attrs must be at least 3 when the target has two attributes and decoys are needed"
                    .into(),
            );
        }
        if self.rules.is_empty() {
            return Err("--rule must select at least one rule".into());
        }
        Ok(())
    }
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value.parse().map_err(|_| format!("{flag}: not a number: {value}"))
}

fn parse_u64(flag: &str, value: &str) -> Result<u64, String> {
    value.parse().map_err(|_| format!("{flag}: not a number: {value}"))
}

fn parse_ks(value: &str) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err("--k has an empty entry".into());
        }
        let k = parse_usize("--k", part)?;
        if k == 0 {
            return Err("--k must be at least 1".into());
        }
        if !out.contains(&k) {
            out.push(k);
        }
    }
    if out.is_empty() {
        Err("--k needs at least one value".into())
    } else {
        Ok(out)
    }
}

fn parse_rules(value: &str) -> Result<Vec<Rule>, String> {
    match value {
        "disagreement" => Ok(vec![Rule::Disagreement]),
        "random" => Ok(vec![Rule::Random]),
        "both" => Ok(vec![Rule::Disagreement, Rule::Random]),
        other => Err(format!("unknown rule: {other} (use disagreement, random, or both)")),
    }
}

/// Seeded generator: the same LCG the other experiments use, so the crate still has no dependencies.
#[derive(Clone, Debug)]
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0, "empty range");
        (self.next() as usize) % n
    }

    fn flip(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

/// Derive an independent stream from the experiment seed and a salt.
fn mix(seed: u64, salt: u64) -> u64 {
    let mut z = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn full_mask(n: usize) -> Mask {
    if n >= 64 {
        u64::MAX
    } else {
        (1u64 << n) - 1
    }
}

/// Every conjunction of between 1 and `max_size` predicates of `vocab`, deduplicated by extension.
fn concepts(vocab: &[Mask], max_size: usize, n: usize) -> Vec<Mask> {
    fn rec(vocab: &[Mask], start: usize, depth: usize, max: usize, cur: Mask, out: &mut Vec<Mask>) {
        if depth > 0 {
            out.push(cur);
        }
        if depth == max {
            return;
        }
        for i in start..vocab.len() {
            rec(vocab, i + 1, depth + 1, max, cur & vocab[i], out);
        }
    }
    let mut out = Vec::new();
    rec(vocab, 0, 0, max_size.min(vocab.len()), full_mask(n), &mut out);
    out.sort_unstable();
    out.dedup();
    out
}

fn choose_distinct(rng: &mut Rng, m: usize, t: usize) -> Vec<usize> {
    let mut pool: Vec<usize> = (0..m).collect();
    for i in 0..t {
        let j = i + rng.below(m - i);
        pool.swap(i, j);
    }
    pool.truncate(t);
    pool
}

/// A random pair of distinct attributes, never the target pair when there is one.
fn random_pair(rng: &mut Rng, m: usize, exclude: Option<(usize, usize)>) -> (usize, usize) {
    loop {
        let a = rng.below(m);
        let b0 = rng.below(m - 1);
        let b = if b0 >= a { b0 + 1 } else { b0 };
        let pair = (a.min(b), a.max(b));
        if Some(pair) != exclude {
            return pair;
        }
    }
}

/// The head of the trial: the object domain, the target extension, and the base vocabulary.
#[derive(Clone, Debug)]
struct World {
    n: usize,
    ext: Vec<Mask>,
    target_attrs: Vec<usize>,
    target_ext: Mask,
    base: Vec<Mask>,
}

/// For `target = 2` this is exactly the target conjunction. Longer targets are supported by treating
/// the whole target as the single predicate carried by the sufficient member; decoy pairs still
/// exclude the first two target attributes.
fn target_pair(world: &World) -> Option<(usize, usize)> {
    if world.target_attrs.len() >= 2 {
        let a = world.target_attrs[0];
        let b = world.target_attrs[1];
        Some((a.min(b), a.max(b)))
    } else {
        None
    }
}

fn build_world(cfg: &Config, trial: u64) -> World {
    let n = cfg.objects;
    let m = cfg.attrs;
    let mut rng = Rng::new(mix(cfg.seed ^ SALT_DATA, trial.wrapping_add(1)));
    let mut obj = vec![vec![false; m]; n];
    for row in obj.iter_mut() {
        for value in row.iter_mut() {
            *value = rng.flip();
        }
    }
    let target_attrs = choose_distinct(&mut rng, m, cfg.target);

    let mut ext = vec![0u64; m];
    for a in 0..m {
        for (x, row) in obj.iter().enumerate() {
            if row[a] {
                ext[a] |= bit(x);
            }
        }
    }
    let mut target_ext = full_mask(n);
    for &a in &target_attrs {
        target_ext &= ext[a];
    }
    let base = (0..m)
        .filter(|a| !target_attrs.contains(a))
        .map(|a| ext[a])
        .collect();
    World { n, ext, target_attrs, target_ext, base }
}

/// One portfolio member: a vocabulary, represented by all of its concept extensions.
#[derive(Clone, Debug)]
struct Candidate {
    concepts: Vec<Mask>,
}

/// Build the target member first (`candidates[0]`) and then `k - 1` decoys.
fn build_candidates(world: &World, cfg: &Config, k: usize, trial: u64) -> Vec<Candidate> {
    let mut target_vocab = world.base.clone();
    target_vocab.push(world.target_ext);
    let mut out = Vec::with_capacity(k);
    out.push(Candidate { concepts: concepts(&target_vocab, cfg.concept_size, world.n) });

    if k > 1 {
        let mut rng = Rng::new(mix(cfg.seed ^ SALT_DECOY, trial.wrapping_add(1)));
        let exclude = target_pair(world);
        for _ in 1..k {
            let (a, b) = random_pair(&mut rng, world.ext.len(), exclude);
            let mut vocab = world.base.clone();
            vocab.push(world.ext[a] & world.ext[b]);
            out.push(Candidate { concepts: concepts(&vocab, cfg.concept_size, world.n) });
        }
    }
    out
}

/// Entropy of the mean member vote for each unlabelled object; ties keep the smaller index.
fn choose_disagreement(families: &[Vec<Mask>], labelled: Mask, n: usize) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for x in 0..n {
        let b = bit(x);
        if labelled & b != 0 {
            continue;
        }
        let mut sum = 0.0f64;
        let mut voters = 0usize;
        for fam in families {
            if fam.is_empty() {
                continue;
            }
            let yes = fam.iter().filter(|c| **c & b != 0).count();
            let vote = if yes == 0 {
                0.0
            } else if yes == fam.len() {
                1.0
            } else {
                0.5
            };
            sum += vote;
            voters += 1;
        }
        if voters == 0 {
            continue;
        }
        let p = sum / voters as f64;
        let h = if p <= 0.0 || p >= 1.0 {
            0.0
        } else {
            -(p * p.log2() + (1.0 - p) * (1.0 - p).log2())
        };
        if best.is_none_or(|(_, best_h)| h > best_h + 1e-12) {
            best = Some((x, h));
        }
    }
    best.map(|(x, _)| x)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Success,
    Failure,
    Conflict,
}

#[derive(Clone, Copy, Debug)]
struct ArmResult {
    outcome: Outcome,
    queries: usize,
    eliminated: usize,
    /// Queries at the first evaluation where every decoy family was empty (portfolio arm only).
    last_decoy_elim: Option<usize>,
}

/// Run one arm: `members` is either one drawn candidate or the whole portfolio.
fn run_arm(
    members: &[Candidate],
    target_ext: Mask,
    n: usize,
    max_queries: usize,
    rule: Rule,
    query_seed: u64,
    track_decoys: bool,
) -> ArmResult {
    let mut families: Vec<Vec<Mask>> = members.iter().map(|m| m.concepts.clone()).collect();
    let mut labelled = 0u64;
    let mut queries = 0usize;
    let mut last_decoy_elim = None;
    let mut rng = Rng::new(query_seed);
    let full = full_mask(n);

    loop {
        let mut decided: Vec<Mask> = Vec::new();
        let mut open = 0usize;
        let mut eliminated = 0usize;
        for fam in &families {
            match fam.len() {
                0 => eliminated += 1,
                1 => decided.push(fam[0]),
                _ => open += 1,
            }
        }
        if track_decoys
            && last_decoy_elim.is_none()
            && families.len() > 1
            && families[1..].iter().all(|fam| fam.is_empty())
        {
            last_decoy_elim = Some(queries);
        }

        // Two decided members that disagree fail the trial outright.
        if decided.len() >= 2 && decided.iter().any(|d| *d != decided[0]) {
            return ArmResult { outcome: Outcome::Conflict, queries, eliminated, last_decoy_elim };
        }
        // Every non-eliminated member is decided and they agree: the portfolio decides.
        if !decided.is_empty() && open == 0 {
            let outcome = if decided[0] == target_ext { Outcome::Success } else { Outcome::Failure };
            return ArmResult { outcome, queries, eliminated, last_decoy_elim };
        }
        // Every member is eliminated: no extension can be decided.
        if decided.is_empty() && open == 0 {
            return ArmResult { outcome: Outcome::Failure, queries, eliminated, last_decoy_elim };
        }
        // The budget is spent. This evaluation happens after the last label, so there is no off-by-one.
        if queries >= max_queries || labelled == full {
            return ArmResult { outcome: Outcome::Failure, queries, eliminated, last_decoy_elim };
        }

        let x = match rule {
            Rule::Random => {
                let free: Vec<usize> =
                    (0..n).filter(|&x| labelled & bit(x) == 0).collect();
                free[rng.below(free.len())]
            }
            Rule::Disagreement => choose_disagreement(&families, labelled, n)
                .or_else(|| (0..n).find(|&x| labelled & bit(x) == 0))
                .expect("an unlabelled object exists"),
        };
        let b = bit(x);
        let positive = target_ext & b != 0;
        for fam in &mut families {
            if positive {
                fam.retain(|c| c & b != 0);
            } else {
                fam.retain(|c| c & b == 0);
            }
        }
        labelled |= b;
        queries += 1;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ArmStats {
    trials: u64,
    success: u64,
    queries: u64,
    eliminated: u64,
    conflict: u64,
    decoy_elim_queries: u64,
    decoy_elim_trials: u64,
}

impl ArmStats {
    fn record(&mut self, result: ArmResult, k: usize, portfolio: bool) {
        self.trials += 1;
        self.success += u64::from(result.outcome == Outcome::Success);
        self.conflict += u64::from(result.outcome == Outcome::Conflict);
        self.queries += result.queries as u64;
        self.eliminated += result.eliminated as u64;
        if portfolio && k > 1 {
            if let Some(q) = result.last_decoy_elim {
                self.decoy_elim_queries += q as u64;
                self.decoy_elim_trials += 1;
            }
        }
    }
}

fn ratio(total: u64, n: u64) -> f64 {
    if n == 0 {
        0.0
    } else {
        total as f64 / n as f64
    }
}

/// One report row: one rule, one arm, one portfolio size.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub rule: Rule,
    pub arm: Arm,
    pub k: usize,
    pub success: f64,
    pub queries: f64,
    pub eliminated: f64,
    pub queries_per_decoy: f64,
    pub conflict: f64,
}

impl Row {
    fn from_stats(rule: Rule, arm: Arm, k: usize, stats: &ArmStats) -> Self {
        let queries_per_decoy = if arm == Arm::Portfolio && k > 1 {
            ratio(stats.decoy_elim_queries, stats.decoy_elim_trials) / (k - 1) as f64
        } else {
            0.0
        };
        Self {
            rule,
            arm,
            k,
            success: ratio(stats.success, stats.trials),
            queries: ratio(stats.queries, stats.trials),
            eliminated: ratio(stats.eliminated, stats.trials),
            queries_per_decoy,
            conflict: ratio(stats.conflict, stats.trials),
        }
    }

    fn to_json(self) -> String {
        format!(
            "{{\"rule\":\"{}\",\"arm\":\"{}\",\"k\":{},\"success\":{:.3},\"queries\":{:.3},\"eliminated\":{:.3},\"queries_per_decoy\":{:.3},\"conflict\":{:.3}}}",
            self.rule.name(),
            self.arm.name(),
            self.k,
            self.success,
            self.queries,
            self.eliminated,
            self.queries_per_decoy,
            self.conflict
        )
    }

    fn summary(&self) -> String {
        let arm = match self.arm {
            Arm::Baseline => format!("baseline(1 of {})", self.k),
            Arm::Portfolio => format!("portfolio({})", self.k),
        };
        format!(
            "{} k={} {}: success={:.3} queries={:.3} eliminated={:.3} queries_per_decoy={:.3} conflict={:.3}",
            self.rule.name(),
            self.k,
            arm,
            self.success,
            self.queries,
            self.eliminated,
            self.queries_per_decoy,
            self.conflict
        )
    }
}

/// The complete report.
pub struct Report {
    pub config: Config,
    pub rows: Vec<Row>,
}

impl Report {
    pub fn to_json(&self) -> String {
        let rows: Vec<String> = self.rows.iter().map(|row| row.to_json()).collect();
        let rules: Vec<String> =
            self.config.rules.iter().map(|r| format!("\"{}\"", r.name())).collect();
        format!(
            "{{\"objects\":{},\"attrs\":{},\"target\":{},\"trials\":{},\"seed\":{},\"max_queries\":{},\"concept_size\":{},\"rules\":[{}],\"rows\":[{}]}}",
            self.config.objects,
            self.config.attrs,
            self.config.target,
            self.config.trials,
            self.config.seed,
            self.config.max_queries,
            self.config.concept_size,
            rules.join(","),
            rows.join(",")
        )
    }
}

/// Run both arms for one rule and one portfolio size.
fn run_pair(cfg: &Config, rule: Rule, k: usize) -> (ArmStats, ArmStats) {
    let mut baseline = ArmStats::default();
    let mut portfolio = ArmStats::default();
    for trial in 0..cfg.trials {
        let world = build_world(cfg, trial);
        let candidates = build_candidates(&world, cfg, k, trial);
        let query_seed = mix(cfg.seed ^ SALT_QUERY, trial.wrapping_add(1));

        // The baseline is one member drawn uniformly from the same candidate set.
        let mut draw = Rng::new(mix(cfg.seed ^ SALT_DRAW, trial.wrapping_add(1)));
        let pick = draw.below(k);
        let drawn = run_arm(
            std::slice::from_ref(&candidates[pick]),
            world.target_ext,
            world.n,
            cfg.max_queries,
            rule,
            query_seed,
            false,
        );
        baseline.record(drawn, k, false);

        let full = run_arm(
            &candidates,
            world.target_ext,
            world.n,
            cfg.max_queries,
            rule,
            query_seed,
            true,
        );
        portfolio.record(full, k, true);
    }
    (baseline, portfolio)
}

/// Run every (rule, k) pair and emit the baseline row first.
pub fn run(config: &Config) -> Report {
    if let Err(e) = config.validate() {
        panic!("invalid portfolio configuration: {e}");
    }
    let mut rows = Vec::new();
    for &rule in &config.rules {
        for &k in &config.ks {
            let (baseline, portfolio) = run_pair(config, rule, k);
            rows.push(Row::from_stats(rule, Arm::Baseline, k, &baseline));
            rows.push(Row::from_stats(rule, Arm::Portfolio, k, &portfolio));
        }
    }
    Report { config: config.clone(), rows }
}

fn usage() -> &'static str {
    "usage: metaform portfolio [--objects N] [--attrs M] [--target T] [--k K1,K2,...]\n\
     [--trials N] [--max-queries N] [--concept-size N] [--seed N] [--rule disagreement|random|both]\n\
     defaults: --objects 12 --attrs 8 --target 2 --k 1,3,5,10 --trials 200\n\
     --max-queries 12 --concept-size 3 --seed 713 --rule both"
}

/// Command-line entry point: print the JSON on stdout and one summary line per arm on stderr.
pub fn run_cli(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", usage());
        return 0;
    }
    let config = match Config::parse(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("portfolio: {e}");
            eprintln!("{}", usage());
            return 2;
        }
    };
    if let Err(e) = config.validate() {
        eprintln!("portfolio: {e}");
        return 2;
    }
    let report = run(&config);
    println!("{}", report.to_json());
    for row in &report.rows {
        eprintln!("{}", row.summary());
    }
    0
}

#[cfg(test)]
fn consistent_concepts(concepts: &[Mask], pos: Mask, neg: Mask) -> Vec<Mask> {
    concepts
        .iter()
        .copied()
        .filter(|c| pos & !c == 0 && c & neg == 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The base vocabulary excludes every target attribute, so it cannot cover the target.
    #[test]
    fn portfolio_base_is_insufficient() {
        let n = 4;
        let target_ext = bit(0);
        // The only base predicate is false on the target positive, so no base-only
        // conjunction can contain it while staying consistent with the full labelling.
        let base = vec![bit(1) | bit(3)];
        let base_concepts = concepts(&base, 3, n);
        let full = full_mask(n);
        let family = consistent_concepts(&base_concepts, target_ext, full & !target_ext);
        assert!(family.is_empty(), "the base alone must not decide the target: {family:?}");
        assert!(!base_concepts.contains(&target_ext), "the base alone cannot express the target");
    }

    /// Adding the target conjunction makes the sufficient member decide the target.
    #[test]
    fn portfolio_exactly_one_member_is_sufficient() {
        let n = 4;
        let target_ext = bit(0) | bit(1);
        // Both base predicates miss one positive, so no base-only conjunction is consistent
        // with the full labelling; the target conjunction is the only consistent concept.
        let base = vec![bit(0) | bit(2), bit(1) | bit(3)];
        let mut vocab = base;
        vocab.push(target_ext);
        let member = concepts(&vocab, 3, n);
        let full = full_mask(n);
        let family = consistent_concepts(&member, target_ext, full & !target_ext);
        assert_eq!(family, vec![target_ext]);
    }

    /// A colliding pair empties a decoy's family; on a fixed seed more labels eliminate more decoys.
    #[test]
    fn portfolio_decoy_is_eliminated() {
        // Constructed collision: positive 0 and negative 2 have identical signatures.
        let n = 4;
        let decoy = concepts(
            &[bit(0) | bit(2), bit(1) | bit(3), bit(0) | bit(2) | bit(3)],
            3,
            n,
        );
        assert!(!consistent_concepts(&decoy, 0, 0).is_empty(), "the family starts non-empty");
        assert!(!consistent_concepts(&decoy, bit(0), 0).is_empty(), "one label is not enough");
        assert!(
            consistent_concepts(&decoy, bit(0), bit(2)).is_empty(),
            "the colliding pair eliminates the decoy"
        );

        // Seeded growth: label the first q objects in index order and count eliminated decoys.
        let cfg = Config {
            objects: 12,
            attrs: 8,
            target: 2,
            trials: 20,
            max_queries: 12,
            concept_size: 3,
            seed: 713,
            ks: vec![5],
            rules: vec![Rule::Disagreement],
        };
        let mut counts = Vec::new();
        for q in [1usize, 4, 8, 12] {
            let mut eliminated = 0u64;
            for trial in 0..cfg.trials {
                let world = build_world(&cfg, trial);
                let candidates = build_candidates(&world, &cfg, 5, trial);
                let mut family = candidates[1].concepts.clone();
                for x in 0..q {
                    let b = bit(x);
                    if world.target_ext & b != 0 {
                        family.retain(|c| c & b != 0);
                    } else {
                        family.retain(|c| c & b == 0);
                    }
                }
                if family.is_empty() {
                    eliminated += 1;
                }
            }
            counts.push(eliminated);
        }
        assert!(
            counts.windows(2).all(|w| w[0] <= w[1]),
            "eliminations must be monotone in the label budget: {counts:?}"
        );
        assert!(
            counts[3] > counts[0],
            "more labels must eliminate more decoys: {counts:?}"
        );
    }

    /// The portfolio must beat the single-member baseline drawn from the same candidate set.
    #[test]
    fn portfolio_success_increases_with_k() {
        let cfg = Config {
            objects: 12,
            attrs: 8,
            target: 2,
            trials: 100,
            max_queries: 12,
            concept_size: 3,
            seed: 713,
            ks: vec![5],
            rules: vec![Rule::Disagreement],
        };
        let report = run(&cfg);
        let baseline = report
            .rows
            .iter()
            .find(|r| r.arm == Arm::Baseline && r.k == 5)
            .expect("baseline row");
        let portfolio = report
            .rows
            .iter()
            .find(|r| r.arm == Arm::Portfolio && r.k == 5)
            .expect("portfolio row");
        assert!(
            portfolio.success > baseline.success,
            "portfolio success {:.3} must beat the k=1 baseline {:.3}",
            portfolio.success,
            baseline.success
        );
    }
}
