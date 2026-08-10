# P16B 实时知识地图契约

状态：已实现、通过验收并提交（`60effc0`）；依赖 P16A（`fbcad7b`）。

## 本轮范围（2026-08-03）

- 新建只读知识地图 DTO 与查询实现，并通过 Engine、HTTP、MCP 复用同一序列化形状。
- 只读取现有权威表和派生状态；不新增掌握度事实、不实现预测地图或前端。
- 优先复用现有索引；只有实测查询计划证明必要时才新增迁移和索引。

服务主命题：定位模糊。

## 范围

- 新增 `KnowledgeMapQuery`、`KnowledgeMapSnapshot`、摘要、节点、边、分页游标及稳定序列化契约。
- 节点覆盖 concept/schema、Pack、R、当前 `p_known`、C、D、相、到期时间、尝试/证据数、不确定度和 provenance；边覆盖类型、方向、权重与来源。
- 默认查询 active Pack；全局视图先返回 Pack/潜在维度聚合，概念图按 root/depth/limit/cursor 分页。
- 所有值从 `concepts/edges/mastery_states/attempts/evidence_items` 和现有模型推导，不新增第二份掌握度状态表。
- 增加 Core facade、HTTP `GET /knowledge-map`、MCP `get_knowledge_map`；三处复用同一 DTO，并纳入 P11B 合同测试。

## 禁区

- 不实现预测地图、画像或前端；不允许用户直接编辑 mastery。
- 不调用 LLM，不改变调度、相判据、BKT/MIRT/FSRS。
- 不返回无 provenance 的派生边或把缺失数据填成确定事实。

## 验收

- 新库、三种 Pack、active Pack 切换、schema 节点、root/depth、分页和空图测试。
- 地图状态与现有 status/event replay 一致；HTTP/MCP 顶层字段合同稳定。
- 10k 节点查询有独立性能回归，默认视图不全量物化跨 Pack 概念。
- SPEC §6 基线、专项测试、`git diff --check` 全绿。

## 回滚

回滚本票提交；没有业务迁移时无需数据回滚。若新增只读索引，按迁移账本回退对应 schema 版本。

## 交付记录（2026-08-03）

- 新增 `KnowledgeMapQuery` / `KnowledgeMapSnapshot` 共用 DTO 与 `Engine::knowledge_map` 只读门面；默认读取 active Pack，支持 Pack、root/depth、相、到期、最低置信度、limit/cursor，并提供全局 Pack/潜变量维度聚合。
- 节点从 `concepts`、`mastery_states`、`attempts`、`evidence_items` 即时推导 concept/schema、R、`p_known`、校准、深度、相、到期、尝试/证据计数、不确定度与 provenance；观测节点携带真实关联 evidence IDs，未观测节点明确标成 `inherited_prior/prior_only`。
- 边只在两端进入当前页且 provenance 完整时返回；缺来源边不伪造，并通过 `summary.omitted_edges_missing_provenance` 审计。
- HTTP 新增 `GET /knowledge-map`，手工查询解析拒绝未知/重复参数和非法百分号编码；MCP 新增 `get_knowledge_map`。两者直接复用 Core DTO，稳定字段写入 `docs/API_CONTRACT.md`。
- 复用 `idx_concepts_pack`、`idx_attempts_concept_created`、`idx_edges_src/dst`；未新增表、掌握度状态、迁移或索引，回滚不涉及业务数据。
- 专项测试覆盖空库、Rust/Algorithms/English 三 Pack、active Pack 切换、schema、全局聚合、root/depth、分页、筛选、状态/事件重放一致性、证据来源、缺 provenance 边与 10k 节点预算。

## 验收记录（2026-08-03）

```text
> cargo fmt --check
(exit 0; no output)

> cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.47s

> cargo test -p polaris-core --test p16b_knowledge_map
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

> cargo test -p polaris-cli knowledge_map
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out

> cargo test -p polaris-cli contract
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 91 filtered out

> cargo test --release -p polaris-core --test p16b_knowledge_map default_10k_pack_query_stays_paginated_and_within_tier_zero_budget -- --exact
running 1 test
test default_10k_pack_query_stays_paginated_and_within_tier_zero_budget ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out

> cargo test --workspace
446 tests across workspace suites; 446 passed; 0 failed

> git diff --check
(exit 0; no whitespace errors)
```

## 当前状态

- 阻塞点：无。
- 下一步建议：仅提交并推送本票文件，不混入既有 `.gitignore`、漫画文档、视觉文件、SQLite、target 或编辑器目录改动；落地后按单票制认领 P16C。
