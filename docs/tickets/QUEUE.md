# 票队列（单票制）

状态：**无 In Progress；P16A 已实现并通过验收，等待用户确认后提交；P15B 已提交（`a9fd13b`）**。任何时刻只允许 1 张票 In Progress。
P12F 已登记为正式排队票；P12G 是 `Learned` 仓库外部依赖门，必须另开该仓库正式票并取得写权限，不能从本队列直接认领。
P03E+ 优先级见 `docs/ENHANCEMENT_ROADMAP.md`（月度对齐见 `C:\MyProject\Learned\rust-mastery-lab\docs\ENHANCEMENT_ROADMAP.md`）。
**Phase 7+ 产品形态轴线见 `docs/PRODUCT_ROADMAP.md`**（轴 6 学习者形态 / 轴 7 多 Pack 承载 / 轴 8 工程演进 / 轴 9 信任面板）。
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
- [x] **P03O BKT-MIRT 融合不确定度传播 shadow gate**（`TICKET_P03O_BKT_MIRT_UNCERTAINTY_SHADOW.md`）← 已实现并通过验收；为当前 λ 融合新增逆方差 shadow 输出与不确定度，不改变主 `p_known` 行为；服务环节：定位模糊

## Phase 4 — UI + MRT

- [x] **P04A 桌面状态镜子契约（原规划标题含 Tauri，但未交付桌面应用）**（`TICKET_P04A_DESKTOP_STATUS_MIRROR.md`）← 已实现并通过验收；交付 Tauri/HTTP 可共用的 Tier 0 DTO 与 CLI JSON 出口，正式 Tauri 产品由 P17A–P17G 实现；服务环节：定位模糊 → 针对性补缺
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
- [x] **P06H 相变动力学 shadow gate**（`TICKET_P06H_PHASE_TRANSITION_DYNAMICS.md`）← 已实现并通过验收；用 `phase_transition` 事件构建 8x8 相迁移 shadow 统计、目标相期望步数与 holdout 验证摘要，不改变相判据、调度或默认产品行为；服务环节：定位模糊 → 针对性补缺
- [x] **P06I G_u 层级 Beta 超先验 shadow gate**（`TICKET_P06I_GU_HIERARCHICAL_BETA_SHADOW.md`）← 已实现并通过验收；用现有 `gu_rules`、同 pattern 历史与一跳图谱邻域构建层级 Beta shadow 先验，并与平坦 `Beta(1,1)` 做 holdout 对比，不改变 G_u 生命周期、调度或默认产品行为；服务环节：定位模糊 → 针对性补缺
- [x] **P06J FSRS 个人参数拟合**（`TICKET_P06J_FSRS_PERSONAL_FIT.md`）← 已实现并通过验收；`fsrs.w` C 类 Fit 路由，按个人复习史做显式 gated fit，过留出门才写入并重放 FSRS 状态；服务环节：定位模糊 → 针对性补缺

## Phase 7+ — 产品形态与工程演进

- [x] **P07A 相图产品化语义层**（`TICKET_P07A_PHASE_PRODUCT_SEMANTICS.md`）← 已实现并通过验收；8 个相图名加“产品名 + 一句话解读”映射，不改判据；服务环节：验证真懂 → 用户读懂
- [x] **P07B 学习者状态镜子 v1**（`TICKET_P07B_LEARNER_MIRROR.md`）← 已实现、通过验收并提交（`a7e37f1`）；只读聚合自信 vs 实际曲线、相分布、近期断言摘要，并提供学习者静态面板入口；服务环节：验证真懂 → 定位模糊
- [x] **P07C 报告 top_signal + suggested_action**（`TICKET_P07C_REPORT_TOP_SIGNAL.md`）← 已实现并通过验收；镜像报告加“如果只看一句”顶部提示与每条断言对应的可选行动；服务环节：定位模糊 → 针对性补缺
- [x] **P07D 行动闭环（相 → 任务响应策略）**（`TICKET_P07D_ACTION_LOOP.md`）← 已实现、通过验收并提交（`78efe0d`）；相图 Phase 转任务响应策略，带 MRT 预登记审计；服务环节：针对性补缺
- [x] **P07E 学习者反馈通道扩展**（`TICKET_P07E_LEARNER_FEEDBACK_CHANNEL.md`）← 已实现、通过验收并提交（`ecee5fb`）；在“标不准”之外加“我现在状态是 / 我想暂停 / 这条断言对了”等语义化触点；服务环节：验证真懂
- [x] **P08A 多 Pack 切换 + 数据隔离开关**（`TICKET_P08A_PACK_SWITCHING.md`）← 已实现并通过验收；`polaris pack switch/list` + 每 pack `shared|isolated` θ 模式，active pack 影响调度与状态；服务环节：全环节（通用性）
- [x] **P09A engine.rs 模块化拆分**（`TICKET_P09A_ENGINE_MODULARIZATION.md`）← 已实现并通过验收；拆 `engine/task_selection.rs`、`engine/submit_pipeline.rs`、`engine/mental_state.rs`，保留 `engine.rs` 薄 facade 与 public API；服务环节：全环节（可演进）
- [x] **P09B polaris config 浏览 CLI + 参数文档自动生成**（`TICKET_P09B_CONFIG_CLI_PARAMETERS.md`）← 已实现并通过验收；`polaris config list [--class A|B|C] [--tuning-route Replay|Mrt|Manual|Fit] [--json|--md]` + 参数文档同源生成/校验；服务环节：全环节（可治理）
- [x] **P09C polaris doctor --diagnose 全面诊断**（`TICKET_P09C_DOCTOR_DIAGNOSE.md`）← 已实现并通过验收；最近 7 天 tuning/breeding/mental_fit/GU/consolidation/report 摘要；服务环节：全环节（运维）
- [x] **P08B LLM 调用隐私清单 + 纯 Tier 0 模式**（`TICKET_P08B_LLM_PRIVACY_TIER0.md`）← 已实现并通过验收；新增外发调用清单、`polaris privacy show` 与 `POLARIS_TIER0_ONLY=1` 全禁外部模型调用；服务环节：信任前提
- [x] **P08C Pack 作者上手指南 + 模板 Pack**（`TICKET_P08C_PACK_AUTHOR_GUIDE_TEMPLATE.md`）← 已实现并通过验收；30 分钟上手文档 + `packs/template/` 最小可验证样板；服务环节：生态可用性
- [x] **P10A 五框架门状态面板 + 实验透明度**（`TICKET_P10A_TRUST_PANEL.md`）← 已实现、通过验收并提交；`polaris trust show` + 只读 HTTP/MCP 出口暴露 F1-F5 门状态、active breeding/MRT 实验与最近关键后台摘要；`breeding.min_n` 默认 6→20；服务环节：验证真懂（验证门可见）

## Phase 11 — 版本化与发布前硬化

- [x] **P11A 数据库 schema 版本化迁移**（`TICKET_P11A_SCHEMA_VERSIONED_MIGRATIONS.md`）← 已实现、通过验收并提交；为 SQLite schema 建立版本号、迁移账本与 doctor 可见性，保留既有数据和用户参数；服务环节：全环节（Local-persistent / 可演进底座）
- [x] **P11B MCP/HTTP API 稳定性合约**（`TICKET_P11B_API_CONTRACT_STABILITY.md`）← 已实现并通过验收；建立 HTTP/MCP 当前公开面契约、兼容/废弃规则和结构化 contract tests；服务环节：全环节（外部接口可演进底座）
- [x] **P11C Pack 作者沙箱模拟**（`TICKET_P11C_PACK_SANDBOX.md`）← 已实现并通过验收；复用 P04E simulation 与 P08C Pack 作者入口，提供不污染真实库的 `polaris pack sandbox <dir>`；服务环节：全环节（Pack 作者发布前验证）
- [x] **P11D 交接状态收敛**（`TICKET_P11D_HANDOFF_STATE_CONVERGENCE.md`）← 已实现并通过验收；清理旧路线图中的下一票误导，不改代码；服务环节：全环节（AI 协作可持续）

## Phase 12 — 无感学习入口与外部知识入库

- [x] **P12A 无感学习入口与外部知识入库规划**（`TICKET_P12A_LEARNING_CAPTURE_ROADMAP.md`）← 已实现并通过验收；落地产品路线图、P12 实施计划和后续票边界，不改代码；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P12B 学习项目声明 v1**（`TICKET_P12B_PROJECT_MANIFEST.md`）← 已实现并通过验收；`p-os.toml` 最小协议、向上发现、校验/展示入口与多项目样例；不做 capture queue，不改数据库；服务环节：验证真懂 → 定位模糊
- [x] **P12C Capture Queue v1**（`TICKET_P12C_CAPTURE_QUEUE.md`）← 已实现并通过验收；raw 外部知识先入库为 pending capture，返回 recorded_only，绝不生成 attempt 或 mastery；服务环节：验证真懂 → 定位模糊
- [x] **P12D Learner Inbox v1**（`TICKET_P12D_LEARNER_INBOX.md`）← 已实现并通过验收；让学生和 AI IDE 能查看并轻处理 capture queue，不生成 attempt、不改 pack；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P12E Inbox Practice Bridge**（`TICKET_P12E_INBOX_PRACTICE_BRIDGE.md`）← 已实现、通过验收并提交；把 `practice_ready` 收件箱条目转成 prompt 草案，并在学生作答+confidence 后调用 Engine::submit；服务环节：验证真懂 → 定位模糊 → 针对性补缺

## Phase 13 — AI IDE 中间层接入

- [x] **P13A AI IDE MCP 项目发现与学习入口**（`TICKET_P13A_AI_IDE_MCP_PROJECT_DISCOVERY.md`）← 已实现、通过验收并提交；把项目发现、raw capture、学习者镜像暴露为通用 Polaris MCP tools，使 AI IDE 可在任意课程仓库中连接同一个 Polaris；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P13B README 与 AI IDE 使用指南**（`TICKET_P13B_README_AI_IDE_USAGE.md`）← 已实现、通过验收并提交；把 README 和使用指南补到普通用户能照做，说明课程仓库、AI IDE、`p-os.toml` 与 Polaris MCP 的配合方式；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P13C AI Interaction Profile v1**（`TICKET_P13C_AI_INTERACTION_PROFILE.md`）← 已实现、通过验收并提交；让用户设置 AI 性格、话多话少、解释深度、主动程度和介入频率，并通过 CLI/HTTP/MCP 暴露给 AI IDE；服务环节：针对性补缺 → 用户可控

## Phase 14 — 真实使用验收

- [x] **P14A 真实使用 smoke v1**（`TICKET_P14A_REAL_USE_SMOKE.md`）← 已实现、通过验收并提交；把 init、AI profile、项目声明、capture、inbox、practice、submit、learner mirror 串成一键可复跑脚本和 transcript；服务环节：验证真懂 → 定位模糊 → 针对性补缺 → 用户能实际用起来
- [x] **P14B MCP 真实会话 smoke v1**（`TICKET_P14B_MCP_REAL_USE_SMOKE.md`）← 已实现、通过验收并提交；启动真实 `polaris.exe --db ... mcp` 子进程，用 JSON-RPC Content-Length framing 跑 AI IDE 会调用的项目发现、AI profile、capture、inbox、practice、submit、learner mirror；服务环节：验证真懂 → 定位模糊 → 针对性补缺 → AI IDE 真能接入
- [x] **P14C AI IDE 接入实战包 v1**（`TICKET_P14C_AI_IDE_ONBOARDING_KIT.md`）← 已实现、通过验收并提交；生成 AI IDE MCP 配置块、学习开场提示和检查清单，并用真实课程路径验证接入材料；服务环节：验证真懂 → 定位模糊 → 针对性补缺 → 用户能实际用起来
- [x] **P14D Learned 根目录自动接入 v1**（`TICKET_P14D_LEARNED_AUTO_CONNECT.md`）← 已实现、通过验收；从 `C:\MyProject\Learned` 只读扫描学习项目，生成无感 AI IDE 接入包；服务环节：验证真懂 → 定位模糊 → 针对性补缺 → 用户打开学习根即可实际用起来

## Phase 15 — 外部学习工作区接入

- [x] **P15A DeepTutor MCP stdio 双帧兼容**（`TICKET_P15A_DEEPTUTOR_MCP_STDIO_COMPAT.md`）← 已实现、通过验收并提交；保留现有 `Content-Length` 客户端兼容，同时接受当前 MCP SDK 使用的 JSON Lines stdio，使 DeepTutor 能调用同一套稳定 Polaris tools；服务环节：验证真懂 → 定位模糊 → 针对性补缺
- [x] **P15B Tier 2 可审计任务回合契约**（`TICKET_P15B_TIER2_AUDITED_TURN_CONTRACT.md`）← 已实现、通过验收并提交（`a9fd13b`）；为外部导师把“取题 → 学生原始作答 → 引擎提交”关联为可校验、可审计的 MCP 回合，不改评分数学或调度权威；服务环节：验证真懂 → 针对性补缺

## Phase 16 — 原始蓝图能力收口

- [x] **P16A 蓝图与 Atlas 收口**（`TICKET_P16A_BLUEPRINT_ATLAS_ALIGNMENT.md`）← 已实现并通过验收，等待用户确认后提交；建立蓝图能力—实现—票据状态矩阵，纠正 P04A/Tauri 与 Aura/Coursebook 边界，让开发者 Atlas 从 QUEUE 动态生成真实进度；服务环节：全环节（工程事实可审计）
- [ ] **P16B 实时知识地图契约**（`TICKET_P16B_KNOWLEDGE_MAP_CONTRACT.md`）← 依赖 P16A；从现有事实表推导分页/子图知识地图，统一 Core/HTTP/MCP/Tauri DTO，不建立第二份 mastery 真相；服务环节：定位模糊
- [ ] **P16C 跨域预测地图**（`TICKET_P16C_CROSS_DOMAIN_PREDICTION_MAP.md`）← 依赖 P16B；把 θ·q、结构/几何锚点和 2–3 条初始路径组成带置信度的预测地图，isolated Pack 禁止继承；服务环节：定位模糊 → 针对性补缺
- [ ] **P16D Global Learner Profile 数据与治理**（`TICKET_P16D_GLOBAL_PROFILE_GOVERNANCE.md`）← 依赖 P16A；画像测量注册、设置、事件、派生状态、验证记录、导出与两级清除；服务环节：验证真懂 → 定位模糊
- [ ] **P16E Global Learner Profile 估计与验证**（`TICKET_P16E_GLOBAL_PROFILE_ESTIMATION.md`）← 依赖 P16D；行为画像、慢特质后验、EMA 调度与前瞻验证，未过门不影响 mastery/调度；服务环节：定位模糊 → 针对性补缺
- [ ] **P16F 目标产品契约**（`TICKET_P16F_GOAL_PRODUCT_CONTRACT.md`）← 依赖 P16A；把已有 goals/dimensions/milestones 暴露为稳定契约，目标只限定候选范围；服务环节：针对性补缺

## Phase 17 — Tauri 正式桌面产品

- [ ] **P17A Tauri 产品底座**（`TICKET_P17A_TAURI_FOUNDATION.md`）← 依赖 P16B–P16F；Tauri 2 + React + TypeScript + Vite，直接调用 Core，共用 DTO 与瓷白设计令牌；服务环节：全环节（正式产品承载）
- [ ] **P17B 常驻小窗与 Today**（`TICKET_P17B_TRAY_TODAY.md`）← 依赖 P17A；托盘、单实例、小窗、Pack 切换、Today 与 2–3 个行动，心流态抑制通知；服务环节：定位模糊 → 针对性补缺
- [ ] **P17C 知识地图工作区**（`TICKET_P17C_MAP_WORKSPACE.md`）← 依赖 P17B、P16C；当前/预测/全局 Pack 地图、搜索筛选、钻取、证据与大图虚拟化；服务环节：定位模糊
- [ ] **P17D 学习工作台**（`TICKET_P17D_LEARNING_WORKBENCH.md`）← 依赖 P17B；Practice、Capture、Inbox、Practice Bridge 与乐观评分修正；服务环节：验证真懂 → 针对性补缺
- [ ] **P17E 画像与治理工作区**（`TICKET_P17E_PROFILE_GOVERNANCE_WORKSPACE.md`）← 依赖 P17C/P17D；Profile、Goals、Mirror、Reports、Trust、Privacy 与 Settings；服务环节：全环节（学习者知情与控制）
- [ ] **P17F 桌面生命周期**（`TICKET_P17F_DESKTOP_LIFECYCLE.md`）← 依赖 P17E；后台任务、数据库解析、启动项、密钥、托盘退出、崩溃恢复和日志导出；服务环节：全环节（本地可靠性）
- [ ] **P17G Windows 发布硬化**（`TICKET_P17G_WINDOWS_RELEASE_HARDENING.md`）← 依赖 P17F；无障碍、性能、视觉回归、NSIS、签名更新和 GitHub Releases CI；服务环节：全环节（公开发布）

## Phase 18 — Overlay、跨仓桥接与真实验收

- [ ] **P12F Concept Suggestion + Overlay Pack**（`TICKET_P12F_CONCEPT_OVERLAY_PACK.md`）← 依赖 P17G；从 Capture 提议带 provenance 的概念/边/图式，人工接受后写可回滚 overlay pack，绝不直接修改 mastery；服务环节：定位模糊 → 针对性补缺

> **P12G 外部依赖门**：P12F 完成后，在 `C:\MyProject\Learned\rust-mastery-lab` 另开“Coursebook ↔ Polaris”正式票并取得该仓库写权限；不得由本仓库执行 AI 直接修改冻结仓库，也不得复活 Aura。P12G 完成前禁止认领 P18A。

- [ ] **P18A 真实纵向验证**（`TICKET_P18A_LONGITUDINAL_VALIDATION.md`）← 依赖 P12F、外部 P12G；真实学习数据前瞻验证画像、跨域预测、HMM/MRT 与策略增益，失败结果保留且不降门槛；服务环节：验证真懂
- [ ] **P18B 1.0 发布验收**（`TICKET_P18B_RELEASE_ACCEPTANCE.md`）← 依赖 P18A；干净 Windows 环境完成安装—学习—更新—备份—恢复—卸载，发布签名 1.0；服务环节：全环节（最终交付）

## Backlog（票外发现的问题记在这里，不顺手做）

- **P12 历史拆分记录**：详见 `docs/LEARNER_CAPTURE_ROADMAP.md` 与 `docs/superpowers/plans/2026-06-18-p12-learner-capture-inbox.md`。P12D、P12E 已完成；P12F 已转为正式排队票；P12G 仅作为 `Learned` 仓库外部依赖门，不在本仓库直接认领。

- P03A 审查后续：当前 Q 降级初始化在单 Rust pack 下使用 `q[0]=1.0` 作为 deterministic one-hot track 维；多 pack/多 track 前需补 `latent.dims` 或 pack/track→维度映射，避免所有概念共用同一潜因子。→ 已转正式票 P03M。
- P05A 验收观察：`cargo test --workspace` 首次在 `p03c_geometry::geometry_candidates_use_hnsw_and_combined_scores` 偶发缺少 `schema:raii` 候选，导致同文件后续用例因 `ENV_LOCK` PoisonError 连锁失败；单跑 `cargo test -p polaris-core --test p03c_geometry` 通过，重跑 `cargo test --workspace` 通过。建议后续单独开票把 HNSW 候选测试改成确定性夹具或扩大候选池；服务环节：全环节（验证稳定性）。→ 已转正式票 P03N。
- 历史强化轴线候选（已转正或归档；来源见 `docs/ENHANCEMENT_ROADMAP.md` 2026-06-12 提案）：
  - ~~P03K 心智动力学拟合层激活~~ ← 已转正式票（见 Phase 3 列表）。
  - 索引审计（全库无 CREATE INDEX；json_extract 热路径）+ DATA_MODEL §11 性能预算回归基准；服务环节：全环节（Tier 0 预算铁律）。→ 索引审计已转 P03L 并完成；性能预算已转正式票 P06E。
  - 属性测试扩面（G_u 生命周期决定性、镜像报告决定性、HMM 数值稳定）；服务环节：全环节（验证稳定性）。→ 已转正式票 P06C。
  - `polaris backup`/完整性自检；服务环节：全环节（Local-persistent 铁律）。→ 已转正式票 P06B。
  - ~~MCP 工具面补全：相图/交错 batch/G_u/镜像报告暴露为 Tier 2 工具；服务环节：验证真懂 → 定位模糊 → 针对性补缺。~~ → 已转正式票 P06A 并完成。
  - 镜像报告 Tier 1 叙事润色（strict-citation 引断言原文，降级=现状断言列表）；服务环节：定位模糊。→ 已转正式票 P06D。
  - 数学深化候选（全部带留出验证门）：校准后验化（分层 Beta-Binomial，复用 P03I 数学）→ 已转正式票 P06F；θ AdaGrad 步长 → 已转正式票 P06G；BKT-MIRT 逆方差加权融合 → 已转正式票 P03O；相变马尔可夫动力学 → 已转正式票 P06H；G_u 层级 Beta 超先验 → 已转正式票 P06I；不过门=假设，不进产品行为。
  - FSRS 个人参数拟合（`fsrs.w` C 类登记的预留票，FSRS-optimizer 思路 + 留出对拍门）。→ 已转正式票 P06J。

- 历史产品形态轴线候选（已转正或完成；来源见 `docs/PRODUCT_ROADMAP.md`，历史排序与依赖背景见该文 §5）：
  - **P07A 相图产品化语义层**（S）→ 已转正式票 P07A：8 个相图名加"产品名 + 一句话解读"映射，不改判据；服务环节：验证真懂 → 用户读懂。
  - **P07B 学习者状态镜子 v1**（M）→ 已转正式票 P07B 并提交（`a7e37f1`）：复用 atlas 静态站基建扩出学习者实时面板（自信 vs 实际曲线、相分布、近期断言摘要）；服务环节：验证真懂 → 定位模糊。依赖 P07A。
  - **P07C 报告 top_signal + suggested_action**（S）→ 已转正式票 P07C 并完成：镜像报告加"如果你只看一句"顶部提示与每条断言对应的可选行动；服务环节：定位模糊 → 针对性补缺。依赖 P07A。
  - **P07D 行动闭环（相 → 任务响应策略）**（M）→ 已转正式票 P07D 并提交（`78efe0d`）：补 `BatchStrategy::PhantomChallenge` 等相专属调度分支，每条带留出验证门；服务环节：针对性补缺。依赖 P07A、P03F、P03G。
  - **P07E 学习者反馈通道扩展**（S）→ 已转正式票 P07E 并提交（`ecee5fb`）：在"标不准"之外加"我现在状态是 / 我想暂停 / 这条断言对了"等语义化触点；服务环节：验证真懂。依赖 P07B。
  - **P08A 多 Pack 切换 + 数据隔离开关**（M）→ 已转正式票 P08A 并通过验收：`polaris pack switch/list` + 每 pack 是否共享 θ 的开关；服务环节：全环节（通用性）。
  - **P08B LLM 调用隐私清单 + 纯 Tier 0 模式**（M）→ 已转正式票 P08B：`polaris privacy show` + `POLARIS_TIER0_ONLY=1`；服务环节：信任前提。Tauri/UI 大投入之前的必备。
  - **P08C Pack 作者上手指南 + 模板 Pack**（S）→ 已转正式票 P08C 并通过验收：30 分钟上手文档 + 5 概念样板 pack；服务环节：生态。依赖 P05A0。
  - **P09A engine.rs 模块化拆分**（M）→ 已转正式票 P09A：拆 `engine/task_selection.rs`、`engine/submit_pipeline.rs`、`engine/mental_state.rs` + 测试搬到 `tests/`；服务环节：全环节（可演进）。
  - **P09B polaris config 浏览 CLI + 参数文档自动生成**（S）→ 已转正式票 P09B 并完成：`polaris config list [--class A|B|C]` + 自动生成 `docs/PARAMETERS.md`；服务环节：全环节（可治理）。
  - **P09C polaris doctor --diagnose 全面诊断**（S）→ 已转正式票 P09C 并完成：最近 7 天 tuning/breeding/mental_fit/gu/consolidation/report 摘要；服务环节：全环节（运维）。依赖 P06B。
  - **P10A 五框架门状态面板 + 实验透明度**（M）：`polaris trust show` 暴露 F1-F5 门状态、当前 breeding/MRT 活跃实验；同时把 `breeding.min_n` 默认从 6 提到 20；服务环节：验证真懂（验证门可见）。依赖 P03I、P05B、P04C。→ 已转正式票 P10A 并提交。
  - **P11A 数据库 schema 版本化迁移**（S）：为当前一次性 schema 建表路径补版本号、迁移账本和 doctor 可见性，作为后续 schema 演进底座。→ 已转正式票 P11A。
