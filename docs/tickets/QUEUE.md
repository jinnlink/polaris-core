# 票队列（单票制）

状态：**P05A1 已完成并提交**。实现与测试已完成；用户已确认继续推进，接受隔离 target 同参数 clippy 作为本机 `target/debug` 文件锁的替代验收证据。任何时刻只允许 1 张票 In Progress。
P03E+ 优先级见 `docs/ENHANCEMENT_ROADMAP.md`（月度对齐见 `C:\MyProject\Learned\rust-mastery-lab\docs\ENHANCEMENT_ROADMAP.md`）。
新增票必须标注它服务主命题（验证真懂→定位模糊→针对性补缺）的哪一环。

## Phase 1 — Walking Skeleton

- [x] **P01 最小闭环**（`TICKET_P01_WALKING_SKELETON.md`）← 已实现并完成子 agent 审查补修；进入 P02 前需新开票认领

## Phase 2 — 图谱 + MCP

- [x] **P02A 类型化超图**（`TICKET_P02A_TYPED_HYPERGRAPH.md`）← 已实现并提交（`e876de3`）；服务环节：定位模糊
- [x] **P02B 图谱感知诊断**（`TICKET_P02B_GRAPH_AWARE_DIAGNOSIS.md`）← 已实现并提交（`0cec9f5`）；服务环节：定位模糊 → 针对性补缺
- [x] **P02C MCP server**（`TICKET_P02C_MCP_SERVER.md`）← 已实现并提交（`b11fa02`）；服务环节：验证真懂 → 定位模糊 → 针对性补缺；Tier 2 门

## Phase 3 — 潜因子 + 心智动力学

- [x] **P03A MIRT 潜因子层**（`TICKET_P03A_MIRT_LATENT.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- [x] **P03B 夜间巩固 v1**（`TICKET_P03B_NIGHTLY_CONSOLIDATION.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- [x] **P03C 几何候选层 v1**（`TICKET_P03C_GEOMETRY_CANDIDATES.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- [x] **P03D 状态 HMM + 行为发射 + 离散时 hazard 放弃模型**（`TICKET_P03D_MENTAL_STATE_HMM.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- [x] **P03E 知识相图判定**（`TICKET_P03E_KNOWLEDGE_PHASE_DIAGRAM.md`）← 已实现并通过验收；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P03F Moves Bloom 扩展**（`TICKET_P03F_MOVES_BLOOM_EXPANSION.md`）← 已实现并通过验收；服务环节：验证真懂 → 针对性补缺
- [x] **P03G 交错调度**（`TICKET_P03G_INTERLEAVED_SCHEDULING.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- [x] **P03H G_u 自动归纳**（`TICKET_P03H_GU_AUTO_INDUCTION.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- [x] **P03I 镜像报告 v1**（`TICKET_P03I_MIRROR_REPORT.md`）← 已实现并通过验收；每条断言带证据 id + 置信度，说不出证据不许进报告；服务环节：验证真懂 → 定位模糊
- [x] **P03J 参数自调优 v1**（`TICKET_P03J_PARAM_TUNING.md`）← 已实现并通过验收；B 类·重放途径：夜间反事实重放调参 + param_tuning_runs 审计（DATA_MODEL §12）；服务环节：定位模糊 → 针对性补缺
- [x] **P03K 心智动力学拟合层激活**（`TICKET_P03K_MENTAL_DYNAMICS_FIT.md`）← 已实现并通过验收；hazard 周拟合 + HMM 状态门控评估 + EM 重估，激活 P03D/P03I 登记的三个死门；服务环节：定位模糊 → 针对性补缺
- [x] **P03L 索引审计**（`TICKET_P03L_INDEX_AUDIT.md`）← 已实现并通过验收；全库首批索引 + 查询计划断言（Tier 0 预算结构保障）；服务环节：全环节

## Phase 4 — UI + MRT

- P04A Tauri 常驻小窗（100% Tier 0 秒开）+ 可展开工作区（状态镜子=相图）
- P04B HTTP API 门
- P04C MRT 微随机化引擎（预登记审计）+ 教法签名后验（F1）+ 个人摩擦曲线拟合（F3）
- P04D 目标引擎移植（goals/dimensions/milestones，参考 Polaris schema v9）
- [x] **P04E 学习模拟端到端测试**（`TICKET_P04E_LEARNING_SIMULATION_TEST.md`）← 已实现并通过验收；三画像 30 天模拟 + 每日 summary + 无死锁/相变/HMM/θ 跟踪断言；服务环节：验证真懂 → 定位模糊 → 针对性补缺（全闭环验证）

## Phase 5 — 第二 pack + 育种

- P05A0 课程接入协议 v1（Domain Pack API / Course Integration Protocol）：把 pack 文件形状、validator 规则、证据映射、评分 rubric、moves、版本兼容和外部课程作者指南文档化；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P05A1 算法 Domain Pack**（`TICKET_P05A1_ALGORITHMS_PACK.md`）← 已实现并提交（默认 target clippy 文件锁由隔离 target 同参数通过替代）；服务环节：验证真懂 → 定位模糊 → 针对性补缺（第二域验证领域无关性）
- P05A 英语 pack（从 Polaris CEFR 表导出）：插拔验收 + 冷启动迁移评估（θ·q 预测地图 vs 实际）
- P05B 教法育种引擎（F5，预登记准入，τ 后验 >0.8 胜在位者才入库）
- P05C ingest 适配器插件化（识屏/浏览器等，独立进程，按需）

## Backlog（票外发现的问题记在这里，不顺手做）

- P03A 审查后续：当前 Q 降级初始化在单 Rust pack 下使用 `q[0]=1.0` 作为 deterministic one-hot track 维；多 pack/多 track 前需补 `latent.dims` 或 pack/track→维度映射，避免所有概念共用同一潜因子。
- 强化轴线候选（详见 `docs/ENHANCEMENT_ROADMAP.md` 2026-06-12 提案，排序待用户裁决）：
  - ~~P03K 心智动力学拟合层激活~~ ← 已转正式票（见 Phase 3 列表）。
  - 索引审计（全库无 CREATE INDEX；json_extract 热路径）+ DATA_MODEL §11 性能预算回归基准；服务环节：全环节（Tier 0 预算铁律）。
  - 属性测试扩面（G_u 生命周期决定性、镜像报告决定性、HMM 数值稳定）+ `polaris backup`/完整性自检；服务环节：全环节（Local-persistent 铁律）。
  - MCP 工具面补全：相图/交错 batch/G_u/镜像报告暴露为 Tier 2 工具；服务环节：验证真懂 → 定位模糊 → 针对性补缺。
  - 镜像报告 Tier 1 叙事润色（strict-citation 引断言原文，降级=现状断言列表）；服务环节：定位模糊。
  - 数学深化候选（全部带留出验证门）：校准后验化（分层 Beta-Binomial，复用 P03I 数学）、BKT-MIRT 逆方差加权融合、G_u 层级 Beta 超先验、相变马尔可夫动力学、θ AdaGrad 步长；不过门=假设，不进产品行为。
  - FSRS 个人参数拟合（`fsrs.w` C 类登记的预留票，FSRS-optimizer 思路 + 留出对拍门）。
