# 产品路线图 — 从"通用学习引擎"到"通用学习底座"

> 本文件回答：**polaris-core 作为一个通用学习底座，下一步要在哪些维度上长出来。**
> 角色定位：补 `SPEC.md`（宪法）与 `docs/ENHANCEMENT_ROADMAP.md`（强化轴线）之外的"产品形态轴线"。
> 与 `docs/tickets/QUEUE.md`（单票执行队列）配合：本文给鸟瞰、依赖与理据，QUEUE 标当前可领的票。
> 一切票名服从 SPEC §0 文档优先级，不引入新顶层概念；不过验证门的特性只能进 backlog 不能进默认行为。

## 1. 当前定位的再确认

polaris-core 是一个**领域无关的、本地优先的、可溯源的学习引擎**：

- 主命题：验证真懂 → 定位模糊 → 针对性补缺。
- 三支柱：抽象引擎（掌握度向量）+ 心智动力学引擎（学习者状态）+ 教学策略引擎（moves + 育种）。
- 三个门：MCP（主）/ HTTP API（伴随 UI）/ 内置 LLM（可选）。
- 装新域 = 放一个 pack 目录，零代码改动。

**当前真实状态**（截至 2026-06-15）：
- Phase 1–6 共 30+ 张票全部验收。
- 引擎本身在 Tier 0 上是**自洽且活跃**的：选题、提交、打分、相图判定、镜像报告、breeding、夜间巩固、参数自调优、心智动力学拟合均已就位。
- 但作为**通用学习底座**，它还停留在"引擎可运行"的形态——离"学习者真正打开它学东西"还有一道明显的产品形态缺口。

## 2. 通用底座视角的诊断框架

我们用四个维度审视底座成熟度：

| 维度 | 评估对象 | 当前评分 | 关键缺口 |
|---|---|---|---|
| **学习者层（L）** | 普通学习者能否打开并坚持使用 | 1/5 | 几乎无 UI；相图/校准/breeding 完全不可读；行动闭环未闭合 |
| **多 Pack 承载（M）** | 多域插拔、隔离、共享是否顺畅 | 2/5 | Pack 切换、跨域 θ 共享/隔离开关、Pack 作者上手缺位 |
| **工程演进（E）** | 引擎能否长期被 AI 协同维护 | 3/5 | engine.rs 2382 行 god-file；100+ 参数缺浏览面；测试与生产同文件 |
| **信任面板（T）** | 用户能否查"系统对我做了什么、判得准不准" | 2/5 | breeding/MRT 不透明；五框架门状态对用户不可见；隐私/LLM 调用清单缺失 |

四个维度对应四条新轴线（轴 6–9），与 ENHANCEMENT_ROADMAP 已登记的轴 1–5（能力/性能/可靠/实用/数学深化）正交，不重复。

## 3. 已规划路径（避免重复登记）

下列均已在 ENHANCEMENT_ROADMAP 或 QUEUE 中登记，本路线图**不再立项**：

- 校准后验化（P06F，已完成）
- 索引审计（P03L）、性能预算回归（P06E）、属性测试扩面（P06C）、数据主权运维（P06B）、MCP 工具面补全（P06A）、镜像报告 Tier 1 润色（P06D） — 全部已完成
- 数学深化候选：BKT-MIRT 逆方差融合、G_u 层级先验、相变动力学、θ AdaGrad、FSRS 个人参数拟合 — 已登记 backlog，等数据量
- Polaris Porcelain Intelligence Atlas（`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`） — 已在 `docs/visuals/atlas/` 下交付，**面向开发者的架构图谱**，不替代学习者层

## 4. 四条新轴线（12 张票）

### 轴 6 — 学习者产品形态（L 维度）

把"引擎在工作"翻译成"学习者能看见、能读懂、能行动"。

| 票号 | 名称 | 复杂度 | 服务环节 | 依赖 |
|---|---|---|---|---|
| **P07A** | 相图产品化语义层 | S | 验证真懂 → 用户读懂 | — |
| **P07B** | 学习者状态镜子 v1 | M | 验证真懂 → 定位模糊 | P07A、Atlas 静态站基建 |
| **P07C** | 报告 `top_signal` + `suggested_action` | S | 定位模糊 → 针对性补缺 | P07A |
| **P07D** | 行动闭环（相 → 任务响应策略） | M | 针对性补缺 | P07A、P03F、P03G |
| **P07E** | 学习者反馈通道扩展 | S | 验证真懂（修正引擎判断） | P07B |

**理据**：
- 当前 8 个相图名（Undetermined/Phantom/Fluctuation/Settling/Solidification/Transfer/Generation/Regression）是给学术/开发者用的词，对学习者不可读。P07A 加一层"产品名 + 一句话解读 + 推荐动作"映射（如 Phantom → "看起来懂"），不改判据，纯外观层 → S 票。
- 镜像报告当前是 5 类断言数组并列，缺"如果你只看一句"的顶部提示和"现在该做什么"的行动链。P07C 在 report.rs 加 `top_signal` 与 `suggested_action` 字段，仍走 strict-citation → S 票。
- 引擎已能识别 Phantom，但 BatchStrategy 只对 HMM 状态做了响应，对相图无专用策略 — 行动闭环未闭合。P07D 加 `BatchStrategy::PhantomChallenge`（专推 transfer/free_produce），并对 Settling/Regression 各加一条响应分支；每条新策略带留出验证门，不过门只是假设 → M 票。
- P07E 让用户能说"我现在状态是 flow / 我想暂停 / 这条断言对了"，比当前的"标不准"单一动作多 2-3 个语义化触点 → S 票。

### 轴 7 — 通用底座承载（M 维度）

让"装新域 = 放一个 pack 目录"真正成立到日常使用层面。

| 票号 | 名称 | 复杂度 | 服务环节 | 依赖 |
|---|---|---|---|---|
| **P08A** | 多 Pack 切换 + 数据隔离开关 | M | 全环节（通用性） | — |
| **P08B** | LLM 调用隐私清单 + 纯 Tier 0 模式 | M | 信任前提 | — |
| **P08C** | Pack 作者上手指南 + 模板 Pack | S | 生态 | P05A0 |

**理据**：
- 当前所有 pack 共用同一 θ 向量和同一 mastery_states 表。学习者今天学 Rust 想加日语，怎么切？数据是否相互污染？P08A 加 `polaris pack switch <name>`、`polaris pack list` 命令，并给每个 pack 一个"是否共享 θ"开关（默认共享，符合 MASTER_PLAN 跨域迁移；用户可在新域冷启动期独立 θ 防止迁移先验干扰）→ M 票。
- 当前 LLM 调用把哪些数据外发完全是黑盒（grader 把 attempt response 发出去，narrative 把断言列表发出去）。P08B 做两件事：①出一份"LLM 调用清单"文档与 `polaris privacy show` 命令；②加 `POLARIS_TIER0_ONLY=1` 环境开关，禁用所有 Tier 1 LLM 调用（grader 全部走 heuristic_score_with_conn，narrative 永远 None）→ M 票。这是 Tauri/UI 大投入之前必须的信任地基。
- P05A0 已经定了 Course Integration Protocol，但缺一份给外部课程作者的"30 分钟上手指南"和一个最小可玩的模板 pack（如 `packs/template/`）。P08C 是纯文档 + 一个 5 概念的样板 pack → S 票。

### 轴 8 — 工程可演进（E 维度）

让 engine.rs 不再是单点风险，让 AI 协同维护可持续。

| 票号 | 名称 | 复杂度 | 服务环节 | 依赖 |
|---|---|---|---|---|
| **P09A** | `engine.rs` 模块化拆分 | M | 全环节（可演进） | — |
| **P09B** | `polaris config` 浏览 CLI + 参数文档自动生成 | S | 全环节（可治理） | — |
| **P09C** | `polaris doctor --diagnose` 全面诊断 | S | 全环节（运维） | P06B |

**理据**：
- engine.rs 2382 行（其中 ~340 行测试与生产代码混合）。Phase 7+ 任何新功能（学习者层、多 pack 体验、信任面板）都会动它，回归风险随时间指数上升。P09A 拆三个子模块：`engine/task_selection.rs`（next_task/batch/ranked candidates）、`engine/submit_pipeline.rs`（submit/replay/grade_pending）、`engine/mental_state.rs`（mental_state_observation/snapshot/posterior）；同时把测试搬到 `crates/polaris-core/tests/engine_*.rs` → M 票。
- 当前 100+ 参数在 config.rs 用代码定义，开发者能读，**用户/管理员/Pack 作者无法快速浏览**。P09B 加 `polaris config list [--class A|B|C] [--tuning-route Replay|Mrt|Manual|Fit]`，同时跑 build.rs 阶段从注册表自动生成 `docs/PARAMETERS.md` → S 票。
- P06B 已经有 `polaris backup` + `polaris doctor`（SQLite 完整性 + 事件溯源重放），但缺"今天引擎到底跑了什么"的活动总览。P09C 在 doctor 上加 `--diagnose` 模式，输出最近 7 天的：tuning runs/breeding evaluations/mental fits/gu inductions/consolidation runs/mirror reports 摘要 → S 票。

### 轴 9 — 信任面板（T 维度）

让"系统对你做了什么、判得准不准"是可查的，而不是黑盒。

| 票号 | 名称 | 复杂度 | 服务环节 | 依赖 |
|---|---|---|---|---|
| **P10A** | 五框架门状态面板 + 实验透明度 | M | 验证真懂（验证门可见） | P03I、P05B、P04C |

**理据**：
- SPEC §3 验证门铁律说"不过门 = 假设，不得进默认行为"，但**用户不知道当前哪些框架过了门**。F1 教法签名、F2 相图判据、F3 摩擦曲线、F4 G_u 误解语法、F5 育种——状态全在不同的表里散落。
- breeding 当前在背后给用户做 A/B 测试，**用户不知道**（违反 SPEC §3 动机伦理"用户知情后仍会认可"测试）；MRT 微随机化也是黑盒。
- P10A 出一个 `polaris trust show` 命令 + 对应 HTTP/MCP 接口，输出：
  - 五框架门当前状态（fitted/unfit、过门/未过、留出 AUC 或 logloss）
  - 当前正在跑的 breeding 实验列表（candidate vs incumbent、当前 win_prob、样本数、admit_p）
  - 当前 MRT 预登记的活跃实验（move_id、随机化窗口、主效应假设）
  - 最近一次 mental_dynamics_fit / param_tuning / nightly_consolidation 摘要
- 同时把 breeding.min_n 默认从 6 提到 20（A 类参数手动门，不会被自动调优偷偷改回去）→ M 票。

## 5. 建议执行顺序

执行序按"先清债、再奠基、再用户层、最后信任面板"，体现"地基稳了再盖楼"：

```
P09A (engine 拆分)
  │
  └──→ P08B (隐私清单 + 纯 Tier 0)
         │
         └──→ P07A (相图语义层) ←── 基础语义
                │
                ├──→ P07B (学习者状态镜子)
                │      │
                │      └──→ P07E (反馈通道)
                │
                ├──→ P07C (top_signal + suggested_action)
                │
                └──→ P07D (相 → 任务响应) ←── P03F、P03G

  穿插（任意时点）：
    P09B (config CLI + 文档自动生成) — S
    P09C (doctor --diagnose) — S
    P08C (Pack 作者指南) — S

  最后：
    P08A (多 Pack 切换)
    P10A (信任面板) ←── 全部前置就绪
```

### 序号 1–10 的具体建议

| 序 | 票号 | 理由 |
|---|---|---|
| 1 | **P09A** engine 拆分 | 清地基；之后加新功能（轴 6/7/9）回归面小 |
| 2 | **P08B** 隐私清单 + 纯 Tier 0 | 上 UI 之前的信任地基；Phase 4 Tauri 没真投入正是这个的连带后果 |
| 3 | **P07A** 相图语义层 | 投入最低、产出最直接的"用户能读懂"票 |
| 4 | **P09B** config CLI | 1-2 天小票，开发者立刻受益 |
| 5 | **P07C** top_signal + suggested_action | 在 P07A 之上加报告人话化 |
| 6 | **P07B** 学习者状态镜子 v1 | 复用 atlas 静态站基建，扩"学习者视图"为相图实时面板 |
| 7 | **P09C** doctor --diagnose | 1-2 天小票，运维需要 |
| 8 | **P07D** 行动闭环 | 用户层第一波反馈消化后再深入 |
| 9 | **P07E** 反馈通道扩展 | 在状态镜子有了之后扩反馈语义 |
| 10 | **P08A** Pack 切换 | 等学习者层稳定后开始多域体验 |
| 11 | **P08C** Pack 作者指南 | 任何时点穿插，纯文档 |
| 12 | **P10A** 信任面板 | 全部前置就绪后做总信任出口 |

## 6. 守住的边界（绝不能动）

为避免"产品演化"破坏 SPEC，以下约束对所有 P07–P10 票生效：

- ❌ 不把"假设"特性自动接入默认行为（验证门未过的策略只能在用户主动开启时启用，且必须显式标注"实验性"）
- ❌ 不引入新顶层概念/命名（用 MASTER_PLAN 既有词汇：相、签名、摩擦、G_u、move、pack、Tier、validation gate…）
- ❌ 不在同步路径加任何 LLM 调用（违反 Tier 0 铁律）
- ❌ 不做"和同龄人比较"的对比或暗模式动机机制（违反 SPEC §3 动机伦理）
- ❌ 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`
- ❌ Worker 不写域特定逻辑进内核 crate
- ❌ 不跳过验证门声称完成；不伪造或估算测试输出

## 7. 不立项的强建议（留 backlog）

下列在审查中浮现但暂不进 P07–P10 主轴，记入 backlog：

- **跨设备/多用户**：当前是单 SQLite 单设备。多设备同步是大票，建议等学习者层稳定（即 P07 全做完）后再立项；不进 1.0。
- **数据可读导出**（JSON/Markdown）：用户主动诉求出现时立项。
- **MCP/HTTP API 稳定性合约**：1.0 release 时强制做版本化与 deprecation 政策；之前先保持当前形态。
- **数据库 schema 版本化迁移**：当前 db/migrate.rs 是一次性建表，schema 演进时再立 P11A。
- **沙箱模式**（用 P04E 的 simulation 给 Pack 作者试自己的 pack）：等 P08C Pack 作者指南交付后再考虑。

## 8. 与现有 ROADMAP 的关系

| 文件 | 角色 |
|---|---|
| `SPEC.md` | 宪法。一切冲突它赢 |
| `docs/MASTER_PLAN.md` | 设计意图蓝图 |
| `docs/DATA_MODEL.md` | 实现细节（公式/DDL/参数）权威 |
| `docs/ENHANCEMENT_ROADMAP.md` | **强化轴线 1–5**（能力/性能/可靠/实用/数学深化） |
| **`docs/PRODUCT_ROADMAP.md`（本文）** | **轴 6–9**（学习者形态/多 Pack 承载/工程演进/信任面板） |
| `docs/tickets/QUEUE.md` | 当前可领的单票队列 |
| `docs/tickets/TICKET_*.md` | 各票完整规格 |

## 9. 下次 AI 续跑该做什么

1. 读本文件 + `docs/AI_RUNBOOK.md` + `SPEC.md`。
2. 检查 `docs/tickets/QUEUE.md` 是否已认领 P07A–P10A 中任意一张。
3. 若无认领：按 §5 序号 1 起领 **P09A engine 模块化拆分**。
4. 按 `docs/tickets/TICKET_P06F_CALIBRATION_POSTERIOR.md` 的格式起草所领票的 `TICKET_PXXX_*.md`，含背景/范围/验收/禁区。
5. 在 QUEUE.md 把所领票从 Backlog 升到对应 Phase 段并标 In Progress。
6. 实现 → 跑全部验收命令（SPEC §6 + 票内集成命令）→ 把真实输出粘到票尾 → 等用户确认后 commit。

---

**本路线图状态**：v1，2026-06-15 产品经理交付。修订记录追加在文末。
