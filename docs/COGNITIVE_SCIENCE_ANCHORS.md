# 认知科学锚点（Cognitive Science Anchors）

> 本文件是 polaris-core 引擎的科学参照框架。每个引擎特性必须溯源到至少一条经验验证的认知科学原理；无科学锚点的特性标记为"假设"，须过验证门才能成为默认行为。
>
> 设计原则：**先验来自科学，后验来自你的数据。** 群体文献效应量初始化参数，个人行为数据经事件溯源修正——系统不是静态教科书的翻译，而是 N-of-1 实验平台。

## 原理-实现映射表

| 原理 | 关键文献 | 引擎实现 | 模块 | 状态 |
|------|---------|---------|------|------|
| **间隔效应** (Spacing Effect) | Cepeda, Pashler, Vul, Wixted & Rohrer 2006, "Distributed practice in verbal recall tasks: A review and quantitative synthesis" *Psychol Bull* | FSRS 调度层：R 分量按遗忘曲线 `R(c)=(1+t/(9·S))^(−1)` 决定复习时机；间隔随稳定性增长自动扩张 | `fsrs.rs`, `scheduler.rs` | 已实现 |
| **测试效应** (Testing Effect) | Roediger & Karpicke 2006, "Test-enhanced learning: Taking memory tests improves long-term retention" *Psychol Sci* | recall/explain/apply moves 强制检索而非重读；评分一律 evidence-bound（引擎自跑 strict-citation grader，不接受外部直写） | `grader.rs`, `teaching.rs` | 已实现 |
| **交错效应** (Interleaving Effect) | Rohrer & Taylor 2007, "The shuffling of mathematics problems improves learning" *Instr Sci*; Kornell & Bjork 2008, "Learning concepts and categories" *Psychol Sci* | 调度器 mini-batch：1 新/弱 + 2 复习，复习概念来自不同超图簇，强制辨析学习 | `scheduler.rs` | 计划 (P03G) |
| **生成效应** (Generation Effect) | Slamecka & Graf 1978, "The generation effect: Delineation of a phenomenon" *J Exp Psychol: Human Learn Mem* | explain/apply/create moves 要求学习者自行产出而非被动接收；自由讲解的 guess 参数降至 0.05（降低猜对概率权重） | `teaching.rs`, `mastery.rs` | 已实现 |
| **合意困难** (Desirable Difficulties) | Bjork 1994, "Memory and metamemory considerations in the training of human beings"; Bjork & Bjork 2011, "Making things hard on yourself, but in a good way" | 85% 规则 + 知识相图：Settling→Solidification 相变期主动提升任务难度；摩擦曲线 φ* 个人化最优困难点（F3） | `scheduler.rs`, `phase.rs` (计划) | 部分实现 |
| **元认知校准** (Metacognitive Calibration) | Kruger & Dunning 1999, "Unskilled and unaware of it" *J Pers Soc Psychol* | 校准 EWMA 追踪自信-正确差 `calib_gap`；Brier 分 `brier_ewma`；幻影态检测（高自信+低实测）触发费曼/超校正修复 | `mastery.rs`, `status.rs` | 已实现 |
| **互补学习系统** (Complementary Learning Systems) | McClelland, McNaughton & O'Reilly 1995, "Why there are complementary learning systems" *Psychol Rev*; O'Reilly & Norman 2002 | 快通路（attempt 级 θ 梯度更新，微秒）+ 慢通路（夜间巩固：残差聚类→图式归纳→留出集回滚门）；仿海马快编码 / 皮层慢整合 | `mirt.rs` (快), `consolidation.rs` (慢) | 已实现 |
| **生产性失败/生产性困惑** (Productive Failure / Productive Confusion) | Kapur 2008, "Productive failure" *Cogn Instr*; Kapur 2016; D'Mello & Graesser 2012, "Dynamics of affective states during complex learning" *Learn Instr* | HMM 状态层区分"生产性困惑"与"挫败"：困惑态不救（守最优困惑区），超时未解才降级；struggle-first 教法 move | `mental_state.rs`, `teaching.rs` | 已实现 (P03D) |
| **最近发展区** (Zone of Proximal Development) | Vygotsky 1978, *Mind in Society* | 脚手架随 p_known 连续衰减（专长反转效应，Kalyuga）；调度器目标预测成功率 0.80–0.90；工作样例在高 p_known 时自动禁用 | `scheduler.rs`, `teaching.rs` | 已实现 |
| **Bloom 分类学** (Bloom's Taxonomy) | Bloom 1956, *Taxonomy of Educational Objectives*; Anderson & Krathwohl 2001 (修订版) | 深度维度 D：recall→explain→apply→transfer；move 按 Bloom 层级选取；掌握要求 D≥apply + 迁移证据。扩展至 7 级 Bloom 对齐 moves | `teaching.rs`, `moves.toml` | 部分实现，扩展计划 (P03F) |
| **知识空间理论** (Knowledge Space Theory) | Doignon & Fangon 1999, *Learning Spaces* | 类型化超图 prerequisite 结构：概念未达标时后继锁定（`p_known ≥ prereq_p` 门控）；前置传播诊断 | `graph.rs`, `diagnosis.rs` | 已实现 |
| **结构映射** (Structure Mapping) | Gentner 1983, "Structure-mapping: A theoretical framework for analogy" *Cogn Sci*; Gentner & Markman 1997 | k-hop 类型化子图贪心对齐得分 `struct(a,b)`；几何层提议经结构层裁决才入图；类比是派生物（同图式共属+对齐分），非手写原语 | `graph.rs`, `geometry.rs` | 已实现 |
| **错误分析** (Error Analysis) | Brown & Burton 1978, "Diagnostic models for procedural bugs in basic mathematical skills" *Cogn Sci*; Siegler Rule Assessment | 误解语法 G_u：8 类 pattern（过度泛化/边界盲区/符号混淆/因果颠倒/流畅性错觉/程序-概念断裂/粒度失配/干扰混淆）；跨域预测验证 + Beta 后验退役 | `misconceptions.toml`, `grader.rs` | 部分实现，自动归纳计划 (P03H) |
| **ICAP 框架** (Interactive-Constructive-Active-Passive) | Chi & Wylie 2014, "The ICAP framework: Linking cognitive engagement processes to learning outcomes" *Educ Psychol* | move 按参与深度排序：交互>建构>主动>被动；生成任务尽量上移；task_type 序数映射 MIRT 难度 d_t | `teaching.rs`, `mirt.rs` | 已实现 |
| **85% 规则** (85% Rule for Optimal Learning) | Wilson, Shenhav, Stiso & Cohen 2019, "The eighty five percent rule for optimal learning" *Nat Commun* | 调度器目标预测成功率 0.80–0.90；被 F3 摩擦曲线吸收为先验（个人 φ* 替代全局固定值） | `scheduler.rs` | 已实现 |
| **睡眠巩固** (Sleep Consolidation) | Diekelmann & Born 2010, "The memory function of sleep" *Nat Rev Neurosci*; Walker 2017, *Why We Sleep* | 夜间巩固 job 安排在空闲/睡眠时段；难材料排睡前末次、晨间首测；巩固产物过留出集回滚门 | `consolidation.rs`, `scheduler.rs` | 已实现 |

## 补充锚点（已在教学法纲要中引用，尚未进入独立引擎模块）

| 原理 | 关键文献 | 计划落地 | 状态 |
|------|---------|---------|------|
| 超校正效应 (Hypercorrection) | Butterfield & Metcalfe 2001 | 幻影态特效药 move：诱导自信判断→揭示→修正 | 教学策略计划 |
| 认知学徒制 (Cognitive Apprenticeship) | Collins, Brown & Newman 1989 | 宏观弧：专家示范→脚手架→渐隐→反思 | 教学策略计划 |
| 自我调节学习 (Self-Regulated Learning) | Zimmerman 2002, SRL 三段循环 | session 三段式骨架（计划-监控-反思） | UI/session 设计计划 |
| 变异理论 (Variation Theory) | Marton & Tsui 2004 | 出题引擎规则：对比项仅差一个关键特征 | 出题引擎计划 |
| 自我决定理论 (Self-Determination Theory) | Deci & Ryan 2000 | UI 铁则：永远给 2-3 选项，绝不单一指令 | UI 设计（已定为铁律） |

## 使用规则

1. **新特性提案必须引用本表中至少一行**，或补充新行（附文献+效度评估）。
2. **无锚点特性标记为"假设"**，须过留出验证门（DATA_MODEL §10 A 类门槛）才能进入默认行为。
3. **被证伪的原理**（如学习风格类型，Pashler 2008）明确列入反伪科学红线（SPEC §3），禁止以任何形式进入系统。
4. **效应量标注**遵循教学法纲要 v3 的诚实标注规范：强/中/弱/混杂/已证伪。
5. 本表与 `docs/MASTER_PLAN.md` 教学法纲要互补：纲要给出方法库细节，本表给出原理→引擎模块的溯源映射。
