# P16C 跨域预测地图

状态：已实现、通过验收并提交（`7dc54f2`）；依赖 P16B（`60effc0`）。

## 本轮范围（2026-08-08）

- 复用现有 q/θ、P03O 不确定度、结构/几何门和本地调度权威，建立只读预测地图契约。
- 优先保障用户一眼区分“做过并观测”“基于已有能力预测”“Pack 初始先验”，并始终得到 2–3 个可选择行动；不以审计字段淹没产品语义。
- 不新增 mastery 真相、不激活 shadow 融合、不写域特例、不实现 P17C 可视化前端。

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

## 交付记录（2026-08-08）

- 新增 `PredictionMapQuery` / `PredictionMapSnapshot` 共用 DTO 与 `Engine::prediction_map`。节点把 `observed`、`latent_prediction`、`inherited_prior` 做成三个独立可空槽位，各自携带值、95% 区间、来源、门状态、模型版本、θ 作用域与 provenance，从序列化层阻止把预测当成 mastery。
- 复用现有 `latent_prediction` 和 q/g2 信息量方差：shared Pack 显式标记 `cross_domain=true/theta_scope=shared`；isolated Pack 只读 `pack_theta`，标记 `cross_domain=false/theta_scope=pack:<id>`，改动 shared θ 不影响 isolated 预测。
- latent 在 P18A 前保持 `shadow`；低信息显示宽区间，q/g2 不确定度不可用时明确为 `unfit` 且区间 `[0,1]`，没有为工程完成降低科学门。
- 教学锚点只读取已持久化、跨 Pack、过 `graph.struct_threshold`、有 provenance 且来源有学习证据的 `maps_to` 边；输出结构分数、差异说明和 evidence IDs。无嵌入/无合格锚点时自然降级为空锚点，不调用网络或 LLM。
- 调度候选查询抽成 Pack-scoped 只读预览，最多返回 3 条带 move、prompt、phase 和 expected success 的初始路径；契约测证明查询不写 attempts、mastery_states 或 mrt_log。
- `scope=global` 保留 P16B 的 Pack/潜变量维度聚合，节点、锚点和路径为空，避免空白产品页也避免伪造未物化的全局节点预测。
- HTTP 新增 `GET /prediction-map`，MCP 新增 `get_prediction_map`；两者复用同一 DTO 和严格查询契约，未知/重复/非法参数保持可预期错误，稳定字段已写入 `docs/API_CONTRACT.md`。
- 未新增表、迁移、掌握度状态或域特例；未修改 P03O 默认融合行为。

## 验收记录（2026-08-08）

```text
> cargo test -p polaris-core --test p16c_prediction_map
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

> cargo test -p polaris-cli http_contract
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out

> cargo test -p polaris-cli mcp_prediction_map
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out

> cargo fmt --check
(exit 0; no output)

> cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.10s

> cargo test --workspace
456 tests across workspace suites; 456 passed; 0 failed

> git diff --check
(exit 0; no whitespace errors; Git reported only expected LF/CRLF conversion warnings for current working-copy files)
```

## 当前状态

- 阻塞点：无。
- 下一步建议：只提交并推送本票产品代码、契约、测试与票据文件；不混入既有 `.gitignore`、漫画/视觉文件、SQLite、target、编辑器目录或执行规划文件。推送后按单票制启动用户已授权继续的 P16D。
