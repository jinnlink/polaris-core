# 票队列（单票制）

状态：**P09A engine.rs 模块化拆分 In Progress**。任何时刻只允许 1 张票 In Progress。
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

- [x] **P03M 多 pack latent 维度映射**（`TICKET_P03M_LATENT_DIMENSION_MAPPING.md`）← 已实现并通过验收；从 P03A 审查后续转正，补齐 `latent.dims` / pack→维度映射，避免 Q 降级初始化把所有 pack 概念压到同一潜因子；服务环节：定位模糊 → 针对性补缺
- [x] **P03N 几何候选池确定性**（`TICKET_P03N_GEOMETRY_CANDIDATE_DETERMINISM.md`）← 已实现并通过验收；从 P05A 验收观察转正，扩大 HNSW 候选池并补确定性夹具，避免强综合分候选在最终排序前被截掉；服务环节：全环节（验证稳定性）

## Phase 4 — UI + MRT

- [x] **P04A Tauri 常驻小窗（100% Tier 0 秒开）+ 可展开工作区（状态镜子=相图）**（`TICKET_P04A_DESKTOP_STATUS_MIRROR.md`）← 已实现并通过验收；先交付 Tauri/HTTP 共用的 Tier 0 状态镜子契约与 CLI JSON 出口，不引入未验证桌面依赖；服务环节：定位模糊 → 针对性补缺
- [x] **P04B HTTP API 门**（`TICKET_P04B_HTTP_API.md`）← 已实现并通过验收；为常驻伴随 UI 暴露本地 HTTP 最小闭环：health/status/next/evidence；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P04C MRT 微随机化引擎（预登记审计）+ 教法签名后验（F1）+ 个人摩擦曲线拟合（F3）**（`TICKET_P04C_MRT_FRICTION_SIGNATURE.md`）← 已实现并通过验收；在 `next` 选 move 决策点加入审计化微随机与摩擦/签名后验；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P04D 目标引擎移植（goals/dimensions/milestones，参考 Polaris schema v9）**（`TICKET_P04D_GOAL_ENGINE.md`）← 已实现并通过验收；迁入 goals/dimensions/milestones 建模核心与 Engine 薄封装，不接入调度/MRT/HTTP/MCP；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P04E 学习模拟端到端测试**（`TICKET_P04E_LEARNING_SIMULATION_TEST.md`）← 已实现并通过验收；三画像 30 天模拟 + 每日 summary + 无死锁/相变/HMM/θ 跟踪断言；服务环节：验证真懂 → 定位模糊 → 针对性补缺（全闭环验证）

## Phase 5 — 第二 pack + 育种

- [x] **P05A0 课程接入协议 v1**（`TICKET_P05A0_COURSE_INTEGRATION_PROTOCOL.md`）← 已实现并提交；把 pack 文件形状、validator 规则、证据映射、评分 rubric、moves、版本兼容和外部课程作者指南文档化；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P05A1 算法 Domain Pack**（`TICKET_P05A1_ALGORITHMS_PACK.md`）← 已实现并提交（默认 target clippy 文件锁由隔离 target 同参数通过替代）；服务环节：验证真懂 → 定位模糊 → 针对性补缺（第二域验证领域无关性）
- [x] **P05A 英语示例 Domain Pack**（`TICKET_P05A_ENGLISH_PACK.md`）← 已实现并提交；从 Polaris CEFR 表形状导出 `examples/packs/english/` 示例 pack，覆盖课程接入协议插拔验收、语言学习 moves 与冷启动评估夹具；服务环节：验证真懂 → 定位模糊 → 针对性补缺（跨域桥首测）
- [x] **P05B 教法育种引擎**（`TICKET_P05B_PEDAGOGY_BREEDING.md`）← 已实现并提交；F5 预登记准入，τ 后验 >0.8 胜在位者才入库，效应衰减自动退役；服务环节：针对性补缺（方法库扩展必须先验证真懂）
- [x] **P05C ingest 适配器插件化**（`TICKET_P05C_INGEST_ADAPTERS.md`）← 已实现并提交；识屏/浏览器等只能做独立进程适配器，经标准事件导入，不进 core；服务环节：验证真懂（外部证据进入统一评分口径）

## Phase 6 — 强化轴线

- [x] **P06A MCP 工具面补全**（`TICKET_P06A_MCP_TOOL_SURFACE.md`）← 已实现并通过验收；把相图快照、G_u 活跃规则、镜像报告生成/读取/标不准暴露给 Tier 2 MCP，不改内核公式与数据模型；服务环节：验证真懂 → 定位模糊
- [x] **P06B 数据主权运维**（`TICKET_P06B_DATA_SOVEREIGNTY_OPS.md`）← 已实现并通过验收；补 `polaris backup` 与 `polaris doctor`，覆盖 SQLite 完整性检查和 mastery_states 事件溯源重放自检；服务环节：全环节（Local-persistent 铁律）
- [x] **P06C 属性测试扩面**（`TICKET_P06C_PROPERTY_TEST_EXPANSION.md`）← 已实现并通过验收；补 G_u 生命周期、镜像报告稳定字段、HMM 滤波数值稳定的属性测试；服务环节：全环节（验证稳定性）
- [x] **P06D 镜像报告 Tier 1 叙事润色**（`TICKET_P06D_MIRROR_REPORT_NARRATIVE.md`）← 已实现并通过验收；显式请求时用 Tier 1 将断言列表润色为周报叙事，strict-citation 引断言原文，失败降级为现状断言列表；服务环节：定位模糊
- [x] **P06E 性能预算回归**（`TICKET_P06E_PERFORMANCE_BUDGET_REGRESSION.md`）← 已实现并通过验收；把 DATA_MODEL §11 的 Tier 0 热路径预算做成可重复回归门；服务环节：全环节（Tier 0 预算铁律）
- [x] **P06F 校准后验化**（`TICKET_P06F_CALIBRATION_POSTERIOR.md`）← 已实现并通过验收；把幻影相判据从纯 EWMA 硬阈值升级为 Beta-Binomial 后验概率门，并让镜像报告复用同一校准摘要；服务环节：验证真懂 → 定位模糊
- [x] **P06G theta AdaGrad 步长**（`TICKET_P06G_THETA_ADAGRAD.md`）← 已实现并通过验收；把 MIRT θ 在线更新从固定步长改为每维 AdaGrad 自适应步长，仍保留 step cap 与在线梯度语义；服务环节：验证真懂 → 定位模糊

## Phase 7+ — 产品形态与工程演进

- [ ] **P09A engine.rs 模块化拆分**（`TICKET_P09A_ENGINE_MODULARIZATION.md`）← In Progress；拆 `engine/task_selection.rs`、`engine/submit_pipeline.rs`、`engine/mental_state.rs`，保留 `engine.rs` 薄 facade 与 public API；服务环节：全环节（可演进）


## Backlog（票外发现的问题记在这里，不顺手做）

- P03A 审查后续：当前 Q 降级初始化在单 Rust pack 下使用 `q[0]=1.0` 作为 deterministic one-hot track 维；多 pack/多 track 前需补 `latent.dims` 或 pack/track→维度映射，避免所有概念共用同一潜因子。→ 已转正式票 P03M。
- P05A 验收观察：`cargo test --workspace` 首次在 `p03c_geometry::geometry_candidates_use_hnsw_and_combined_scores` 偶发缺少 `schema:raii` 候选，导致同文件后续用例因 `ENV_LOCK` PoisonError 连锁失败；单跑 `cargo test -p polaris-core --test p03c_geometry` 通过，重跑 `cargo test --workspace` 通过。建议后续单独开票把 HNSW 候选测试改成确定性夹具或扩大候选池；服务环节：全环节（验证稳定性）。→ 已转正式票 P03N。
- 强化轴线候选（详见 `docs/ENHANCEMENT_ROADMAP.md` 2026-06-12 提案，排序待用户裁决）：
  - ~~P03K 心智动力学拟合层激活~~ ← 已转正式票（见 Phase 3 列表）。
  - 索引审计（全库无 CREATE INDEX；json_extract 热路径）+ DATA_MODEL §11 性能预算回归基准；服务环节：全环节（Tier 0 预算铁律）。→ 索引审计已转 P03L 并完成；性能预算已转正式票 P06E。
  - 属性测试扩面（G_u 生命周期决定性、镜像报告决定性、HMM 数值稳定）；服务环节：全环节（验证稳定性）。→ 已转正式票 P06C。
  - `polaris backup`/完整性自检；服务环节：全环节（Local-persistent 铁律）。→ 已转正式票 P06B。
  - ~~MCP 工具面补全：相图/交错 batch/G_u/镜像报告暴露为 Tier 2 工具；服务环节：验证真懂 → 定位模糊 → 针对性补缺。~~ → 已转正式票 P06A 并完成。
  - 镜像报告 Tier 1 叙事润色（strict-citation 引断言原文，降级=现状断言列表）；服务环节：定位模糊。→ 已转正式票 P06D。
  - 数学深化候选（全部带留出验证门）：校准后验化（分层 Beta-Binomial，复用 P03I 数学）→ 已转正式票 P06F；θ AdaGrad 步长 → 已转正式票 P06G；BKT-MIRT 逆方差加权融合、G_u 层级 Beta 超先验、相变马尔可夫动力学；不过门=假设，不进产品行为。
  - FSRS 个人参数拟合（`fsrs.w` C 类登记的预留票，FSRS-optimizer 思路 + 留出对拍门）。
