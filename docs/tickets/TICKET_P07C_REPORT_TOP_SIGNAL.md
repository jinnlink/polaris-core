# P07C 报告 top_signal + suggested_action

状态：已完成

服务主命题环节：定位模糊 -> 针对性补缺

## 背景

P03I 已经让镜像报告输出断言、假设、参数建议与跳过原因；P06D 进一步允许显式请求 Tier 1 叙事润色。但当前报告仍是多个数组并列，学习者无法快速回答：

1. 如果我只看一句，本周最该注意什么？
2. 看完每条断言后，我下一步能做什么？

P07C 在 P07A 的“可读语义层”之后，补一层报告可行动性：新增 `top_signal` 和每条 `ReportItem.suggested_action`。本票只扩展 schema 与 deterministic 派生字段，不改 admission、strict-citation、候选生成、LLM 叙事或报告证据门。

## 范围

1. `ReportItem` 增字段：
   - 新增：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub suggested_action: Option<String>
```

   - 按现有 `kind` 生成固定行动文案：
     - `calibration_phantom`：可以为该概念挑一道更高深度的验证题（迁移 / 自由解释）。
     - `hint_abandon_conditional`：下次连续求提示时，不妨先停下复述你对边界的理解，再看提示。
     - `abandon_time_contrast`：考虑避开高放弃率时段，或把该时段改为纯复习任务。
     - `gu_pattern`：针对该错误模式做一道反例 / 边界题，看能否独立识别。
     - `consolidation_hypothesis`：这是引擎提出的待验证假设，暂当参考、不必当结论。
     - `param_suggestion`：给开发者的参数复核建议，不影响你的今天。
     - `hazard_risk_summary`：今天可以适当降低任务强度，或把高摩擦任务往后挪。
   - 未识别 kind 返回 `None`。

2. `MirrorReport` 增字段：
   - 新增：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub top_signal: Option<ReportItem>
```

   - `top_signal` 从已保留的 `assertions + suggestions + hypotheses` 中确定性选择。
   - 产品/架构审查裁决：`param_suggestion` 默认不进入 `top_signal` 候选池。
   - 排序：
     - `score = confidence * kind_weight`
     - kind_weight：
       - `calibration_phantom` = 1.30
       - `gu_pattern` = 1.20
       - `abandon_time_contrast` = 1.10
       - `hazard_risk_summary` = 1.05
       - `hint_abandon_conditional` = 1.00
       - `consolidation_hypothesis` = 0.60
     - 平手：`kind` 字典序，再 `id` 字典序。
   - 若三个数组都为空，或只剩 `param_suggestion`，则 `top_signal = None`。

3. schema 兼容：
   - `suggested_action` 和 `top_signal` 都是可选字段。
   - 读取旧 `mirror_reports.report_json` 时缺字段应反序列化为 `None`。
   - 产品/架构审查裁决：不 bump `REPORT_SCHEMA_VERSION`；新增 Option 字段 + serde default 属于后向兼容变更。

4. CLI/输出：
   - `print_mirror_report` 在文本输出顶部展示 `top_signal`（若存在），格式例如：

```text
top_signal: <claim>
top_action: <suggested_action or ->

assertions: ...
```

   - JSON 输出天然包含新增字段。
   - MCP/HTTP 不新增新接口；读取现有报告资源时随 JSON 字段自然暴露。
   - JSON 消费者应温柔降级：`top_signal` / `suggested_action` 缺失或为 `None` 代表旧版报告或无可行动信号，不应崩溃。

5. 测试：
   - 新增/扩展 `p03i_mirror_report.rs`：
     - 报告 item 含 `suggested_action`。
     - `top_signal` 选择最高 score 且平手确定。
     - 空报告 `top_signal=None`。
     - 只有 `param_suggestion` 时 `top_signal=None`。
     - `calibration_phantom + param_suggestion` 同时存在时，`top_signal=calibration_phantom`。
     - 旧 JSON 缺字段能反序列化。
   - CLI 单测覆盖文本输出包含 `top_signal/top_action`。
   - P06D narrative 测试应继续通过，证明 strict-citation 只引用 claim，不受 action 字段影响。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core --test p03i_mirror_report
cargo test -p polaris-core --test p06d_mirror_report_narrative
cargo test -p polaris-cli report
```

专项验收要求：

- 不修改 `admit_assertion` / `admit_hypothesis` 的证据门、置信门、反馈抑制逻辑。
- 不修改 strict-citation 校验规则。
- 不修改候选生成 SQL 与统计公式。
- `suggested_action` 只由 kind 固定映射生成，不调用 LLM。
- `top_signal` 排序确定，同输入同输出。

## 禁区

- 不新增 LLM 调用或同步路径外部请求。
- 不自动调参、不改变 scheduler/MRT/breeding 行为。
- 不把假设当事实；`consolidation_hypothesis` 的 action 必须保持“待验证假设”语义。
- 不修改冻结参考仓库。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/` 等预存改动。

## 本轮范围（2026-06-15）

- 当前状态：P09B 已提交（`62e7671`）。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`。本票不得回退或混入这些改动。
- 产品/架构审查结果：按 6 条意见修订后进入实现；`param_suggestion` 不进入 top_signal，schema_version 不 bump。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md crates/polaris-core/src/report.rs crates/polaris-cli/src/main.rs crates/polaris-core/tests/p03i_mirror_report.rs crates/polaris-core/tests/p06d_mirror_report_narrative.rs
Remove-Item docs/tickets/TICKET_P07C_REPORT_TOP_SIGNAL.md
```

提交后：

```powershell
git revert <P07C-commit-sha>
```

## 交付记录（2026-06-15）

### 变更清单

- `crates/polaris-core/src/report.rs`：
  - `ReportItem` 新增可选 `suggested_action`。
  - `MirrorReport` 新增可选 `top_signal`。
  - 新增 `suggested_action_for_kind()` 固定映射。
  - 新增 `select_top_signal()`，过滤 `param_suggestion`，按学习者行动潜力权重确定性选出顶部信号。
  - `REPORT_SCHEMA_VERSION` 保持 1。
- `crates/polaris-cli/src/main.rs`：
  - `print_mirror_report()` 改为复用 `mirror_report_text()`。
  - 文本输出在报告顶部展示 `top_signal` / `top_action`，并与正文留空行。
- 测试：
  - `p03i_mirror_report.rs` 覆盖空报告无 top_signal、phantom action、param_suggestion 不浮顶、phantom beats param_suggestion、旧 JSON 兼容。
  - CLI 单测覆盖 top_signal/top_action 文本输出。
- 文档：
  - `QUEUE.md` 标记 P07C In Progress。

### TDD 红灯记录

```text
cargo test -p polaris-core --test p03i_mirror_report top_signal
error[E0609]: no field `top_signal` on type `MirrorReport`
error[E0609]: no field `suggested_action` on type `&ReportItem`
exit 101
```

```text
cargo test -p polaris-cli mirror_report_text_surfaces_top_signal_before_sections
error[E0560]: struct `ReportItem` has no field named `suggested_action`
error[E0560]: struct `MirrorReport` has no field named `top_signal`
error[E0425]: cannot find function `mirror_report_text` in this scope
exit 101
```

### 验收输出

```text
cargo test -p polaris-core --test p03i_mirror_report
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p06d_mirror_report_narrative
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-cli report
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.76s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 35 passed
polaris-core unit: 67 passed
p03i_mirror_report: 16 passed
p06d_mirror_report_narrative: 6 passed
all existing integration tests and doc-tests passed
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- `param_suggestion` 不进入 top_signal 候选池，避免把开发者/管理员调参建议置顶给学习者。
- `suggested_action` 是固定 kind 映射，不调用 LLM，不改变 strict-citation。
- `top_signal` 从已过 admission 的 items 中派生，不改变任何报告证据门。
- 新字段均为 Option + serde default，旧报告 JSON 可温柔降级。

### 待审事项

- 产品/架构审查结果：ship it。
