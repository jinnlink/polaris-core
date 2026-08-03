# P16B 实时知识地图契约

状态：Queued；依赖 P16A。

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
