# P16F 目标产品契约

状态：已实现并通过验收，等待提交；依赖 P16A。

服务主命题：针对性补缺。

## 范围

- 在现有 goals/dimensions/milestones 上新增 `GoalWorkspaceSnapshot`、稳定 CRUD、进度刷新与归档契约。
- 增加 HTTP goals 路由、MCP goals 工具和 P11B 合同测试；Tauri 后续直接调用同一 Core DTO。
- 用户选择目标时，只把候选限制在目标 Pack/维度/概念范围；范围内仍由本地 scheduler 给出 2–3 个行动。
- 目标进度只由证据和 mastery 推导，目标或里程碑不得直接写 `mastery_states`。

## 禁区

- 不让目标优先级覆盖 prerequisite、相图取证或评分权威。
- 不接入未验证画像干预；不实现桌面 UI。
- 不新增第二套 goals 表或域特定目标逻辑。

## 验收

- CRUD、维度权重、里程碑刷新、归档、非法范围、Pack 切换、目标候选 2–3 条和无目标兼容测试。
- HTTP/MCP/Core DTO 对齐；旧 P04D API 与数据不丢失。
- SPEC §6 基线、专项测试、`git diff --check` 全绿。

## 回滚

回滚本票公开契约与范围过滤；保留已有 P04D 目标数据和行为。

## 设计裁决（2026-08-09）

- 用户已确认在现有 `goals` 表增加通用 `scope_json` 列，稳定结构为 `{pack_ids, dimension_keys, concept_ids}`，不新增第二套 goals 表。
- 多种范围取交集，前置概念递归闭包始终纳入候选；空 scope 使用 active Pack，跨 Pack 目标不改写 active Pack。

## 交付记录（2026-08-09）

### 变更清单

- schema v9 为新旧 goals 增加可迁移、可回滚的 `scope_json`，旧数据默认空 scope。
- Core 增加目标范围校验、稳定 CRUD/归档、权威学习事实派生进度、里程碑刷新和 `GoalWorkspaceSnapshot`。
- 目标工作区复用现有 scheduler 返回 2–3 个行动，scope 只限制候选并保留 prerequisite 闭包；读取工作区不持久化进度。
- HTTP/MCP 新增 goals CRUD、归档、进度刷新与工作区入口，并更新 API/Data Model 合同。

### 实跑验收

- `cargo fmt --all -- --check` → 通过（无输出）。
- `cargo clippy --workspace --all-targets -- -D warnings` → 通过，`Finished dev profile ... in 1.34s`。
- `cargo test --workspace` → 通过：CLI 120/120、Core 81/81、P16F 7/7，全部集成测试与 doc-tests 0 failed。
- `git diff --check` → 通过（无输出）。

### 回滚方式

- 代码与公开契约使用 `git revert <P16F 提交哈希>` 回滚。
- schema v9 不做破坏性自动降级；需回到旧二进制时，使用升级前备份恢复数据库。旧 goals 在 v9 中保留且 scope 默认为空。
