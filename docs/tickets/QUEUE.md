# 票队列（单票制）

状态：**P03A 已完成并提交；P03B 未认领**。任何时刻只允许 1 张票 In Progress。
新增票必须标注它服务主命题（验证真懂→定位模糊→针对性补缺）的哪一环。

## Phase 1 — Walking Skeleton

- [x] **P01 最小闭环**（`TICKET_P01_WALKING_SKELETON.md`）← 已实现并完成子 agent 审查补修；进入 P02 前需新开票认领

## Phase 2 — 图谱 + MCP

- [x] **P02A 类型化超图**（`TICKET_P02A_TYPED_HYPERGRAPH.md`）← 已实现并提交（`e876de3`）；服务环节：定位模糊
- [x] **P02B 图谱感知诊断**（`TICKET_P02B_GRAPH_AWARE_DIAGNOSIS.md`）← 已实现并提交（`0cec9f5`）；服务环节：定位模糊 → 针对性补缺
- [x] **P02C MCP server**（`TICKET_P02C_MCP_SERVER.md`）← 已实现并提交（`b11fa02`）；服务环节：验证真懂 → 定位模糊 → 针对性补缺；Tier 2 门

## Phase 3 — 潜因子 + 心智动力学

- [x] **P03A MIRT 潜因子层**（`TICKET_P03A_MIRT_LATENT.md`）← 已实现并通过验收；服务环节：定位模糊 → 针对性补缺
- P03B 夜间巩固 v1：残差因子分解→LLM 溯因→留出验证门+回滚（consolidation_runs）
- P03C 几何层：嵌入 + HNSW 候选检索
- P03D 状态 HMM + 行为发射 + 离散时 hazard 放弃模型
- P03E 知识相图视图 + 误解语法 G_u v1（前瞻预测验证）
- P03F 镜像报告 v1（每条断言带证据 id + 置信度，说不出证据不许进报告）
- P03G Bloom 深度评判入 grader（D 分量正式化）
- P03H 参数自调优 v1（B 类·重放途径：夜间反事实重放调参 + param_tuning_runs 审计，DATA_MODEL §12）

## Phase 4 — UI + MRT

- P04A Tauri 常驻小窗（100% Tier 0 秒开）+ 可展开工作区（状态镜子=相图）
- P04B HTTP API 门
- P04C MRT 微随机化引擎（预登记审计）+ 教法签名后验（F1）+ 个人摩擦曲线拟合（F3）
- P04D 目标引擎移植（goals/dimensions/milestones，参考 Polaris schema v9）

## Phase 5 — 第二 pack + 育种

- P05A 英语 pack（从 Polaris CEFR 表导出）：插拔验收 + 冷启动迁移评估（θ·q 预测地图 vs 实际）
- P05B 教法育种引擎（F5，预登记准入，τ 后验 >0.8 胜在位者才入库）
- P05C ingest 适配器插件化（识屏/浏览器等，独立进程，按需）

## Backlog（票外发现的问题记在这里，不顺手做）

- P03A 审查后续：当前 Q 降级初始化在单 Rust pack 下使用 `q[0]=1.0` 作为 deterministic one-hot track 维；多 pack/多 track 前需补 `latent.dims` 或 pack/track→维度映射，避免所有概念共用同一潜因子。
