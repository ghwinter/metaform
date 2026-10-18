# 03 文献综合：谁固定了什么

v1.0 · 2026-10-19 03:30 · **覆盖 36/36 篇**（六个批次，全部基于 `.library/_text/` 本地文本）

## 0 方法

- 36 篇 PDF → `pdftotext` → `.library/_text/`（`scripts/extract-pdf-text.ps1`，17.6 s）。
- 分六批抽取，每篇固定六字段：**研究问题 / 固定的表示或语言 / 能否改变表示 / 触发信号 / oracle / 选择准则**。
- 只信本地文本；文本没写的写"未说明"，禁止凭记忆补全。引用原文 ≤ 25 词。
- 已知缺口：`gentner1983` 正文 OCR 损坏，只有末页可读；FCA 的 attribute exploration 只在
  `03a` 的文档里没有对应实现（`concepts` 包不含该算法），需要用 Ganter & Wille 原著补引。

## 1 对照表（36 篇）

### 1.1 问题发现与认知建模（02）

| 文件 | 固定的表示/语言 | 能改表示？ | 触发信号 | oracle | 选择准则 |
|---|---|---|---|---|---|
| adolfi2024 | 未形式化，"language or framework"仅约定 | 概念层允许，无算子 | 拟真性约束被违反 | 无 | 满足拟真性约束 |
| kemp2008 | 图语法假设空间（8 形式 / 20 候选） | 部分：F 可变，限枚举内 | 数据集 D | D 本身 | max P(S,F∣D) |
| kemp2008 SI | 元语法模板 | 否（实作只枚举 20） | 同正文 | 未说明 | 同正文 |
| kemp2010（因果） | schema 空间；域问题与谓词给定 | 部分：类别数可变 | 多域事件数据 | 无 | schema 后验 |
| kemp2010（理论） | 关系系统 (z,g)；关系给定 | 部分：类别划分与数量可变 | 关系矩阵 R | 无 | P(z,g∣R)，CRP 先验 |
| tracing_insight | 火柴题初始表征 | 是（被试），模型不产生 | 提示、眼动 | 提示由实验者给 | 无；变点检测 |

### 1.2 结构映射与向量符号架构（03b / 03c）

| 文件 | 固定的表示/语言 | 能改表示？ | 触发信号 | oracle | 选择准则 |
|---|---|---|---|---|---|
| falkenhainer1989 | 类型化谓词演算，谓词需同名 | 否，只可换规则 | 无 | 建模者 | 结构评分 |
| gentner1983 | 谓词论元结构（仅句法） | 否 | 无 | 人的表征 | 系统性 |
| sme4_complexity | 同上，语句只增不改 | 否，Remap 靠外部 | 外部判定 | 外部任务模型 | 分数阈值 |
| sme4_using | 需预载词汇表 | 否 | 未说明 | 使用者 | 未说明 |
| kleyko2022（综述 I） | D 维 HV + 绑定/叠加 | 可加新符号，语义不变 | 无 | 设计者 | 相似性、容量 |
| kleyko2023（综述 II） | 同上，类比需谓词图 | 否，编码是瓶颈 | 未说明 | 外部预处理 | 精度、成本 |
| schlegel2021 | 所选 VSA 空间与算子 | 否，设计时定 | 基准实验 | 设计者 | 容量、精度 |

### 1.3 规则与程序归纳（04）

| 文件 | 固定的表示/语言 | 能改表示？ | 触发信号 | oracle | 选择准则 |
|---|---|---|---|---|---|
| cropper2021 | 子句 + 人工语言偏置（metarules / mode） | 部分：可发明谓词，偏置不变 | 需要链式分解、子句过长 | 正负例 + BK（人给） | 覆盖 + 程序更短 |
| ellis2020 | 领域 λ-演算基元集 | 是：库增长 | wake 阶段被复用的子树 | 任务 I/O | 后验 + 描述长度 |
| lake2015 | 概率程序（离散动作基元） | 基本否：概念可组合，先验可学 | 背景字符集（离线） | 少量样例图像 | 后验 |
| makke2024 | 预定义函数库 L | 否 | 无（人工选库） | 数据 (x,y) | 精度 / 简洁 / 可解释 |
| palmarini2024 | λ-演算 + 库 D | 是：chunking | 识别模型使用率 + 压缩增益 | 任务 + 神经识别模型 q | 压缩目标 |
| pysr | 用户给算子、变量、深度上限 | 否（自定义算子人写） | 无 | 数据 + 用户配置 | 精度-复杂度 Pareto |

### 1.4 概率、信息与模型选择（05）

| 文件 | 固定的表示/语言 | 能改表示？ | 触发信号 | oracle | 选择准则 |
|---|---|---|---|---|---|
| di_fiore2024 | 变量 χ、GP 核、保真层级 | 否（保真层级 ≠ 新语言） | 采集函数峰值 | 高保真仿真/实验 | EI / PI / MES |
| farquhar2024 | token 分布、NLI 等价类 | 否（只聚类既有意义） | 语义熵高 | NLI 判双向蕴含 | 语义熵阈值 |
| friston2026 | 因子化模型、预设模型空间 M | 部分：只在 M 内缩减 | 结构 EIG 高 | 环境反馈 | 期望自由能、Bayes 因子 |
| grunwald2019 | 模型族 {M_γ}、似然 | 否（只在 Γ 内选） | 数据（被动） | 数据本身 | 最短码长 |
| lieder2019 | 假设空间、成本函数 | 原则上可，无在线机制 | 精度-成本差 | 环境统计 + 效用 | 约束下期望效用 |
| perfors2011 | 假设空间 H、生成过程 | 有限可；潜在空间不变 | 观测数据 | 数据 | 后验 + 奥卡姆 |

### 1.5 意图、语用与因果表示（06 / 07）

| 文件 | 固定的表示/语言 | 能改表示？ | 触发信号 | oracle | 选择准则 |
|---|---|---|---|---|---|
| frank_goodman2012 | 共享词表 V、布尔词义、指称集 C | 否 | 无 | 人类实测显著性先验 | 效用 = 信息量 − 成本 |
| ying2024 | 固定目标集 G、网格动作、联合规划 | 否 | 指令与动作 | 人类被试、GPT-3 似然 | 后验 P(g∣u,a) |
| scholkopf2021 | 因果变量集、图、可干预集（视为给定） | 是，但须外部干预/偏移 | 干预、分布偏移、SMS | 多环境干预数据 | ICM/SMS 稀疏变化 |

### 1.6 反例、自纠与科学发现（08 / 09）

| 文件 | 固定的表示/语言 | 能改表示？ | 触发信号 | oracle | 选择准则 |
|---|---|---|---|---|---|
| clarke2000_cegar | 原子公式（谓词）集；h 可变 | 部分：只细化 h 的等价类划分 | 伪反例最短前缀 | 具体模型 K，SplitPATH 符号验证 | 最粗可分离细化（实用次优） |
| agarwal2026 | 同一模型 CoT 词元 | 否：只换采样/投票/束搜索 | 算力 + 难度 + 模型型 | 评测真值 | 单位算力准确率 |
| huang2024 | 同一 LLM + 提示 | 否 | 自生成反馈 | 无（真值仅对照） | 答案准确率 |
| madaan2023 | 同一 LLM + 三类提示 | 否 | 自反馈文本 | 无（人工/GPT-4 偏好） | 偏好率 / 任务指标 |
| snell2025 | 提议分布 + 固定验证器 | 否 | 验证器分、难度分箱 | PRM/ORM | 验证器加权 best-of-N |
| tacl2024 综述 | LLM + 反馈提示 | 否 | 反馈生成 | 外部工具/知识/人类/真值 | 按 RQ 与检查表 |
| abduction2022 | 条件句框架 + 背景知识 | **是**：创造型溯因可引入新词项 | 事实 F 无已知条件句可蕴涵 | 模式生成/辩护规则 | 模式内排序 |
| kramer2026 | 方程/微分方程 + 文法算子 | 部分：库学习增构件 | 拟合误差 / 复杂度 | 实验数据、领域专家 | 精度 + MDL + 可解释 |

## 2 我自己核验过的原文锚点

1. **ILP 自己承认瓶颈在语言。** "Even for ILP experts, determining a suitable language bias is
   often a frustrating and time-consuming process. We think the need for an almost perfect
   language bias is severely holding back ILP"（`cropper2021_ilp_at_30.txt`）。
2. **Kemp 说需要 predicate invention，却用"假设空间的两种意义"绕开。**
   "Predicate invention is crucial since part of our goal is to discover novel concepts, and new
   predicates will be needed to refer to these concepts."（`kemp2010_probabilistic_theory_formation.txt`）
   随后以"搜索常走到与起点很远的区域"辩护——**轨迹论证，不是词汇论证**。
3. **词汇增长存在，但由语料驱动。** "as the model gains a richer conceptual vocabulary, its
   representations evolve"（`ellis2020_dreamcoder.txt`），判据是压缩。
4. **主动推断自己承认模型空间没解决。** "An outstanding issue is the elaboration of alternative
   models; i.e., the specification of the model space."（`friston2026_active_inference.txt`）
5. **贝叶斯学习只在既有假设里选。** "all the model does is choose among hypotheses that already
   exist"（`perfors2011_bayesian_tutorial.txt`）
6. **CEGAR 细化的是分区，不是词汇。** 原子公式"automatically extracted from the program"；
   且"The problem of finding the coarsest refinement is NP-hard"（`clarke2000_cegar.txt`）。
7. **唯一给出"造新词"规范机制的是创造型溯因。** "introduction of a new term that is hitherto
   undefined"（`abduction_conditionals_2022.txt`）——但没有判定该不该造、造哪个的准则。
8. **LLM 一侧完全没有换语言的能力。** 六篇（自纠四篇 + 测试时计算两篇）一致：反馈同源自评，
   语言不变；综述的原话是"no prior work demonstrates successful self-correction with feedback
   from prompted LLMs"（`tacl2024_self_correction_survey.txt`）。

## 3 空位判断（36 篇全覆盖后的定稿）

| 维度 | 现状 | 缺口 |
|---|---|---|
| 触发信号 | 压缩停滞、复用频率、覆盖增益、后验、伪反例、拟合误差、拟真性约束 | 没有一个是"候选内涵集为空"这个**可判定**信号 |
| 语言修订算子 | predicate invention（无准则）、library refactoring（语料驱动）、h 分区细化、创造型溯因（无判据） | 缺"由单次查询触发、按信息增益选下一个谓词"的算子 |
| 选择准则 | MDL / 后验 / 覆盖 / 结构 EIG / 期望效用 / Pareto | 没有一个用于选"下一个该问哪个谓词" |
| oracle | 人类专家、任务集、模型检查器、NLI、图像标签 | 没有"廉价噪声 oracle + 主动选题"的组合 |

**定稿结论**：这 36 篇里，**没有任何一篇把"当前语言表达不了"作为可判定触发，并配一个符号成本
合理的提问策略**。最接近的三处：

1. FCA 的属性探索（`03a` 只有库，没有算法实现；需引原著）——人给属性、人答题；
2. ILP 的 predicate invention——能造词，但没有准则；
3. 创造型溯因——能引入新词项，但没有触发判据。

这三处合起来正好是本文的算法骨架，而它们从未被合成。

## 4 对论文的影响（三条，全部落地）

1. **§6 相关工作**已改写：Cropper 的语言偏置原话、Kemp 的 predicate invention 与"两种假设空间"、
   DreamCoder 的语料驱动重构、Friston 的 outstanding issue、Perfors 的"只在既有假设里选"。
2. **T2 表述**改为 trajectory vs vocabulary，直接回应 Kemp 的轨迹辩护。
3. **贡献 2 措辞**改为"把已有准则接到新的可判定触发信号上"，不冒充新准则。

## 5 实验：域 A（合成形式背景）

完整数据与解释见 `research/exp/RESULTS.md`。三句话版本：

1. **T2 的经验版成立**：初始词汇缺一个区分属性时，V₀ 内一致的假设 **300/300 为空**。
2. **C3 作废**：均匀后验下，"信息增益选题"与"随机问一个还有信息的问题"完全持平
   （1.96 vs 1.86；乘积空间 7.78 vs 7.78）。全部收益来自"不问没信息的问题"，不是"选得聪明"。
3. **加先验才有微弱优势**：p(S) ∝ 2^-|S| 时 1.63 vs 1.87（9–13%）。贝叶斯主动学习在这个域是弱杠杆。

对论文的直接影响：主指标从"提问数"改为**总成本（对象查询 + 谓词提议）下的恢复率**，
并比较三种停止规则（F 空才换 / 预算用完才换 / 按期望收益换）。这与 T2 一致：
既然词汇内搜索必然失败，优化对象级选题就是在优化一个注定失败的过程。

## 6 下一步

1. 修 C3 的措辞（上面这条）。
2. 域 A 扩到 8 属性 × 14 对象、加"LLM 直觉选题"基线，看 1.62× 是否放大。
3. 域 B（Lean 判定）/ 域 C（测试判定）接入。
4. 补 FCA 原著引用（attribute exploration），否则 §6 的第一段没有出处。
