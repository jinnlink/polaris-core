# 增强路线图（Enhancement Roadmap）

> 本文件是 P03D 之后所有增强票的鸟瞰图：依赖关系、相对复杂度、优先级排序和设计理据。
> 单票执行纪律见 `docs/tickets/QUEUE.md`；各票完整规格见对应 `TICKET_*.md`。
> 状态（2026-06-17）：本文件保留历史路线图和设计理据。P03E-P06J 及相关增强票已按 `docs/tickets/QUEUE.md` 转正或归档；当前可执行状态只看 QUEUE。

## 票-阶段映射

| 票号 | 名称 | 阶段 | 复杂度 | 科学锚点 |
|------|------|------|--------|---------|
| P03E | 知识相图判定 | Phase 3 | M | Bjork 双强度理论→多维相分类 |
| P03F | Moves Bloom 扩展 | Phase 3 | M | Bloom/Anderson-Krathwohl 分类学 + ICAP |
| P03G | 交错调度 | Phase 3 | L | Rohrer & Taylor 2007 交错效应 + Wilson 2019 85% |
| P03H | G_u 自动归纳 | Phase 3 | L | Brown & Burton 1978 错误分析 + Siegler |
| P04E | 学习模拟端到端测试 | Phase 4 | L | — (验证性测试，非特性) |
| P05A1 | 算法 Domain Pack | Phase 5 | S | — (领域无关性验证) |

复杂度：S = 1-2 天，M = 3-5 天，L = 1-2 周，XL = 2+ 周。

## 依赖图

```
P03D (HMM, 已完成)
  │
  ├──→ P03E (相图判定)
  │      │
  │      ├──→ P03F (Moves Bloom 扩展)
  │      │      │
  │      │      └──→ P03G (交错调度) ←── P03E
  │      │
  │      └──→ P03H (G_u 自动归纳) ←── P03B (巩固)
  │
  └──→ P03G (交错调度) ←── HMM 状态感知
  
P03E + P03F + P03G + P03H
  │
  └──→ P04E (学习模拟测试) ── 验证以上全部

P05A0 (课程接入协议)
  │
  └──→ P05A1 (算法 Pack) ←── P03F (需要 7-move schema)
```

### 依赖说明

- **P03E → P03F**：相图判定需要理解 max_depth 语义；Moves 扩展丰富了 depth 的层级粒度，使相变判据更精确。但 P03E 可在现有 3-move 上先行实现（recall/explain/apply 足以覆盖基本相判据），P03F 扩展后 P03E 自动受益。
- **P03E + P03F → P03G**：交错调度的 batch 需要 Phase 信息选取概念（如疲劳态只排已掌握概念），也需要 move 粒度为每个 slot 选取合适 task_type。
- **P03B + P03E → P03H**：G_u 候选经巩固 holdout 门验证；G_u 触发的 misconception 影响相判定（幻影态检测）。
- **P03D → P03G**：交错调度的 HMM 状态感知依赖 P03D 的 dominant_state 输出。
- **P03E-H → P04E**：学习模拟测试是全闭环验证，需要所有 Phase 3 增强就绪才能完整测试。但可在部分增强完成后先跑简化版。
- **P05A0 + P03F → P05A1**：算法 pack 需要课程接入协议和 7-move schema；若 P03F 未完成，降级到 3-move 仍可通过。

## 优先级排序与理据

**历史执行顺序：P03E → P03F → P03G → P03H → P04E → P05A1（已完成）**

以下内容记录当时的排序理据，不再作为当前认领入口。

1. **P03E 知识相图判定** — 首先实现。相是后续所有增强的基础语义：签名选法（F1）按相选 move，摩擦曲线（F3）按相移动最优点，交错调度按相组 batch，镜像报告按相叙述。相图是掌握度向量的"人话翻译"——没有相，向量对用户不可解释。DATA_MODEL §9 判据已冻结，实现风险低。

2. **P03F Moves Bloom 扩展** — 紧随相图。3 种 move 是深度评判的瓶颈：如果只有 recall/explain/apply，max_depth 永远停在 apply，无法区分 Settling 与 Solidification。7 种 move 为深度维度 D 提供完整的观测通道，使"足够深度上被证明"（SPEC §1 掌握定义）可操作化。

3. **P03G 交错调度** — 在有了相和丰富 move 后实现。交错效应文献 meta-analysis 显示 d≈0.42（Brunmair 2019），是性价比最高的调度改进。此时 HMM（P03D）已就绪，可做状态感知 batch。

4. **P03H G_u 自动归纳** — 在交错调度后。G_u 依赖足够多的 attempt 数据（跨概念错误模式需要样本量），交错调度加速了数据收集。G_u 候选经巩固验证（P03B 已有基础设施），实现路径清晰。

5. **P04E 学习模拟测试** — Phase 3 增强全部就绪后，用模拟器做端到端验证。此测试是内核"还活着且在教人"的最终断言——掌握度单调上升、无死锁、相变合理、HMM 不卡死。跨 Phase 放在 P04 是因为它验证的是 Phase 3 的组合效果。

6. **P05A1 算法 Pack** — 最后实现。算法 pack 验证领域无关性——不同图谱拓扑（算法的深 prerequisite 链 vs Rust 的宽浅结构）下引擎行为一致。依赖课程接入协议（P05A0）提供的 validator 规则。

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| P03E 相判据阈值不适应个人差异 | 幻影/脆弱误判 | 阈值设为 B 类参数，预留 P03J 自调优路径 |
| P03F 新 move 的 MIRT d_t 初值不准 | 难度预测偏移 | d_t 为 B 类参数，可重放调优；85% 规则兜底 |
| P03G batch 在概念数 <5 时无法满足簇多样性 | 降级为单概念 | 明确降级路径：概念不足时等价现有 next_task |
| P03H G_u 误报率过高 | grader 提示词噪声化 | Beta 后验退役机制 + precision 门槛 |
| P04E 模拟器与真实学习者行为偏差 | 假阳/假阴性 | 多画像覆盖 + 后续真实数据对拍 |
| P05A1 暴露内核隐含的域假设 | 需回改内核 | 该暴露本身就是 P05A1 的价值 |

## 与已有框架的关系

新增票均组装自 MASTER_PLAN 既有五框架原语，不引入新顶层概念：

- P03E = F2 知识相图的 Tier 0 纯函数化
- P03F = 深度维度 D 的观测通道扩展（Bloom 对齐）
- P03G = FSRS R 层 + 超图 + HMM 的调度合成（交错效应锚定）
- P03H = F4 误解语法的自动发现管线
- P03I = 心智动力学引擎的透明化出口（断言 evidence-bound + Beta 后验置信度）
- P04E = 全闭环属性测试（验证组合）
- P05A1 = 领域无关性实证（第二 pack）

## 强化轴线提案（2026-06-12，P03I 交付时沉淀；历史候选，已转正或归档）

> 原则：全部用 MASTER_PLAN 既有词汇组装（相、签名、G_u、巩固、hazard、θ/Q、重放、Tier）；
> 每个理论对象带留出验证门，不过门 = 假设（SPEC §3）。本节保留为历史设计理据，不再作为当前立项入口。
> 状态（2026-06-17）：本节候选已按 QUEUE 转正或归档，不再作为待裁决队列。

### 轴 1 能力——让已登记的门"活"起来（优先级最高）

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| P03J（已排队） | 参数自调优 v1：B 类·重放途径参数的夜间反事实重放 + param_tuning_runs 审计（DATA_MODEL §12） | 事件溯源 fold（已有） | M |
| P03K 心智动力学拟合层激活 | hazard 周拟合 job（`fit_hazard_model` 已有纯函数，引擎从未拟合/持久化，恒为 unfit）→ 持久化 β + validation_auc；HMM 状态层门控评估（`hmm.gate_auc_margin` 已登记但 `strategy_enabled` 恒 false、observed_auc_margin 恒 null）→ 周评估"下一动作"预测 AUC margin，过门才允许状态调策略；HMM EM 重估（graded ≥ `hmm.em_min_n` 启用，DATA_MODEL §7） | P03D；解锁镜像报告 hazard 类断言与 P03G 状态感知的实证依据 | L |
| FSRS 个人参数拟合（已转 P06J） | `fsrs.w`（C 类，登记"个人复习史拟合"）：FSRS-optimizer 思路按人拟合遗忘曲线，留出对拍门 | 复习史样本量 | M |

### 轴 2 性能

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| 索引审计 | 当前全库无 CREATE INDEX；热路径：attempts(concept_id, created_at)、behavior_events(type, at)、json_extract 谓词（G_u 归纳/镜像报告/mental_state 查询）改生成列或表达式索引 | 无 | S |
| 性能预算回归 | DATA_MODEL §11 预算表（U(c)<10ms@10k、fold<50µs、重放<1ms/百条、HMM 一步<1µs）做成 criterion 基准 + 预算断言，防回归 | 无 | S |

### 轴 3 可靠性

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| 属性测试扩面 | fold/scheduler/phase 已有 proptest；补：G_u 生命周期决定性（任意 attempt 序列）、镜像报告稳定字段决定性、HMM 滤波数值稳定（极端观测不产生 NaN/退化） | 无 | S |
| 数据主权运维 | `polaris backup`（VACUUM INTO）+ 启动 `PRAGMA integrity_check` + 事件溯源重放自检（mastery_states 与全量重放一致性抽查）——Local-persistent 铁律的运维面 | 无 | S |

### 轴 4 实用性

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| MCP 工具面补全 | MCP server 停在 P02C 工具集；把 P03E-I 能力暴露为工具：相图快照、交错 batch、G_u 活跃规则、镜像报告（生成/读取/标不准）——Tier 2 门吃到 Phase 3 红利 | P02C | M |
| 镜像报告 Tier 1 润色 | LLM 把断言列表润色成周报叙事，strict-citation 引断言原文（断言 id 即 evidence id），降级 = 直接呈现断言列表（P03I 已是降级形态） | P03I | S |

### 轴 5 数学/理论深化（五框架内深化，非新概念；全部带验证门）

| 候选 | 数学内容 | 框架归属 | 验证门 |
|---|---|---|---|
| 校准后验化 | calib_gap 从 EWMA 点估计升级为分层 Beta-Binomial（per-concept + 全局收缩，复用 P03I 已实现的 ln-gamma/不完全 Beta 数学）；幻影判据从硬阈值变后验概率 P(高估\|数据)>τ | C 分量 / F2 幻影相 | 幻影标记的 30 天前瞻验证率优于现行硬阈值（calib.phantom_* 已登记此调优目标） |
| 融合不确定度传播 | BKT-MIRT 融合权 λ=n/(n+5) 改逆方差加权：BKT 路与 θ 路各带后验方差，精度加权融合，p̂ 输出带不确定度（镜像报告与探针任务派发可消费） | 抽象引擎 p_known | 留出 logloss 不劣于现行 λ 融合（margin 同 consol.accept_margin） |
| G_u 层级先验 | 同 pattern 跨概念簇共享 Beta 超先验（超图邻域分层）；新概念装入时 gu_risk 概率化（而非二值标记） | F4 误解语法 | 30 天前瞻 precision ≥ 现行平坦 Beta(1,1)（§9 已定义窗口） |
| 相变动力学 | 记录 per-user 相变迁计数（7 相马尔可夫转移矩阵），输出"脆弱→活跃的期望证据数"类预测；纯 Tier 0 统计 | F2 相图 | 相轨迹对下次表现的预测增益优于静态相分类（五框架门 F2 的强化形态） |
| θ 步长自适应 | mirt.eta 固定 0.05 改 AdaGrad 式按维累积二阶矩（仍是在线梯度 + step cap，C 类初始化语义不变） | 潜因子层 | 重放留出 logloss 改善 ≥ margin，否则保持 |

### 历史建议执行序（已收敛）

以下序列已在 QUEUE 后续票中转正或归档，保留为设计背景。

1. **P03J**（QUEUE 已排）——重放自调优是"数据接管 B 类"的机制基础。
2. **P03K**——P03D/P03I 登记的三个门（hazard AUC、HMM gate、EM）目前是死字段，激活后镜像报告与状态调度才有实证含金量。
3. 轴 2/3 的 4 张 S 票可在大票之间穿插（每张 1-2 天，回报立现）。
4. 轴 5 数学深化建议放 P03K 之后：校准后验化与 G_u 层级先验依赖更多真实数据量才能过前瞻门。
5. MCP 工具面补全可在任何时点插入（纯暴露层，无模型风险）。

## 产品形态轴线提案（2026-06-15，产品经理审查沉淀；详见 `docs/PRODUCT_ROADMAP.md`）

> 原则：与轴 1–5 正交。轴 1–5 让"引擎更准"，轴 6–9 让"学习者能用、能信、能持续维护"。
> 全部用 MASTER_PLAN 既有词汇组装，不引入新顶层概念；行为变化必须带留出验证门。
> 本节保留为历史设计理据，不再作为当前立项入口。

### 轴 6 学习者产品形态（让"引擎在工作"翻译成"用户能看见、能读懂、能行动"）

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| P07A 相图产品化语义层 | 8 个相图名加"产品名 + 一句话解读"映射；不改判据，纯外观层 | — | S |
| P07B 学习者状态镜子 v1 | 复用 atlas 静态站基建扩出学习者实时面板（自信 vs 实际曲线、相分布、近期断言） | P07A、atlas 基建 | M |
| P07C 报告 top_signal + suggested_action | 镜像报告加顶部"如果你只看一句"提示与每条断言对应可选行动；strict-citation 不变 | P07A | S |
| P07D 行动闭环（相 → 任务响应） | 补 `BatchStrategy::PhantomChallenge` 等相专属调度分支；每条带留出验证门 | P07A、P03F、P03G | M |
| P07E 学习者反馈通道扩展 | 在"标不准"之外加"我现在状态是 / 我想暂停"等语义化触点 | P07B | S |

### 轴 7 通用底座承载（让"装新域 = 放一个 pack 目录"在日常使用层面成立）

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| P08A 多 Pack 切换 + 数据隔离开关 | `polaris pack switch/list` + 每 pack 是否共享 θ 的配置 | — | M |
| P08B LLM 调用隐私清单 + 纯 Tier 0 模式 | `polaris privacy show` + `POLARIS_TIER0_ONLY=1` 全局开关 | — | M |
| P08C Pack 作者上手指南 + 模板 Pack | 30 分钟上手文档 + 5 概念样板 pack（如 `packs/template/`） | P05A0 | S |

### 轴 8 工程可演进（让 engine.rs 不再是单点风险，让 AI 协同维护可持续）

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| P09A engine.rs 模块化拆分 | 拆 `engine/task_selection.rs`、`engine/submit_pipeline.rs`、`engine/mental_state.rs` + 测试搬到 `tests/` | — | M |
| P09B polaris config 浏览 CLI + 参数文档自动生成 | `polaris config list [--class A|B|C]` + 自动生成 `docs/PARAMETERS.md` | — | S |
| P09C polaris doctor --diagnose 全面诊断 | 最近 7 天 tuning/breeding/mental_fit/gu/consolidation/report 摘要 | P06B | S |

### 轴 9 信任面板（让"系统对你做了什么、判得准不准"可查，而不是黑盒）

| 候选票 | 内容 | 依赖 | 复杂度 |
|---|---|---|---|
| P10A 五框架门状态面板 + 实验透明度 | `polaris trust show` 暴露 F1-F5 门状态、breeding/MRT 活跃实验；breeding.min_n 默认 6→20 | P03I、P05B、P04C | M |

### 历史建议执行序（已完成，详见 `docs/PRODUCT_ROADMAP.md` §5）

以下序列已在 P07A-P10A 中完成，保留为依赖背景；当前下一票以 QUEUE 为准。

1. **P09A**——先清地基（engine 拆分），之后加新功能回归面小。
2. **P08B**——隐私清单 + 纯 Tier 0 是信任地基，Tauri/UI 大投入之前必备。
3. **P07A**——投入最低、产出最直接的"用户能读懂"票。
4. **P09B**（穿插）——1-2 天小票，开发者立刻受益。
5. **P07C**——在 P07A 之上加报告人话化。
6. **P07B**——复用 atlas 基建扩学习者实时面板。
7. **P09C**（穿插）——1-2 天小票，运维需要。
8. **P07D**——用户层第一波反馈消化后再深入。
9. **P07E**——状态镜子有了之后扩反馈语义。
10. **P08A**——多域体验。
11. **P08C**（任意时点穿插）——纯文档。
12. **P10A**——全部前置就绪后做总信任出口。
