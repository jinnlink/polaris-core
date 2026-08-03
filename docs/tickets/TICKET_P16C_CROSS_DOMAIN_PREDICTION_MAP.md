# P16C 跨域预测地图

状态：Queued；依赖 P16B。

服务主命题：定位模糊 → 针对性补缺。

## 范围

- 新增预测地图 DTO，在每个节点上严格分离 `observed`、`latent_prediction`、`inherited_prior`，包含值、区间、来源、模型版本和现有验证门状态。
- shared θ Pack 使用现有 `latent_prediction` 与 q/θ 不确定度；isolated θ Pack 不继承其他 Pack 画像或 θ。
- 结构/几何层只提议教学锚点；锚点必须通过现有结构门并携带 source concept、差异说明和 provenance。
- 为新装或低证据 Pack 输出 2–3 条初始候选路径；它们调用本地调度权威，不自动生成 attempt、mastery 或“已会”结论。
- 增加 Core facade、HTTP `GET /prediction-map`、MCP `get_prediction_map` 和合同测试。

## 禁区

- 不把 P03O shadow 融合切成默认 mastery；不让预测覆盖观测状态。
- 无嵌入时必须降级为潜因子/结构或无锚点，不调用同步 LLM。
- 不在内核写 Rust/英语/算法域特例。

## 验收

- shared/isolated、零证据/有证据、无嵌入、低置信、结构门失败、Pack 切换和决定性排序测试。
- UI/外部契约无法把预测字段误读为观测 mastery。
- 合成 leave-one-pack-out 只验证管线；真实科学结论留给 P18A。
- SPEC §6 基线、专项测试、`git diff --check` 全绿。

## 回滚

回滚本票提交；预测为只读派生结果，无业务数据清理。
