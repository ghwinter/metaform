//! 查询驱动的表征变更：核心状态机。
//!
//! 这是 `research/01-formal-core.md` 与 `research/lean/QueryDriven.lean` 的可执行对应物。
//! 三个概念一一对应：
//!
//! | 论文 | 这里 |
//! |---|---|
//! | 签名 `sig_V(x)` | [`Explorer::signature`] |
//! | 不可分辨 / 等价类 | [`Explorer::classes`] |
//! | 候选集 `F_V(Q)` 是否为空（T2） | [`Explorer::status`] 返回 [`Status::Insufficient`] |
//! | 扩词表 | [`Explorer::propose`] |
//!
//! 设计上刻意保持"笨"：不内置搜索、不内置评分。控制器只有一条规则——
//! 候选集空就换词，换完再问。域 A 的成本实验表明，更聪明的调度只值几个百分点
//! （`research/exp/RESULTS.md` §3.6）。

use std::collections::HashMap;

/// 多算子定型（论文 §4）：五类步骤共用一个度量 `Φ`，逐类验证它能改变什么。
pub mod operators;

/// 对象集合的位掩码。64 个对象以内够用；需要更多就换 `bitvec`。
pub type Mask = u64;

/// 第 `i` 个对象的位。
pub fn bit(i: usize) -> Mask {
    1 << i
}

/// 一个谓词：名字 + 外延。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Predicate {
    pub name: String,
    pub ext: Mask,
}

impl Predicate {
    pub fn new(name: impl Into<String>, ext: Mask) -> Self {
        Self { name: name.into(), ext }
    }
}

/// 词汇：一小组谓词。顺序决定签名的坐标顺序。
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    pub preds: Vec<Predicate>,
}

impl Vocabulary {
    pub fn with(preds: Vec<Predicate>) -> Self {
        Self { preds }
    }

    pub fn len(&self) -> usize {
        self.preds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.preds.is_empty()
    }

    pub fn add(&mut self, p: Predicate) {
        self.preds.push(p);
    }
}

/// 探索状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// 存在正例与反例落在同一等价类：当前词汇已被证明不够（T2）。
    Insufficient,
    /// 每个等价类都已被标注覆盖：目标的外延完全确定。
    Identified,
    /// 还有一些类没被覆盖，继续提问。
    Undetermined,
}

/// 一次探索：固定对象域，维护词汇与已标注样本。
#[derive(Clone, Debug)]
pub struct Explorer {
    pub n: usize,
    pub vocab: Vocabulary,
    pub pos: Mask,
    pub neg: Mask,
    /// 每个对象的票数 `(yes, no)`。噪声 oracle 下重复提问靠它聚合。
    pub votes: Vec<(u8, u8)>,
    /// 被直接问过的对象。等价类的标签会"继承"给没问过的成员，这里记下哪些是真的问过。
    pub asked: Mask,
}

impl Explorer {
    pub fn new(n: usize, vocab: Vocabulary) -> Self {
        assert!(n <= 64, "Mask 是 u64，最多 64 个对象");
        Self { n, vocab, pos: 0, neg: 0, votes: vec![(0, 0); n], asked: 0 }
    }

    /// `x` 在当前词汇下的签名。
    pub fn signature(&self, x: usize) -> Vec<bool> {
        let b = bit(x);
        self.vocab.preds.iter().map(|p| p.ext & b != 0).collect()
    }

    /// 按签名分组，返回每个等价类的对象掩码。
    pub fn classes(&self) -> Vec<Mask> {
        let mut groups: HashMap<Vec<bool>, Mask> = HashMap::new();
        for x in 0..self.n {
            *groups.entry(self.signature(x)).or_insert(0) |= bit(x);
        }
        groups.into_values().collect()
    }

    /// 当前状态（票差阈值 1，即"多数即可"）。
    pub fn status(&self) -> Status {
        self.status_with(1)
    }

    /// 带票差阈值的状态。
    ///
    /// `theta` 是"确证"所需的最小票差：只有 `yes - no >= theta` 的对象算强正例，
    /// `no - yes >= theta` 算强反例。噪声越大，`theta` 应越大（见 RESULTS §3.11：
    /// 完整循环里 θ=1 在 ε=0.1 时只有 25% 成功，θ=3 是 98%）。
    ///
    /// 三条规则（与 `research/exp/noisy_loop.py` 对齐）：
    /// 1. 撞车优先：某类同时有强正例与强反例 → `Insufficient`。
    /// 2. 弱票阻塞：某类里还有"投过票但未达 θ"的成员 → 不算已决，返回 `Undetermined`。
    ///    否则"一个强标签 + 一个弱反对"会被静默接受，给出错误外延。
    /// 3. 覆盖：每个类都要有强标签，否则 `Undetermined`。
    pub fn status_with(&self, theta: u8) -> Status {
        let (sp, sn) = self.strong(theta);
        let classes = self.classes();
        if classes.iter().any(|c| c & sp != 0 && c & sn != 0) {
            return Status::Insufficient;
        }
        let weak = self.voted_mask() & !(sp | sn);
        if classes.iter().any(|c| c & weak != 0) {
            return Status::Undetermined;
        }
        let known = sp | sn;
        if classes.iter().any(|c| c & known == 0) {
            return Status::Undetermined;
        }
        Status::Identified
    }

    /// 投过票的对象（不论强弱）。
    pub fn voted_mask(&self) -> Mask {
        self.votes
            .iter()
            .enumerate()
            .filter(|(_, &(y, n))| y + n > 0)
            .fold(0u64, |acc, (x, _)| acc | bit(x))
    }

    /// 需要补票的对象：投过票、但在阈值 `theta` 下还不算强标签。
    pub fn pending(&self, theta: u8) -> Vec<usize> {
        let (sp, sn) = self.strong(theta);
        let weak = self.voted_mask() & !(sp | sn);
        (0..self.n).filter(|&x| weak & bit(x) != 0).collect()
    }

    /// 由票数直接估 oracle 的错误率：每个对象的少数票占比，再对投过票的对象取均值。
    ///
    /// 票数少时这个估计偏低（r=1 时只能是 0 或 1），所以它适合"跑了几轮之后"用，
    /// 不适合第一轮。
    pub fn implied_epsilon(&self) -> f64 {
        let mut sum = 0.0;
        let mut n = 0u32;
        for &(yes, no) in &self.votes {
            let total = yes as u32 + no as u32;
            if total > 0 {
                sum += (yes.min(no) as f64) / (total as f64);
                n += 1;
            }
        }
        if n == 0 {
            0.0
        } else {
            sum / n as f64
        }
    }

    /// 按估出来的 ε 给一个 θ（`RESULTS.md` §3.11 的经验规则）。
    pub fn recommended_theta(&self) -> u8 {
        let e = self.implied_epsilon();
        if e <= 0.10 {
            3
        } else if e >= 0.20 {
            5
        } else {
            4
        }
    }

    /// 在当前阈值下"强正例 / 强反例"的掩码。
    pub fn strong(&self, theta: u8) -> (Mask, Mask) {
        let mut sp = 0u64;
        let mut sn = 0u64;
        for (x, &(yes, no)) in self.votes.iter().enumerate() {
            if yes > no && yes - no >= theta {
                sp |= bit(x);
            } else if no > yes && no - yes >= theta {
                sn |= bit(x);
            }
        }
        (sp, sn)
    }

    /// 下一个该问的对象：先补弱票，再问未覆盖类的代表。
    pub fn next_question(&self) -> Option<usize> {
        self.next_question_with(1)
    }

    /// 带阈值的下一个问题：先补弱票（规则 2），再问未覆盖类。
    pub fn next_question_with(&self, theta: u8) -> Option<usize> {
        if let Some(x) = self.pending(theta).into_iter().next() {
            return Some(x);
        }
        let (sp, sn) = self.strong(theta);
        let known = sp | sn;
        self.classes()
            .into_iter()
            .find(|c| c & known == 0)
            .map(|c| c.trailing_zeros() as usize)
    }

    /// 记录一次成员查询的答案（单票）。
    pub fn answer(&mut self, x: usize, is_member: bool) {
        if is_member {
            self.answer_votes(x, 1, 0);
        } else {
            self.answer_votes(x, 0, 1);
        }
    }

    /// 记录一次带票数的回答。同一个对象可以多次投票。
    pub fn answer_votes(&mut self, x: usize, yes: u8, no: u8) {
        assert!(x < self.n);
        self.votes[x].0 = self.votes[x].0.saturating_add(yes);
        self.votes[x].1 = self.votes[x].1.saturating_add(no);
        self.asked |= bit(x);
        self.sync_plain_labels();
    }

    /// 没被直接问过的成员：多成员等价类里、不在 `asked` 里的对象。
    /// 这些对象的标签是从同类成员"继承"来的，可能错——停机前应当各探一个。
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
        out.sort_unstable(); // classes() 走 HashMap，顺序不稳；这里固定下来
        out
    }

    /// 用多数票维护 `pos` / `neg`（给 `explain`、`cover` 这类"看标签"的地方用）。
    fn sync_plain_labels(&mut self) {
        let (sp, sn) = self.strong(1);
        self.pos = sp;
        self.neg = sn;
    }

    /// 采纳一个新谓词（扩词表）。调用方负责保证它"值得"——本结构不做评分。
    pub fn propose(&mut self, p: Predicate) {
        self.vocab.add(p);
    }

    /// T4 的容量：样本上可区分的可定义集数上界 = 2^(签名类数)。
    pub fn capacity(&self) -> u64 {
        1u64.checked_shl(self.classes().len() as u32).unwrap_or(u64::MAX)
    }

    /// 已确定的答案集合（调试用）。
    pub fn known(&self) -> Mask {
        self.pos | self.neg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 个对象；谓词 a 把 {0,1} 与 {2,3} 分开。
    fn ctx() -> Explorer {
        let a = Predicate::new("a", bit(0) | bit(1));
        Explorer::new(4, Vocabulary::with(vec![a]))
    }

    #[test]
    fn t2_insufficient_when_pos_and_neg_collide() {
        let mut e = ctx();
        // 0 与 1 在 a 上同为 true，签名相同
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);
    }

    #[test]
    fn proposing_a_separating_predicate_leaves_insufficiency() {
        let mut e = ctx();
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);

        // b 只含对象 0，把 0 与 1 分开
        e.propose(Predicate::new("b", bit(0)));
        assert_ne!(e.status(), Status::Insufficient);
    }

    /// 论文定理 5（Lean 文件里叫 T6）：词汇不足不会被更多例子修复。
    #[test]
    fn more_data_never_repairs_insufficiency() {
        let mut e = ctx();
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);
        e.answer(2, true);
        assert_eq!(e.status(), Status::Insufficient);
        e.answer(3, false);
        assert_eq!(e.status(), Status::Insufficient);
    }

    #[test]
    fn identification_loop_terminates() {
        let mut e = ctx();
        e.answer(0, true);
        e.answer(3, false);
        let mut guard = 0;
        while e.status() == Status::Undetermined {
            let x = e.next_question().expect("undetermined implies a free class");
            // 目标 S = {0, 1}
            let member = x == 0 || x == 1;
            e.answer(x, member);
            guard += 1;
            assert!(guard <= 4, "loop must terminate within |X| steps");
        }
        assert_eq!(e.status(), Status::Identified);
    }

    #[test]
    fn t4_capacity_grows_with_splitting_predicate() {
        let mut e = ctx();
        let before = e.capacity();
        e.propose(Predicate::new("b", bit(0) | bit(2)));
        assert!(e.capacity() > before, "a splitting predicate must raise capacity");
    }

    #[test]
    fn votes_need_a_margin() {
        // 两个单例类：p 只含对象 0
        let mut e = Explorer::new(2, Vocabulary::with(vec![Predicate::new("p", bit(0))]));
        e.answer_votes(0, 1, 1); // 平票，不算确证
        e.answer_votes(1, 0, 3); // 反例确证
        assert_eq!(e.status_with(1), Status::Undetermined); // 0 还没有确证标签
        assert_eq!(e.status_with(2), Status::Undetermined);

        e.answer_votes(0, 2, 1); // 票差 1，阈值 2 下仍不算确证
        assert_eq!(e.status_with(2), Status::Undetermined);

        e.answer_votes(0, 2, 0); // 累计 3:1，票差 2
        assert_eq!(e.status_with(2), Status::Identified);
        assert_eq!(e.status_with(1), Status::Identified);
    }

    #[test]
    fn noise_can_be_outvoted() {
        let mut e = ctx(); // {0,1} 同类
        e.answer_votes(0, 5, 1); // 强正例
        e.answer_votes(1, 1, 5); // 强反例
        assert_eq!(e.status(), Status::Insufficient);
        // 阈值调高到 6：两边都不够确证，于是退回未决，而不是误报撞车
        assert_eq!(e.status_with(6), Status::Undetermined);
    }

    #[test]
    fn weak_vote_blocks_identification() {
        let mut e = Explorer::new(2, Vocabulary::with(vec![Predicate::new("p", bit(0))]));
        e.answer_votes(0, 3, 0); // 强正
        e.answer_votes(1, 1, 1); // 平票 → 弱票，阻塞"已识别"
        assert_eq!(e.status_with(2), Status::Undetermined);
        assert_eq!(e.pending(2), vec![1]);
        assert_eq!(e.next_question_with(2), Some(1)); // 先补弱票

        e.answer_votes(1, 0, 2); // 累计 (1,3)，差 2 → 强反
        assert_eq!(e.status_with(2), Status::Identified);
        assert!(e.pending(2).is_empty());
    }

    #[test]
    fn epsilon_hat_and_theta() {
        let mut e = Explorer::new(4, Vocabulary::default());
        e.answer_votes(0, 3, 0);
        e.answer_votes(1, 0, 3);
        assert!(e.implied_epsilon() < 0.01);
        assert_eq!(e.recommended_theta(), 3);

        e.answer_votes(2, 2, 1); // 少数票 1/3
        e.answer_votes(3, 1, 2);
        assert!(e.implied_epsilon() > 0.15); // ≈ 0.167
        assert_eq!(e.recommended_theta(), 4); // 落在 0.1–0.2 之间
    }

    #[test]
    fn asked_and_unprobed_track_direct_questions() {
        let mut e = Explorer::new(3, Vocabulary::default()); // 一个类
        e.answer_votes(0, 3, 0);
        assert_eq!(e.asked, bit(0));
        assert_eq!(e.unprobed(), vec![1, 2]);
        e.answer_votes(1, 0, 3);
        assert_eq!(e.unprobed(), vec![2]);
    }

    #[test]
    fn empty_vocabulary_makes_every_pair_insufficient() {
        let mut e = Explorer::new(3, Vocabulary::default());
        // 空词汇：三个对象同一签名，只有一个类，可定义集只有 {} 与 X 两个
        assert_eq!(e.capacity(), 2);
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);
        e.propose(Predicate::new("p", bit(0)));
        // 加入 p 后类变为 {0} 与 {1,2}：前者含正例、后者含反例，全部被覆盖
        assert_eq!(e.status(), Status::Identified);
    }
}
