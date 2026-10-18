//! 多算子定型：论文 §4（`Operators as typed transitions`）的可执行对应物。
//!
//! 论文把"下一步做什么"拆成五类步骤，并只用一个度量比较它们：
//!
//! | 论文 | 这里 |
//! |---|---|
//! | 状态 `s=(H,V,E,B)` | [`State`] |
//! | 活集 `L(s)` / 势 `Φ(s)` | [`State::live`] / [`State::phi`] |
//! | 算子 `(rd, wr, warrant, cost, falsifier)` | [`Op`] + [`Op::class`] + [`Op::cost`] + [`Op::warrant`] |
//! | T8 生成单调 | 生成只往 `H` 里加，`Φ` 不减 |
//! | T9 闭包幂等 | 闭包两次 = 闭包一次 |
//! | T10 证据唯一严格减小 | 能在活集上劈开的一问，严格降 `Φ` |
//! | T11 重组打不破撞车 | 词汇内部布尔组合被 `warrant` 直接拒收 |
//! | 推论（交替） | `Φ=0` 时只有新原子能救；`Φ≥2` 时只有取证能降 |
//! | 控制的全部收益 = 省掉的冗余 | [`run`] 的 `skip_redundant` 与 [`demo`] |
//!
//! 这里刻意不实现搜索、不实现排序打分：本模块只回答"这一步能不能改变度量"。
//!
//! ## 没有实现的部分（别把对应关系读大）
//!
//! - **生成的 warrant 没有实装**：论文说生成的 warrant 是"覆盖 + 简洁"（T7 与 MDL），
//!   这里只检查 T8 的方向（加候选不降 `Φ`），不判断候选好不好。
//! - **表征的 warrant 检查两件事**：新原子不是当前词汇的布尔组合（T11 的否定面），
//!   且每个候选在 `V ∪ {新原子}` 下可定义——否则就是把没表达的东西记成已表达。
//!   覆盖（T7）只作调度分数 [`Op::coverage`]，不作拒收条件；保真条件（换表征后能重建
//!   旧证据）仍未实装。
//! - **标签继承要验证**：普通取证会把对象记进 `asked`；[`Op::Probe`] 只能问"多成员等价类里
//!   没被直接问过"的成员，用来检验继承来的标签。`State::unprobed` 列出这些成员。
//! - **关系只通过投影进入词汇**：`State` 存二元关系（`rels[r][x]` 是后继集），
//!   但 `signature` / `definable` 仍是一元的；关系要先经 [`State::image`] 或
//!   [`State::preimage`] 变成一元谓词，才能作为原子被 `Represent` 提出。
//! - **控制的代价模型没有实装**：`is_discriminating` 只说"会不会劈开活集"，
//!   不说"值不值这个代价"；实验成本在调用方。
//! - **控制不是一步**：它是 [`run`] 的 `skip_redundant` 模式；论文里的 `B` 由 `run` 记账。

use crate::{bit, Mask};

/// 四类可执行算子 + 控制。类型即论文里的 `(rd, wr)`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// 归纳 / 溯因 / 类比：只写 `H`，按包含增长。
    Generation,
    /// 演绎 / 一致性检查：只写 `H`，过滤到与证据一致的候选。
    Closure,
    /// 词汇扩张 / 反统一 / 对称商：只写 `V`。
    Representation,
    /// 反例搜索 / 主动学习 / 实验：只写 `E`。
    Evidence,
    /// 元推理：只写 `B`，在其余四类之间选。
    Control,
}

impl Class {
    pub fn writes(self) -> &'static str {
        match self {
            Class::Generation | Class::Closure => "H",
            Class::Representation => "V",
            Class::Evidence => "E",
            Class::Control => "B",
        }
    }
}

/// 方向：写 `H` 的算子靠它区分"加（↑）"与"筛（↓）"。
/// 论文说"类型定下类别"；只看 `wr` 只有四个取值，必须加上方向才是五个。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    None,
}

/// 状态：候选族 `H`、词汇 `V`、证据 `E`、预算 `B`。
#[derive(Clone, Debug)]
pub struct State {
    pub n: usize,
    pub vocab: Vec<Mask>,
    /// 二元关系：`rels[r][x]` 是 `x` 在关系 `r` 下的后继集合。
    /// 关系不直接进签名，只通过 [`State::image`] / [`State::preimage`] 投影成一元谓词后进入词汇。
    pub rels: Vec<Vec<Mask>>,
    pub hyps: Vec<Mask>,
    pub pos: Mask,
    pub neg: Mask,
    /// 被直接问过的对象。类标签会"继承"给同类成员，所以没被直接问过的成员是未验证的。
    pub asked: Mask,
    pub budget: u32,
}

impl State {
    pub fn new(n: usize) -> Self {
        assert!(n <= 64, "Mask 是 u64，最多 64 个对象");
        Self {
            n,
            vocab: Vec::new(),
            rels: Vec::new(),
            hyps: Vec::new(),
            pos: 0,
            neg: 0,
            asked: 0,
            budget: u32::MAX,
        }
    }

    /// 未验证的成员：多成员等价类里、没被直接问过的对象。
    /// 这些对象的标签是从同类成员"继承"来的，可能错。
    pub fn unprobed(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for c in self.classes() {
            if c.count_ones() < 2 {
                continue;
            }
            for x in 0..self.n {
                if c & bit(x) != 0 && self.asked & bit(x) == 0 {
                    out.push(x);
                }
            }
        }
        out
    }

    /// 关系 `r` 从集合 `s` 出发一步能到达的对象：`{y | ∃x∈s, r(x,y)}`。
    pub fn image(&self, r: usize, s: Mask) -> Mask {
        let mut out = 0u64;
        for x in 0..self.n {
            if s & bit(x) != 0 {
                out |= self.rels[r][x];
            }
        }
        out
    }

    /// 关系 `r` 下能一步到达 `s` 的对象：`{x | ∃y∈s, r(x,y)}`。
    /// 这是把二元关系投影成一元谓词的最短路径（ILP 里的一条 lift）。
    pub fn preimage(&self, r: usize, s: Mask) -> Mask {
        let mut out = 0u64;
        for x in 0..self.n {
            if self.rels[r][x] & s != 0 {
                out |= bit(x);
            }
        }
        out
    }

    pub fn full(n: usize) -> Mask {
        if n >= 64 {
            u64::MAX
        } else {
            (1u64 << n) - 1
        }
    }

    /// 候选 `h` 与证据一致：含全部正例，不含任何反例。
    pub fn consistent(&self, h: Mask) -> bool {
        self.pos & !h == 0 && h & self.neg == 0
    }

    /// 活集 `L(s)`。
    pub fn live(&self) -> Vec<Mask> {
        self.hyps.iter().copied().filter(|h| self.consistent(*h)).collect()
    }

    /// 势 `Φ(s)`。
    pub fn phi(&self) -> usize {
        self.live().len()
    }

    /// `x` 在 `V` 下的签名。
    pub fn signature(&self, x: usize) -> Vec<bool> {
        self.vocab.iter().map(|p| p & bit(x) != 0).collect()
    }

    /// 按签名分组（等价类）。
    pub fn classes(&self) -> Vec<Mask> {
        self.classes_with(0)
    }

    /// 词汇加上 `extra` 之后的等价类。`extra = 0` 时就是当前词汇的划分。
    pub fn classes_with(&self, extra: Mask) -> Vec<Mask> {
        let mut keys: Vec<Vec<bool>> = Vec::new();
        let mut masks: Vec<Mask> = Vec::new();
        for x in 0..self.n {
            let mut s = self.signature(x);
            s.push(extra & bit(x) != 0);
            match keys.iter().position(|k| *k == s) {
                Some(i) => masks[i] |= bit(x),
                None => {
                    keys.push(s);
                    masks.push(bit(x));
                }
            }
        }
        masks
    }

    /// 用当前词汇的合取描述 `h` 所需的最少文字数（每个谓词可正可负），
    /// 至多枚举 3 个文字；超出返回 `None`。这是 MDL 的工程近似：只算模型项，不算残差。
    pub fn mdl(&self, h: Mask) -> Option<usize> {
        let m = self.vocab.len();
        if m > 12 {
            return None;
        }
        let full = State::full(self.n);
        for k in 1..=3usize.min(m) {
            for combo in combos(m, k) {
                for signs in 0..(1u32 << k) {
                    let mut mask = full;
                    for (i, &idx) in combo.iter().enumerate() {
                        let lit = if signs >> i & 1 == 0 {
                            self.vocab[idx]
                        } else {
                            full & !self.vocab[idx]
                        };
                        mask &= lit;
                    }
                    if mask == h {
                        return Some(k);
                    }
                }
            }
        }
        None
    }

    /// 当前词汇下不可分辨、却分别落在正例与反例里的对象对（论文的撞车对）。
    pub fn colliding_pairs(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for x in 0..self.n {
            if self.pos & bit(x) == 0 {
                continue;
            }
            for y in 0..self.n {
                if self.neg & bit(y) != 0 && self.signature(x) == self.signature(y) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    /// `h` 是否 `V`-可定义：在每个等价类上取常值（论文 T1）。
    pub fn definable(&self, h: Mask) -> bool {
        self.definable_with(0, h)
    }

    /// `h` 在 `V ∪ {extra}` 下是否可定义。`Represent` 的 warrant 用它：
    /// 新原子必须真的让这些候选可表达，否则这一步只是把没表达的东西记成已表达。
    pub fn definable_with(&self, extra: Mask, h: Mask) -> bool {
        for c in self.classes_with(extra) {
            let inside = h & c;
            if inside != 0 && inside != c {
                return false;
            }
        }
        true
    }

    /// 当前词汇下的全部 `V`-可定义集（等价类之并）。
    /// 演示域保持小（类数 ≤ 20），否则返回空。
    pub fn enumerate_definable(&self) -> Vec<Mask> {
        let cs = self.classes();
        if cs.len() > 20 {
            return Vec::new();
        }
        (0u32..(1u32 << cs.len()))
            .map(|m| {
                let mut out = 0u64;
                for (i, c) in cs.iter().enumerate() {
                    if m & (1 << i) != 0 {
                        out |= c;
                    }
                }
                out
            })
            .collect()
    }
}

/// `m` 个下标里取 `k` 个的所有组合（`m` 很小，演示域够用）。
fn combos(m: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    fn go(start: usize, m: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..m {
            cur.push(i);
            go(i + 1, m, k, cur, out);
            cur.pop();
        }
    }
    go(0, m, k, &mut cur, &mut out);
    out
}

/// 从候选原子里选覆盖面最大的一个（T7 的可执行版本）。
/// 若某个原子单独就能让目标可定义，它必然分开全部撞车对，因此这个选择在该情形下不是启发式。
/// 布尔重组（`V`-可定义的候选）不参与——那是 T11 说打不破撞车的那种。
pub fn best_cover(s: &State, pool: &[Mask]) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, &a) in pool.iter().enumerate() {
        if s.definable(a) {
            continue;
        }
        let cov = s
            .colliding_pairs()
            .iter()
            .filter(|(x, y)| ((a >> *x) & 1) != ((a >> *y) & 1))
            .count();
        match best {
            None => best = Some((i, cov)),
            Some((_, bc)) => {
                if cov > bc {
                    best = Some((i, cov));
                }
            }
        }
    }
    best.map(|(i, _)| i)
}

/// 两个原子才够用时，三种顺序各自要几个原子。`k` 是"方向"数（需要 `k` 个原子）。
pub struct ComposeStats {
    pub trials: u64,
    pub fixed: f64,
    pub random: f64,
    pub greedy: f64,
    pub fixed_ok: f64,
    pub random_ok: f64,
    pub greedy_ok: f64,
}

/// 需要多个原子时：贪心覆盖 vs 随机顺序 vs 固定顺序（固定把整类原子排在前面，它们不提供新切分）。
pub fn compose_bench(trials: u64, k: usize, budget: usize) -> ComposeStats {
    assert!(k >= 1 && k <= 4, "两比特编码类号，k ≤ 4");
    let mut seed = 0x1234_5678u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let build = || {
        let n = 2 * k;
        let mut s = State::new(n);
        let block = |pred: fn(usize) -> bool| -> Mask {
            (0..k)
                .filter(|i| pred(*i))
                .map(|i| bit(2 * i) | bit(2 * i + 1))
                .fold(0, |a, b| a | b)
        };
        s.vocab = vec![block(|i| i % 2 == 1), block(|i| i / 2 % 2 == 1)];
        s.pos = (0..k).map(|i| bit(2 * i)).fold(0, |a, b| a | b);
        s.neg = (0..k).map(|i| bit(2 * i + 1)).fold(0, |a, b| a | b);
        s.hyps = s.enumerate_definable();
        s
    };
    let good: Vec<Mask> = (0..k).map(|i| bit(2 * i)).collect();
    let bad: Vec<Mask> = (0..k).map(|i| bit(2 * i) | bit(2 * i + 1)).collect();
    let fixed_order: Vec<Mask> = bad.iter().chain(good.iter()).copied().collect();

    let (mut f, mut r, mut g, mut fo, mut ro, mut go) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for _ in 0..trials {
        let s0 = build();
        let target = s0.pos;

        let (fs, fok) = steps_to_define(&s0, &fixed_order, target, budget);
        f += fs as u64;
        fo += u64::from(fok);

        let mut order = fixed_order.clone();
        for i in (1..order.len()).rev() {
            let j = (rnd() as usize) % (i + 1);
            order.swap(i, j);
        }
        let (rs, rok) = steps_to_define(&s0, &order, target, budget);
        r += rs as u64;
        ro += u64::from(rok);

        let greedy = greedy_order(&s0, &[good.clone(), bad.clone()].concat());
        let (gs, gok) = steps_to_define(&s0, &greedy, target, budget);
        g += gs as u64;
        go += u64::from(gok);
    }
    let d = trials.max(1) as f64;
    ComposeStats {
        trials,
        fixed: f as f64 / d,
        random: r as f64 / d,
        greedy: g as f64 / d,
        fixed_ok: fo as f64 / d,
        random_ok: ro as f64 / d,
        greedy_ok: go as f64 / d,
    }
}

/// 按覆盖贪心排出原子顺序（跳过 `V`-可定义的重组）。
fn greedy_order(s0: &State, pool: &[Mask]) -> Vec<Mask> {
    let mut s = s0.clone();
    let mut remaining: Vec<Mask> = pool.to_vec();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let Some(i) = best_cover(&s, &remaining) else { break };
        let a = remaining.remove(i);
        s.vocab.push(a);
        order.push(a);
    }
    order
}

/// 按顺序逐个加入原子，直到目标可定义；返回 (步数, 是否成功)。失败记 `budget + 1`。
fn steps_to_define(s0: &State, order: &[Mask], target: Mask, budget: usize) -> (usize, bool) {
    let mut s = s0.clone();
    if s.definable(target) {
        return (0, true);
    }
    for (i, &a) in order.iter().enumerate() {
        if i >= budget {
            break;
        }
        s.vocab.push(a);
        if s.definable(target) {
            return (i + 1, true);
        }
    }
    (budget + 1, false)
}

/// 同预算对照：同一批查询，固定顺序 / 随机顺序 / 逐步选择，各自要几轮。
pub fn selection_bench(trials: u64) -> (u64, f64, f64, f64) {
    let mut seed = 0x9e37_79b9u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let (mut fixed, mut random, mut selected, mut done) = (0u64, 0u64, 0u64, 0u64);
    for _ in 0..trials {
        let n = 6;
        let mut s = State::new(n);
        s.pos = bit(0);
        s.hyps = (0..8)
            .map(|_| {
                let mut h = bit(0);
                for x in 1..n {
                    if rnd() % 2 == 0 {
                        h |= bit(x);
                    }
                }
                h
            })
            .collect();
        s.hyps.sort_unstable();
        s.hyps.dedup();
        if s.phi() < 2 {
            continue;
        }
        let truth = s.hyps[(rnd() as usize) % s.hyps.len()];
        let ops: Vec<Op> = (1..n)
            .map(|x| Op::Evidence { x, member: truth & bit(x) != 0, cost: 1 })
            .collect();

        let mut a = s.clone();
        fixed += run_order(&mut a, &ops).cost as u64;

        let mut b = s.clone();
        let mut order = ops.clone();
        for i in (1..order.len()).rev() {
            let j = (rnd() as usize) % (i + 1);
            order.swap(i, j);
        }
        random += run_order(&mut b, &order).cost as u64;

        let mut c = s.clone();
        selected += run_selected(&mut c, &ops).expect("selected run").cost as u64;
        done += 1;
    }
    let d = done.max(1) as f64;
    (done, fixed as f64 / d, random as f64 / d, selected as f64 / d)
}

/// 按给定顺序跑到判定（`Φ = 1`）为止（对照用的固定/随机顺序）。
fn run_order(s: &mut State, ops: &[Op]) -> Outcome {
    let mut cost = 0u32;
    let mut ran = 0usize;
    for op in ops {
        if s.phi() == 1 {
            break;
        }
        if op.is_redundant(s) {
            continue;
        }
        if op.apply(s).is_err() {
            break;
        }
        cost += op.cost();
        ran += 1;
    }
    Outcome { phi: s.phi(), cost, ran, skipped: 0, skipped_cost: 0 }
}

/// 一步被 falsifier 击中，或前提不满足。
///
/// `Recombination` 是论文 T11 的直接落地；其余三种是工程层的前提（与证据矛盾 /
/// 与已确认答案相反 / 预算不够），论文没有对它们作断言。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falsified {
    /// 表征步骤里的谓词是当前词汇的布尔组合：T11，重组打不破撞车。
    Recombination,
    /// 新增候选与证据矛盾。
    Inconsistent,
    /// 候选在 `V ∪ {新原子}` 下仍不可定义：这一步把没表达的东西记成了已表达。
    NotExpressible,
    /// 探测对象已经被直接问过。
    AlreadyAsked,
    /// 探测对象所在等价类只有一个成员：没有"继承"可验证。
    NotInherited,
    /// 证据与已确认答案相反。
    Contradiction,
    /// 预算不够。
    OutOfBudget,
}

/// 状态分量的位标志，对应论文 Def 5 的 `rd` / `wr`。
pub mod comp {
    pub const H: u8 = 1;
    pub const V: u8 = 2;
    pub const E: u8 = 4;
    pub const B: u8 = 8;
}

/// 五类步骤。`cost` 用同一单位（查询轮 / 提议开销），否则不可加、不可调度。
#[derive(Clone, Debug)]
pub enum Op {
    /// 生成：把候选并进 `H`。warrant 是覆盖，falsifier 是不一致。
    Generate { cands: Vec<Mask>, cost: u32 },
    /// 闭包：把 `H` 过滤到与 `E` 一致。warrant 是推导，falsifier 无（删掉的都可查）。
    Close { cost: u32 },
    /// 表征：加入新原子 `pred`，并允许用它的候选进入 `H`。
    Represent { pred: Mask, cands: Vec<Mask>, cost: u32 },
    /// 取证：向 oracle 问一个对象。warrant 是 oracle 的回答。
    Evidence { x: usize, member: bool, cost: u32 },
    /// 探测：问一个"标签是从同类继承来的"成员，用来验证继承是否成立。
    /// 与 `Evidence` 的区别只在 warrant：对象必须没被直接问过，且所在的类不止一个成员。
    Probe { x: usize, member: bool, cost: u32 },
}

impl Op {
    /// 方向。写 `H` 的两类靠它分开。
    pub fn dir(&self) -> Dir {
        match self {
            Op::Generate { .. } => Dir::Up,
            Op::Close { .. } => Dir::Down,
            Op::Represent { .. } | Op::Evidence { .. } | Op::Probe { .. } => Dir::None,
        }
    }

    /// 读哪些分量（论文 Def 5 的 `rd`）。
    pub fn reads(&self) -> u8 {
        match self {
            Op::Generate { .. } | Op::Close { .. } | Op::Evidence { .. } | Op::Probe { .. } => {
                comp::H | comp::E
            }
            Op::Represent { .. } => comp::V | comp::H | comp::E,
        }
    }

    /// 写哪些分量（论文 Def 5 的 `wr`）。`Represent` 写 `V`，并因此写 `H`（新原子允许新候选进入）。
    pub fn writes(&self) -> u8 {
        match self {
            Op::Generate { .. } | Op::Close { .. } => comp::H,
            Op::Represent { .. } => comp::V | comp::H,
            Op::Evidence { .. } | Op::Probe { .. } => comp::E,
        }
    }

    pub fn class(&self) -> Class {
        match self {
            Op::Generate { .. } => Class::Generation,
            Op::Close { .. } => Class::Closure,
            Op::Represent { .. } => Class::Representation,
            Op::Evidence { .. } | Op::Probe { .. } => Class::Evidence,
        }
    }

    pub fn cost(&self) -> u32 {
        match self {
            Op::Generate { cost, .. }
            | Op::Close { cost }
            | Op::Represent { cost, .. }
            | Op::Evidence { cost, .. }
            | Op::Probe { cost, .. } => *cost,
        }
    }

    /// warranty：这一步凭什么可以被接受。T11 在这里落地。
    pub fn warrant(&self, s: &State) -> Result<(), Falsified> {
        match self {
            Op::Generate { .. } => {
                // 生成不需要 warrant：加进来的候选若与证据矛盾，闭包会把它删掉
                // （T8 只说 Φ 不降，不问候选好坏——好坏由 Φ 与代价来量）。
                Ok(())
            }
            Op::Close { .. } => Ok(()),
            Op::Represent { pred, cands, .. } => {
                if s.definable(*pred) {
                    return Err(Falsified::Recombination);
                }
                // 每个候选都必须在新词汇 V ∪ {pred} 下可定义，否则它只是被"记成"表达了。
                if cands.iter().any(|c| !s.definable_with(*pred, *c)) {
                    return Err(Falsified::NotExpressible);
                }
                if cands.iter().any(|c| !s.consistent(*c)) {
                    return Err(Falsified::Inconsistent);
                }
                Ok(())
            }
            Op::Evidence { x, member, .. } => {
                let b = bit(*x);
                if (*member && s.neg & b != 0) || (!*member && s.pos & b != 0) {
                    return Err(Falsified::Contradiction);
                }
                Ok(())
            }
            Op::Probe { x, member, .. } => {
                let b = bit(*x);
                if s.asked & b != 0 {
                    return Err(Falsified::AlreadyAsked);
                }
                let cls = s.classes().into_iter().find(|c| c & b != 0).unwrap_or(0);
                if cls.count_ones() < 2 {
                    return Err(Falsified::NotInherited);
                }
                if (*member && s.neg & b != 0) || (!*member && s.pos & b != 0) {
                    return Err(Falsified::Contradiction);
                }
                Ok(())
            }
        }
    }

    /// 这一步预计能去掉多少东西：生成/闭包数候选，表征看覆盖，取证看是否劈开。
    pub fn gain(&self, s: &State) -> usize {
        match self {
            Op::Generate { cands, .. } => cands
                .iter()
                .filter(|c| !s.hyps.contains(*c) && s.consistent(**c))
                .count(),
            Op::Close { .. } => s.hyps.iter().filter(|h| !s.consistent(**h)).count(),
            Op::Represent { .. } => self.coverage(s),
            Op::Evidence { .. } | Op::Probe { .. } => usize::from(self.is_discriminating(s)),
        }
    }

    /// 冗余：写不出任何新东西（状态逐位不变）。控制类永远冗余。
    ///
    /// 注意"冗余"与"无区分力"不是一回事：一个在所有活候选上答案都一样的取证
    /// **不降 Φ**，但它仍然写 `E`（约束之后的候选），因此不叫冗余。见 [`Op::is_discriminating`]。
    pub fn is_redundant(&self, s: &State) -> bool {
        match self {
            Op::Generate { cands, .. } => cands
                .iter()
                .all(|c| s.hyps.contains(c) || !s.consistent(*c)),
            Op::Close { .. } => s.hyps.iter().all(|h| s.consistent(*h)),
            Op::Represent { pred, cands, .. } => {
                s.vocab.contains(pred) && cands.iter().all(|c| s.hyps.contains(c))
            }
            Op::Evidence { x, .. } => {
                let b = bit(*x);
                s.pos & b != 0 || s.neg & b != 0 // 已经有答案，再问一遍是空写
            }
            Op::Probe { x, .. } => s.asked & bit(*x) != 0 || s.pos & bit(*x) != 0 || s.neg & bit(*x) != 0,
        }
    }

    /// 覆盖分数：这一步分开了多少对当前撞车的正反例。
    /// 它只是调度信号，不是 warrant——远期才有用的原子覆盖为 0，仍然合法。
    pub fn coverage(&self, s: &State) -> usize {
        match self {
            Op::Generate { .. } | Op::Close { .. } | Op::Evidence { .. } | Op::Probe { .. } => 0,
            Op::Represent { pred, .. } => s
                .colliding_pairs()
                .iter()
                .filter(|(x, y)| ((pred >> *x) & 1) != ((pred >> *y) & 1))
                .count(),
        }
    }

    /// 区分力：这一步会不会真的在活集上劈开（因此严格降 `Φ`）。
    /// 非区分性的取证不降 `Φ`，只加约束；这是论文里"每题都值同样一个比特"的可执行版本。
    pub fn is_discriminating(&self, s: &State) -> bool {
        match self {
            Op::Generate { cands, .. } => {
                cands.iter().any(|c| !s.hyps.contains(c) && s.consistent(*c))
            }
            Op::Close { .. } => s.hyps.iter().any(|h| !s.consistent(*h)),
            Op::Represent { pred, .. } => !s.definable(*pred),
            Op::Evidence { x, member, .. } | Op::Probe { x, member, .. } => {
                let b = bit(*x);
                s.live().iter().any(|h| (h & b != 0) != *member)
            }
        }
    }

    /// 执行一步。先过 warrant，再写状态。
    pub fn apply(&self, s: &mut State) -> Result<(), Falsified> {
        self.warrant(s)?;
        match self {
            Op::Generate { cands, .. } => {
                for c in cands {
                    if !s.hyps.contains(c) {
                        s.hyps.push(*c);
                    }
                }
                Ok(())
            }
            Op::Close { .. } => {
                let live = s.live();
                s.hyps = live;
                Ok(())
            }
            Op::Represent { pred, cands, .. } => {
                s.vocab.push(*pred);
                for c in cands {
                    if !s.hyps.contains(c) {
                        s.hyps.push(*c);
                    }
                }
                Ok(())
            }
            Op::Evidence { x, member, .. } | Op::Probe { x, member, .. } => {
                if *member {
                    s.pos |= bit(*x);
                } else {
                    s.neg |= bit(*x);
                }
                s.asked |= bit(*x);
                Ok(())
            }
        }
    }
}

/// 论文 Def 5 的两个关系：`o₂` 是否被 `o₁` 告知，以及 `o₂` 在 `o₁` 之后是否冗余。
pub fn informed_by(prev: &Op, next: &Op) -> bool {
    next.reads() & (prev.reads() | prev.writes()) != 0
}

/// `next` 在 `prev` 之后没有写出任何新分量（论文 Def 5 的冗余）。
pub fn redundant_after(prev: &Op, next: &Op) -> bool {
    next.writes() & !prev.writes() == 0
}

/// 在候选步骤里按 `gain / cost` 选一个（比值相等时取靠前的）。
/// warrant 不过或冗余的步骤直接跳过——这就是控制类的全部工作。
pub fn select(s: &State, ops: &[Op]) -> Option<usize> {
    let mut best: Option<(usize, u64, u64)> = None;
    for (i, op) in ops.iter().enumerate() {
        if op.is_redundant(s) || op.warrant(s).is_err() {
            continue;
        }
        let gain = op.gain(s) as u64;
        let cost = op.cost().max(1) as u64;
        match best {
            None => best = Some((i, gain, cost)),
            Some((_, bg, bc)) => {
                if gain * bc > bg * cost {
                    best = Some((i, gain, cost));
                }
            }
        }
    }
    best.map(|(i, _, _)| i)
}

/// 每一步都重选当前最值的一步。停机条件是**已判定且没有继承来的标签**：
/// `Φ = 1` 还不够，还要 `unprobed` 为空；否则优先排一个探测。
pub fn run_selected(s: &mut State, ops: &[Op]) -> Result<Outcome, Falsified> {
    let mut remaining: Vec<Op> = ops.to_vec();
    let mut cost = 0u32;
    let mut ran = 0usize;
    loop {
        if s.phi() == 1 && s.unprobed().is_empty() {
            break;
        }
        let idx = if s.phi() == 1 {
            // 已判定，但还有继承来的标签：只接受能验证它的探测。
            let Some(x) = s.unprobed().into_iter().next() else { break };
            match remaining
                .iter()
                .position(|o| matches!(o, Op::Probe { x: px, .. } if *px == x))
            {
                Some(i) => i,
                None => break,
            }
        } else {
            match select(s, &remaining) {
                Some(i) => i,
                None => break,
            }
        };
        let op = remaining.remove(idx);
        if cost.saturating_add(op.cost()) > s.budget {
            return Err(Falsified::OutOfBudget);
        }
        op.apply(s)?;
        cost += op.cost();
        ran += 1;
    }
    Ok(Outcome {
        phi: s.phi(),
        cost,
        ran,
        skipped: ops.len() - ran,
        skipped_cost: remaining.iter().map(|o| o.cost()).sum(),
    })
}

/// 一次运行的结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub phi: usize,
    pub cost: u32,
    pub ran: usize,
    pub skipped: usize,
    /// 被跳过步骤的代价之和（跳过的都是不改变状态的步骤）。
    pub skipped_cost: u32,
}

/// 按给定顺序跑一串算子。`skip_redundant` 为真时，控制类真的在省冗余步。
pub fn run(ops: &[Op], s: &mut State, skip_redundant: bool) -> Result<Outcome, Falsified> {
    let mut cost = 0u32;
    let mut ran = 0usize;
    let mut skipped = 0usize;
    let mut skipped_cost = 0u32;
    for op in ops {
        if skip_redundant && op.is_redundant(s) {
            skipped += 1;
            skipped_cost += op.cost();
            continue;
        }
        if cost.saturating_add(op.cost()) > s.budget {
            return Err(Falsified::OutOfBudget);
        }
        op.apply(s)?;
        cost += op.cost();
        ran += 1;
    }
    Ok(Outcome { phi: s.phi(), cost, ran, skipped, skipped_cost })
}

/// 一组演示：同一串步骤，固定顺序 vs 跳过冗余。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Demo {
    pub name: &'static str,
    pub fixed_cost: u32,
    pub scheduled_cost: u32,
    pub phi_before: usize,
    pub phi_after: usize,
    pub skipped: usize,
    /// 被跳过步骤的代价之和；应当等于 `fixed_cost - scheduled_cost`。
    pub skipped_cost: u32,
}

/// 三个演示场景。每个都只用论文里的量（Φ、代价、冗余步）。
pub fn demo() -> Vec<Demo> {
    let mut out = Vec::new();

    // A：没有冗余时，调度省不到任何东西（论文：控制只值冗余那部分）。
    {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        // 三个活候选，一问即可判定。
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![Op::Evidence { x: 1, member: true, cost: 1 }];
        out.push(run_pair("no-redundancy", &s, &ops));
    }

    // B：固定顺序里塞了重复闭包与非区分性提问；调度省掉的正是它们的代价。
    {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: false, cost: 1 },
            Op::Evidence { x: 3, member: false, cost: 1 },
        ];
        out.push(run_pair("repeated-closure", &s, &ops));
    }

    // C：状态已耗尽（Φ=0）。生成救不了，只有新原子能；固定顺序还要多付两次冗余。
    {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1); // 0 与 1 同类 → T2 撞车
        s.hyps = s.enumerate_definable();
        let ops = vec![
            Op::Generate { cands: vec![bit(2) | bit(3)], cost: 2 },
            Op::Close { cost: 1 },
            Op::Represent { pred: bit(0), cands: vec![bit(0), bit(0) | bit(2) | bit(3)], cost: 10 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: false, cost: 1 },
        ];
        out.push(run_pair("exhausted-needs-atom", &s, &ops));
    }

    out
}

/// 同一串步骤跑两遍：不跳过 / 跳过冗余。返回两者都到同一状态的那一行。
fn run_pair(name: &'static str, s: &State, ops: &[Op]) -> Demo {
    let before = s.phi();
    let fixed = run(ops, &mut s.clone(), false).expect("fixed run");
    let sched = run(ops, &mut s.clone(), true).expect("scheduled run");
    assert_eq!(fixed.phi, sched.phi, "两种调度必须到同一状态");
    assert_eq!(fixed.cost - sched.cost, sched.skipped_cost, "差额必须等于被跳过步骤的代价");
    Demo {
        name,
        phi_before: before,
        phi_after: fixed.phi,
        fixed_cost: fixed.cost,
        scheduled_cost: sched.cost,
        skipped: sched.skipped,
        skipped_cost: sched.skipped_cost,
    }
}

/// 随机自检：生成 `trials` 条随机调度，验证两件事——
/// 跳过冗余不改变最终 `Φ`；省下的钱恰好等于被跳过步骤的代价。
///
/// 返回 `(trials, violations)`。二进制每次跑 `ops` 都会执行它，因此论文 §4 的
/// "控制不动势"在运行时也被检查一次，不只在测试里。
pub fn audit(trials: u64) -> (u64, Vec<String>) {
    let mut seed = 0x5eed_1234u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        seed >> 33
    };
    let n = 6;
    let mut violations = Vec::new();
    for t in 0..trials {
        let mut s = State::new(n);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.hyps = s.enumerate_definable();
        let mut answered = s.pos | s.neg;
        let mut ops = Vec::new();
        for _ in 0..8 {
            match rnd() % 3 {
                0 => ops.push(Op::Close { cost: 1 }),
                1 => {
                    let m = rnd() & State::full(n);
                    ops.push(Op::Generate { cands: vec![m], cost: 2 });
                }
                _ => {
                    let free: Vec<usize> =
                        (0..n).filter(|&x| answered & bit(x) == 0).collect();
                    if free.is_empty() {
                        ops.push(Op::Close { cost: 1 });
                    } else {
                        let x = free[(rnd() as usize) % free.len()];
                        answered |= bit(x);
                        ops.push(Op::Evidence { x, member: rnd() % 2 == 0, cost: 1 });
                    }
                }
            }
        }
        match (run(&ops, &mut s.clone(), false), run(&ops, &mut s.clone(), true)) {
            (Ok(a), Ok(b)) => {
                if a.phi != b.phi {
                    violations.push(format!("trial {t}: phi {} != {}", a.phi, b.phi));
                }
                if a.cost - b.cost != b.skipped_cost {
                    violations.push(format!("trial {t}: cost identity broken"));
                }
            }
            (Err(_), Err(_)) => {}
            _ => violations.push(format!("trial {t}: one schedule errored, the other did not")),
        }
    }
    (trials, violations)
}

/// 当前词汇下"被识别"的外延：所有含正例的等价类之并。
pub fn implied_extension(s: &State) -> Mask {
    let mut out = 0u64;
    for c in s.classes() {
        if c & s.pos != 0 {
            out |= c;
        }
    }
    out
}

/// 继承标签的演示：早停给出错误外延，探测后补一次表征才正确。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeDemo {
    pub early_phi: usize,
    pub early_queries: u32,
    pub early_correct: bool,
    pub verified_queries: u32,
    pub verified_cost: u32,
    pub verified_correct: bool,
}

/// 真值是 `{0}`（4 个对象、空词汇下不可定义）。
pub fn probe_demo() -> ProbeDemo {
    let truth = bit(0);

    // 早停：只问 0，Φ=1，但外延是整类 → 错。
    let mut early = State::new(4);
    early.hyps = early.enumerate_definable();
    Op::Evidence { x: 0, member: true, cost: 1 }.apply(&mut early).expect("ask 0");

    // 验证版：探测 → 状态退回不足 → 加原子 → 继续探测到没有继承标签。
    let mut verified = early.clone();
    let ops = vec![
        Op::Probe { x: 1, member: false, cost: 1 },
        Op::Represent { pred: bit(0), cands: vec![bit(0)], cost: 10 },
        Op::Probe { x: 2, member: false, cost: 1 },
        Op::Probe { x: 3, member: false, cost: 1 },
    ];
    let out = run_selected(&mut verified, &ops).expect("verified run");

    ProbeDemo {
        early_phi: early.phi(),
        early_queries: 1,
        early_correct: implied_extension(&early) == truth,
        verified_queries: 1 + out.ran as u32,
        verified_cost: 1 + out.cost,
        verified_correct: implied_extension(&verified) == truth,
    }
}

/// 演示结果的 JSON（与其他子命令一致：机器可读）。
pub fn demo_json() -> String {
    let rows: Vec<String> = demo()
        .iter()
        .map(|d| {
            format!(
                "{{\"name\":\"{}\",\"phi_before\":{},\"phi_after\":{},\"fixed_cost\":{},\"scheduled_cost\":{},\"skipped\":{},\"skipped_cost\":{}}}",
                d.name, d.phi_before, d.phi_after, d.fixed_cost, d.scheduled_cost, d.skipped, d.skipped_cost
            )
        })
        .collect();
    let (trials, violations) = audit(200);
    let (sel_n, fixed, random, selected) = selection_bench(200);
    let c = compose_bench(200, 2, 2);
    let p = probe_demo();
    format!(
        "{{\"claim\":\"scheduling saves exactly the redundant spend\",\"audit\":{{\"trials\":{},\"violations\":{}}},\"selection\":{{\"trials\":{},\"fixed_cost\":{:.2},\"random_cost\":{:.2},\"selected_cost\":{:.2}}},\"compose\":{{\"k\":2,\"budget\":2,\"greedy\":{:.2},\"random\":{:.2},\"fixed\":{:.2},\"greedy_ok\":{:.2},\"random_ok\":{:.2},\"fixed_ok\":{:.2}}},\"probe\":{{\"early_phi\":{},\"early_queries\":{},\"early_correct\":{},\"verified_queries\":{},\"verified_cost\":{},\"verified_correct\":{}}},\"scenarios\":[{}]}}",
        trials,
        violations.len(),
        sel_n,
        fixed,
        random,
        selected,
        c.greedy,
        c.random,
        c.fixed,
        c.greedy_ok,
        c.random_ok,
        c.fixed_ok,
        p.early_phi,
        p.early_queries,
        p.early_correct,
        p.verified_queries,
        p.verified_cost,
        p.verified_correct,
        rows.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state3() -> State {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        s
    }

    /// T8：生成只加候选，Φ 永不下降。
    #[test]
    fn t8_generation_never_reduces_phi() {
        let s = state3();
        let before = s.phi();
        let mut t = s.clone();
        Op::Generate { cands: vec![bit(0) | bit(3)], cost: 2 }.apply(&mut t).unwrap();
        assert!(t.phi() >= before);
        // 加一个不一致的候选：它一进来就是死的，Φ 仍然不降。
        let mut u = s.clone();
        Op::Generate { cands: vec![0], cost: 1 }.apply(&mut u).unwrap();
        assert_eq!(u.phi(), before);
    }

    /// T9：闭包两次 = 闭包一次；第二次是冗余步。
    #[test]
    fn t9_closure_is_idempotent() {
        let mut s = state3();
        // 塞一个不一致候选进去（直接放，模拟生成算子写坏了）。
        s.hyps.push(bit(2) | bit(3));
        let mut once = s.clone();
        Op::Close { cost: 1 }.apply(&mut once).unwrap();
        let phi_once = once.phi();
        Op::Close { cost: 1 }.apply(&mut once).unwrap();
        assert_eq!(once.phi(), phi_once);
        assert!(Op::Close { cost: 1 }.is_redundant(&once));
    }

    /// T10：能在活集上劈开的一问严格降 Φ；劈不开的不降 Φ（但它仍然写 E，所以不算冗余）。
    #[test]
    fn t10_evidence_is_the_only_strict_reducer() {
        let s = state3();
        let before = s.phi();
        let mut t = s.clone();
        Op::Evidence { x: 1, member: true, cost: 1 }.apply(&mut t).unwrap();
        assert!(t.phi() < before);

        // 所有活候选都含 1 且 1 还没问过：问 1 不改变 Φ，但它是新证据（写 E），不是冗余步。
        let mut s2 = State::new(4);
        s2.pos = bit(0);
        s2.hyps = vec![
            bit(0) | bit(1),
            bit(0) | bit(1) | bit(2),
            bit(0) | bit(1) | bit(3),
        ];
        let q = Op::Evidence { x: 1, member: true, cost: 1 };
        assert!(!q.is_discriminating(&s2));
        assert!(!q.is_redundant(&s2));
        let mut u = s2.clone();
        q.apply(&mut u).unwrap();
        assert_eq!(u.phi(), s2.phi());
        // 同一个问题再问一次才是冗余。
        assert!(q.is_redundant(&u));
    }

    /// T11：由词汇布尔组合出来的谓词被 warrant 直接拒收。
    #[test]
    fn t11_recombination_is_rejected() {
        let s = State::new(4);
        // 词汇只有一个谓词 p = {0,1}；not(p) 仍是它的布尔组合。
        let mut v = s.clone();
        v.vocab = vec![bit(0) | bit(1)];
        let not_p = State::full(4) & !(bit(0) | bit(1));
        let op = Op::Represent { pred: not_p, cands: vec![not_p], cost: 5 };
        assert_eq!(op.warrant(&v), Err(Falsified::Recombination));
        // 新原子（不在布尔闭包里）可以过。
        let atom = bit(0) | bit(2);
        assert!(!v.definable(atom));
        assert_eq!(Op::Represent { pred: atom, cands: vec![atom], cost: 5 }.warrant(&v), Ok(()));
    }

    /// 推论前半：Φ=0 时生成/闭包/取证都救不了。
    #[test]
    fn exhausted_state_survives_without_a_new_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        s.hyps = s.enumerate_definable();
        assert_eq!(s.phi(), 0);
        let ops = vec![
            Op::Generate { cands: vec![bit(0) | bit(1) | bit(2)], cost: 1 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: true, cost: 1 },
        ];
        let out = run(&ops, &mut s, false).unwrap();
        assert_eq!(out.phi, 0);
    }

    /// 推论后半：新原子进来后 Φ 才可能大于 0。
    #[test]
    fn a_new_atom_can_raise_phi() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        s.hyps = s.enumerate_definable();
        let before = s.phi();
        Op::Represent { pred: bit(0), cands: vec![bit(0)], cost: 5 }.apply(&mut s).unwrap();
        assert!(s.phi() > before);
    }

    /// 远期才有用的原子（当前覆盖 0）仍然合法：覆盖只作调度分数。
    #[test]
    fn represent_allows_a_future_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        assert_eq!(s.colliding_pairs(), vec![(0, 1)]);
        // {2} 切开了类 {2,3}（不可定义），但分不开撞车的 (0,1)。
        let op = Op::Represent { pred: bit(2), cands: vec![], cost: 5 };
        assert_eq!(op.warrant(&s), Ok(()));
        assert_eq!(op.coverage(&s), 0, "当前零覆盖，但它是合法的一步");
    }

    /// 候选必须在新词汇 V ∪ {pred} 下可定义：否则 Φ=1 是假 decided。
    #[test]
    fn represent_rejects_candidates_outside_new_vocabulary() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0) | bit(2);
        s.neg = bit(1) | bit(3);
        // 候选 {0,2} 在 V' = {V, {0}} 下仍切开 {2,3} → 不可定义。
        let bad = Op::Represent { pred: bit(0), cands: vec![bit(0) | bit(2)], cost: 5 };
        assert_eq!(bad.warrant(&s), Err(Falsified::NotExpressible));

        // 同一原子，若证据只要求含 0 不含 1，则 {0} 与 {0,2,3} 都合法。
        let mut t = State::new(4);
        t.vocab = vec![bit(0) | bit(1)];
        t.pos = bit(0);
        t.neg = bit(1);
        let ok = Op::Represent {
            pred: bit(0),
            cands: vec![bit(0), bit(0) | bit(2) | bit(3)],
            cost: 5,
        };
        assert_eq!(ok.warrant(&t), Ok(()));
    }

    /// `(wr, dir)` 给四类签名四个不同值；只看 `wr` 时生成与闭包撞在一起。
    /// 这就是"为什么是 4 不是 3、不是 5"的机器版本：数类别要数到方向这一位。
    #[test]
    fn type_with_direction_separates_the_classes() {
        let ops = [
            Op::Generate { cands: vec![], cost: 1 },
            Op::Close { cost: 1 },
            Op::Represent { pred: 0, cands: vec![], cost: 1 },
            Op::Evidence { x: 0, member: true, cost: 1 },
        ];
        // 只看 wr：生成与闭包同签名。
        assert_eq!(ops[0].writes(), ops[1].writes());
        // 加上方向：四个签名互不相同。
        let mut seen: Vec<(u8, Dir)> = Vec::new();
        for op in &ops {
            let sig = (op.writes(), op.dir());
            assert!(!seen.contains(&sig), "{:?} 与已有签名重复", op.class());
            seen.push(sig);
        }
        assert_eq!(seen.len(), 4);
    }

    /// 关系投影出一元谓词，从而进入词汇并分开撞车对。
    #[test]
    fn relations_enter_through_lifts() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0);
        s.neg = bit(1);
        // 关系 r: 0→2, 1→3。
        s.rels = vec![vec![bit(2), bit(3), 0, 0]];
        assert_eq!(s.image(0, bit(0)), bit(2));
        assert_eq!(s.preimage(0, bit(2)), bit(0));
        // 前像 {0} 在旧词汇下不可定义，却恰好分开 (0,1)。
        let lift = s.preimage(0, bit(2));
        assert_eq!(lift, bit(0));
        assert!(!s.definable(lift));
        let op = Op::Represent { pred: lift, cands: vec![lift], cost: 5 };
        assert_eq!(op.warrant(&s), Ok(()));
        assert_eq!(op.coverage(&s), 1);
    }

    /// 关系的像可以复合（两步可达）。
    #[test]
    fn relational_images_compose() {
        let mut s = State::new(4);
        s.rels = vec![vec![bit(1), bit(2), bit(3), 0]];
        assert_eq!(s.image(0, bit(0)), bit(1));
        assert_eq!(s.image(0, s.image(0, bit(0))), bit(2));
        assert_eq!(s.preimage(0, bit(2)), bit(1));
        assert_eq!(s.preimage(0, bit(0)), 0);
    }

    /// MDL：用当前词汇的合取描述一个集合需要几个文字。
    #[test]
    fn mdl_counts_literals() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1), bit(0)];
        assert_eq!(s.mdl(bit(0)), Some(1));
        assert_eq!(s.mdl(bit(1)), Some(2));
        assert_eq!(s.mdl(bit(2) | bit(3)), Some(1));
        assert_eq!(s.mdl(bit(0) | bit(2)), None);
    }

    /// 选择：可劈开的问题优先于劈不开的。
    #[test]
    fn selection_prefers_a_discriminating_question() {
        let mut s = State::new(5);
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![
            Op::Evidence { x: 4, member: false, cost: 1 }, // 所有活候选都不含 4：不劈开
            Op::Evidence { x: 1, member: true, cost: 1 },  // 劈开
        ];
        assert_eq!(select(&s, &ops), Some(1));
    }

    /// 选择：覆盖更多撞车对的原子优先。
    #[test]
    fn selection_prefers_higher_coverage_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0) | bit(2);
        s.neg = bit(1) | bit(3);
        let ops = vec![
            Op::Represent { pred: bit(0), cands: vec![], cost: 5 },
            Op::Represent { pred: bit(0) | bit(2), cands: vec![], cost: 5 },
        ];
        assert_eq!(ops[1].coverage(&s), 2);
        assert_eq!(select(&s, &ops), Some(1));
    }

    /// 逐步重选能走到判定，且不会重复执行。
    #[test]
    fn run_selected_reaches_a_decision() {
        let mut s = State::new(5);
        s.pos = bit(0);
        s.hyps = vec![bit(0) | bit(1), bit(0) | bit(2), bit(0) | bit(3)];
        let ops = vec![
            Op::Evidence { x: 4, member: false, cost: 1 },
            Op::Evidence { x: 1, member: true, cost: 1 },
        ];
        let out = run_selected(&mut s, &ops).expect("selected run");
        assert_eq!(out.phi, 1);
        assert_eq!(out.ran, 1, "第一个问题就能判定");
        assert_eq!(out.cost, 1);
    }

    /// 单个原子就足够时，覆盖最大者就是充分的那个（T7）：它必分开全部撞车对。
    #[test]
    fn best_cover_picks_the_sufficient_atom() {
        let mut s = State::new(4);
        s.vocab = vec![bit(0) | bit(1)];
        s.pos = bit(0) | bit(2);
        s.neg = bit(1) | bit(3);
        // 撞车对 (0,1) 与 (2,3)；目标是 S = {0,2}。
        let decoy = bit(0);
        let good = bit(0) | bit(2);
        let pool = vec![decoy, good];
        assert_eq!(best_cover(&s, &pool), Some(1));
        // 用它做一次表征，Φ 直接到 1。
        let op = Op::Represent { pred: good, cands: vec![good], cost: 5 };
        assert_eq!(op.warrant(&s), Ok(()));
        op.apply(&mut s).expect("sufficient atom");
        assert_eq!(s.phi(), 1);
    }

    /// 同预算对照：逐步选择不应比固定顺序更贵（合成 8 候选状态，确定性种子）。
    #[test]
    fn selection_bench_beats_fixed_order() {
        let (n, fixed, random, selected) = selection_bench(50);
        assert!(n > 0);
        assert!(selected <= fixed, "selected {selected} vs fixed {fixed}");
        assert!(selected <= random + 0.5, "selected {selected} vs random {random}");
    }

    /// 需要两个原子时：贪心覆盖两个就够，固定顺序（整类原子在前）在预算内失败。
    #[test]
    fn compose_bench_greedy_composes() {
        let st = compose_bench(50, 2, 2);
        assert_eq!(st.greedy, 2.0);
        assert_eq!(st.greedy_ok, 1.0);
        assert!(st.greedy < st.random, "greedy {} vs random {}", st.greedy, st.random);
        assert_eq!(st.fixed_ok, 0.0);
    }

    /// 探测：未验证的成员可以被问；问过之后状态可能从"已判定"退回"不足"。
    #[test]
    fn probe_can_reopen_a_decided_state() {
        let mut s = State::new(3);
        s.pos = bit(0);
        s.asked = bit(0); // 0 是直接问过的
        s.hyps = vec![bit(0) | bit(1) | bit(2)];
        assert_eq!(s.phi(), 1);
        assert_eq!(s.unprobed(), vec![1, 2]);
        let op = Op::Probe { x: 1, member: false, cost: 1 };
        assert_eq!(op.warrant(&s), Ok(()));
        op.apply(&mut s).expect("probe");
        assert_eq!(s.phi(), 0, "继承来的标签被证伪，状态退回不足");
        assert_eq!(s.asked, bit(0) | bit(1));
    }

    /// 探测的 warrant：没直接问过，且类里不止一个成员。
    #[test]
    fn probe_requires_an_unasked_member_of_a_nontrivial_class() {
        let mut s = State::new(3);
        s.vocab = vec![bit(0)]; // 类 {0} 与 {1,2}
        s.pos = bit(0);
        Op::Evidence { x: 0, member: true, cost: 1 }.apply(&mut s).unwrap();
        assert_eq!(Op::Probe { x: 0, member: true, cost: 1 }.warrant(&s), Err(Falsified::AlreadyAsked));
        assert_eq!(Op::Probe { x: 1, member: true, cost: 1 }.warrant(&s), Ok(()));

        let mut t = State::new(3);
        t.vocab = vec![bit(0)];
        assert_eq!(Op::Probe { x: 0, member: true, cost: 1 }.warrant(&t), Err(Falsified::NotInherited));
    }

    /// 普通取证也会记下"直接问过"，单成员类不计入未验证集合。
    #[test]
    fn evidence_marks_asked_and_unprobed_skips_singletons() {
        let mut s = State::new(3);
        s.vocab = vec![bit(0)];
        s.pos = bit(0);
        assert_eq!(s.unprobed(), vec![1, 2]);
        Op::Evidence { x: 2, member: true, cost: 1 }.apply(&mut s).unwrap();
        assert_eq!(s.asked, bit(2));
        assert_eq!(s.unprobed(), vec![1]);
    }

    /// 停机规则：Φ=1 但还有继承标签时，先探测；探测证伪就退回不足。
    #[test]
    fn run_selected_probes_before_stopping() {
        let make = || {
            let mut s = State::new(2);
            s.pos = bit(0);
            s.asked = bit(0);
            s.hyps = vec![bit(0) | bit(1)];
            s
        };
        let mut s = make();
        assert_eq!(s.phi(), 1);
        assert_eq!(s.unprobed(), vec![1]);

        let out = run_selected(&mut s, &[Op::Probe { x: 1, member: false, cost: 1 }]).unwrap();
        assert_eq!(out.ran, 1);
        assert_eq!(out.phi, 0, "探测证伪了继承来的标签");

        let mut t = make();
        let out = run_selected(&mut t, &[Op::Probe { x: 1, member: true, cost: 1 }]).unwrap();
        assert_eq!(out.ran, 1);
        assert_eq!(out.phi, 1);
        assert!(t.unprobed().is_empty(), "探过之后没有继承标签了");

        // 没有可用的探测算子：按 Φ=1 停，但未验证成员还在（调用方需知道）。
        let mut u = make();
        let out = run_selected(&mut u, &[]).unwrap();
        assert_eq!(out.ran, 0);
        assert_eq!(u.unprobed(), vec![1]);
    }

    /// 演示：Φ=1 早停给出错误外延，探测后正确。
    #[test]
    fn probe_demo_shows_the_gap() {
        let p = probe_demo();
        assert_eq!(p.early_phi, 1);
        assert!(!p.early_correct, "早停的外延应当错");
        assert!(p.verified_correct, "探测之后应当对");
        assert!(p.verified_cost > 1);
    }

    /// 控制不是一步，而是 [`run`] 的跳过模式：它写 `B`，不动 `Φ`。
    #[test]
    fn control_is_the_scheduler_not_a_step() {
        assert_eq!(Class::Control.writes(), "B");
        let s = state3();
        let ops = vec![Op::Close { cost: 1 }, Op::Close { cost: 1 }];
        let fixed = run(&ops, &mut s.clone(), false).unwrap();
        let sched = run(&ops, &mut s.clone(), true).unwrap();
        assert_eq!(fixed.phi, sched.phi);
    }

    /// 论文 Def 5 的两个关系：被告知 / 冗余。
    #[test]
    fn type_relations_follow_definition() {
        let close = Op::Close { cost: 1 };
        let repr = Op::Represent { pred: bit(0), cands: vec![], cost: 5 };
        let ev = Op::Evidence { x: 1, member: true, cost: 1 };
        let gen = Op::Generate { cands: vec![bit(0)], cost: 2 };
        assert!(informed_by(&close, &repr), "闭包写 H，表征读 H");
        assert!(!redundant_after(&close, &repr), "表征写出新分量 V");
        assert!(redundant_after(&close, &close), "闭包写不出闭包没写过的东西");
        assert!(!redundant_after(&gen, &ev), "取证写 E，生成没写过 E");
        assert!(informed_by(&ev, &gen), "取证写 E，生成读 E");
        assert_eq!(repr.writes(), comp::V | comp::H);
        assert_eq!(ev.writes(), comp::E);
    }

    /// `is_discriminating` 必须等价于"应用之后 Φ 严格下降"。
    #[test]
    fn discriminating_iff_phi_drops() {
        let mut seed = 7u64;
        let mut rnd = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            seed >> 33
        };
        for _ in 0..200 {
            let n = 6;
            let mut s = State::new(n);
            s.vocab = vec![bit(0) | bit(1)];
            s.pos = bit(0);
            s.neg = bit(2);
            s.hyps = s.enumerate_definable();
            for _ in 0..3 {
                let m = rnd() & State::full(n);
                if !s.hyps.contains(&m) {
                    s.hyps.push(m);
                }
            }
            let free: Vec<usize> =
                (0..n).filter(|&x| s.pos & bit(x) == 0 && s.neg & bit(x) == 0).collect();
            if free.is_empty() {
                continue;
            }
            let x = free[(rnd() as usize) % free.len()];
            let op = Op::Evidence { x, member: rnd() % 2 == 0, cost: 1 };
            let before = s.phi();
            let mut t = s.clone();
            op.apply(&mut t).unwrap();
            assert_eq!(op.is_discriminating(&s), t.phi() < before, "x={x}");
        }
    }

    /// 调度省下的钱必须正好等于被跳过步骤的代价（论文的"控制只值冗余"）。
    #[test]
    fn scheduling_saves_exactly_the_redundant_spend() {
        let s = state3();
        // 三步闭包 + 两步取证，其中两次闭包冗余。
        let ops = vec![
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Close { cost: 1 },
            Op::Evidence { x: 2, member: false, cost: 1 },
            Op::Evidence { x: 3, member: false, cost: 1 },
        ];
        let fixed = run(&ops, &mut s.clone(), false).unwrap();
        let sched = run(&ops, &mut s.clone(), true).unwrap();
        assert_eq!(fixed.phi, sched.phi, "两种顺序必须到同一状态");
        assert_eq!(fixed.cost - sched.cost, sched.skipped as u32);
    }

    /// 预算耗尽时返回 falsifier，而不是静默继续。
    #[test]
    fn budget_is_enforced() {
        let mut s = state3();
        s.budget = 1;
        let ops = vec![Op::Evidence { x: 1, member: true, cost: 2 }];
        assert_eq!(run(&ops, &mut s, false), Err(Falsified::OutOfBudget));
    }

    /// 每个演示行都必须满足：固定代价 − 调度代价 = 被跳过步骤的代价。
    #[test]
    fn demo_cost_identity_holds() {
        let rows = demo();
        assert_eq!(rows.len(), 3);
        for d in rows {
            assert_eq!(d.fixed_cost - d.scheduled_cost, d.skipped_cost, "{}", d.name);
            assert!(d.scheduled_cost <= d.fixed_cost, "{}", d.name);
        }
    }

    /// 随机调度：跳过冗余永远不改变最终状态，省下的钱恰好等于被跳过步骤的代价。
    /// 与二进制 `ops` 跑的是同一个 [`audit`]。
    #[test]
    fn random_schedules_preserve_state_and_accounting() {
        let (trials, violations) = audit(200);
        assert_eq!(trials, 200);
        assert!(violations.is_empty(), "{violations:?}");
    }
}
