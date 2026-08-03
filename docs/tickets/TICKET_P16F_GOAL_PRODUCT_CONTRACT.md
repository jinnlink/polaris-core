# P16F 目标产品契约

状态：Queued；依赖 P16A。

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
