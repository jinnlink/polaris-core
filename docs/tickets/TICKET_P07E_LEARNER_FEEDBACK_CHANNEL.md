# P07E 学习者反馈通道扩展

状态：In Progress（2026-06-17 认领）
服务主命题环节：验证真懂

## 背景

P07B 已提供学习者状态镜子，P07C/P07D 已把镜像报告与任务响应做成可行动出口。但当前学习者能反馈的语义仍过窄：报告只能“标不准”，学习者无法明确说“这条判断是对的”、无法自报“我现在状态是 flow / frustrated”等，也无法表达“我想暂停”。

本票补齐学习者反馈入口。反馈先作为可审计事实进入 `behavior_events`，为后续校准和产品体验提供数据；本票不把用户自报直接写回掌握度、HMM 后验或调度策略。

## 范围

1. 报告断言反馈扩展：
   - 保留现有“标不准”能力。
   - 支持 `accurate` 与 `inaccurate` 两种 verdict。
   - `inaccurate` 继续参与现有报告抑制窗口。
   - `accurate` 只记录为校正反馈，不抬高掌握度，不取消既有抑制。
2. 学习者状态自报：
   - 新增受控状态反馈：`flow`、`productive_confusion`、`frustrated`、`bored`、`anxious`、`fatigued`。
   - 写入 `behavior_events.type='learner_feedback'`。
   - payload 至少包含 `kind='state'`、`state`、`source`、可选 `note`。
3. 暂停意图：
   - 新增 `pause` 反馈，表达“我想暂停 / 今天到这里”。
   - 写入 `behavior_events.type='learner_feedback'`。
   - payload 至少包含 `kind='pause'`、`reason`、`source`、可选 `note`。
   - 不把暂停记为 `abandon`，不污染 hazard / mental fit。
4. 对外入口：
   - CLI：新增 `polaris feedback state ...` 与 `polaris feedback pause ...`。
   - HTTP：新增 `POST /feedback`，返回稳定 receipt。
   - MCP：新增学习者状态反馈与暂停意图工具。
   - 现有 `report-feedback` 命令保持兼容，并支持 `--verdict accurate|inaccurate`。
5. 返回语义：
   - 新反馈入口返回稳定结构：`event_id`、`kind`、规范化字段、`effect='recorded_only'`。
   - 输出必须明确反馈已记录，但不会直接改分、改调度或覆盖模型状态。

## 预计修改面

- `crates/polaris-core/src/learner_feedback.rs`：新增反馈输入、校验、事件写入与 receipt。
- `crates/polaris-core/src/mental_state.rs`：如有必要，补受控状态名解析。
- `crates/polaris-core/src/report.rs`：扩展 report feedback verdict；保持 `inaccurate` 抑制逻辑。
- `crates/polaris-core/src/engine.rs`、`crates/polaris-core/src/lib.rs`：导出 facade。
- `crates/polaris-cli/src/main.rs`：CLI 反馈入口与文本输出。
- `crates/polaris-cli/src/http.rs`：`POST /feedback`。
- `crates/polaris-cli/src/mcp.rs`：反馈工具定义与处理。
- `crates/polaris-core/tests/p07e_learner_feedback.rs`：核心事件与校验测试。
- 现有 report/http/mcp/CLI 测试：补 P07E 专项覆盖。
- `docs/tickets/QUEUE.md` 与本票交付记录。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core --test p07e_learner_feedback
cargo test -p polaris-core --test p03i_mirror_report report_feedback
cargo test -p polaris-cli learner_feedback
cargo test -p polaris-cli http_learner_feedback
cargo test -p polaris-cli mcp_learner_feedback
```

专项验收要求：

- 状态自报写入 `behavior_events`，payload 字段稳定，非法状态被拒绝。
- 暂停意图写入 `behavior_events`，payload 字段稳定，且不生成 `abandon` 事件。
- 所有学习者反馈都不改 `mastery_states`、不改 `attempts`、不改 HMM 后验。
- `accurate` report feedback 能记录，且不抑制该断言后续入报。
- `inaccurate` report feedback 仍按既有逻辑抑制该断言。
- CLI/HTTP/MCP 三个入口都返回或展示 `recorded_only` 语义。
- 不新增 DDL，不触发 LLM，不修改冻结参考仓库。

## 禁区

- 不新增表、不改 `behavior_events` DDL。
- 不把用户自报状态直接覆盖 `mental_state` 或 HMM posterior。
- 不把 `pause` 写成 `abandon`。
- 不改 BKT、MIRT、FSRS、U(c)、相图判据或调度策略。
- 不做 P08A pack 切换、不做 P10A 信任面板。
- 不新增外发请求或 LLM 调用。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/`、atlas 计划等预存改动。

## 开工前复述（2026-06-17）

- 当前状态：P07B 已提交，当前无 In Progress 票，本票按产品路线图在 P07B 后认领。
- 本轮范围：只做学习者反馈事件通道，补 `accurate/inaccurate`、状态自报、暂停意图，并暴露 CLI/HTTP/MCP 入口。
- 禁区确认：不改表结构、不改调度、不改模型后验、不触发 LLM、不修改冻结参考仓库、不混入预存脏改动。
- 预计修改面：核心新增 `learner_feedback` 模块，扩展 report feedback，补 Engine facade 与 CLI/HTTP/MCP 入口，补专项测试。
- 验收命令：见上方“验收”。

## 回滚方式

## 交付记录（2026-06-17）

### 变更清单

- 新增 `learner_feedback` 核心模块，统一校验 `state` / `pause` 反馈并写入 `behavior_events.type='learner_feedback'`，返回稳定 receipt：`event_id`、`kind`、`session_id`、规范化字段与 `effect='recorded_only'`。
- 扩展 report feedback verdict：`accurate` / `inaccurate` 均可记录；只有 `inaccurate` 继续参与镜像报告断言抑制，`accurate` 不改 mastery、不改调度、不取消既有抑制。
- 新增 CLI 入口：`polaris feedback state`、`polaris feedback pause`；兼容扩展 `report-feedback --verdict accurate|inaccurate`。
- 新增 HTTP 入口：`POST /feedback`，对非法 JSON、非法 kind/state 返回稳定 400 错误。
- 扩展 MCP 工具：`record_learner_feedback`、`mark_report_assertion_accurate`，保留既有 `mark_report_assertion_inaccurate`。
- 补齐核心、CLI、HTTP、MCP 与 report feedback 专项测试；确认反馈事件不改 `mastery_states`、不改 `attempts`、不写 `abandon`、不触发 LLM。

### 审查记录

- 代码质量审查发现：`pause.reason` 不应是狭窄枚举；已改为自然文本、trim 后非空校验。
- 代码质量审查发现：CLI 只有 parser 测试不足；已补真实 `run(Cli)` 写入临时数据库的 state/pause/invalid state 测试。
- 规格复核结论：未发现阻塞问题。复核确认本票只写 `behavior_events`，未见 DDL/HMM/mastery/scheduler/LLM 越界写路径。
- 规格复核备注：输入层保留 `state_report` / `learner_state` / `pause_request` 与 `tired` / `fatigue` 等 UX 别名；落库 payload 仍规范化到冻结枚举，保留这些别名用于降低用户输入摩擦。

### 验收输出

```powershell
> cargo fmt --check
# exit code 0
```

```powershell
> cargo test -p polaris-core --test p07e_learner_feedback
running 3 tests
test learner_feedback_rejects_unknown_kind_or_state ... ok
test learner_feedback_records_state_report_as_behavior_event ... ok
test learner_feedback_records_pause_request_without_changing_mastery_or_attempts ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

```powershell
> cargo test -p polaris-core --test p03i_mirror_report report_feedback
running 3 tests
test report_feedback_for_unknown_assertion_is_rejected ... ok
test report_feedback_inaccurate_suppresses_assertion_in_next_report ... ok
test report_feedback_accurate_records_without_suppressing_assertion ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.02s
```

```powershell
> cargo test -p polaris-cli learner_feedback
running 10 tests
test tests::learner_feedback_text_reports_recorded_receipt ... ok
test tests::learner_feedback_flags_parse ... ok
test http::tests::http_learner_feedback_rejects_malformed_json_with_stable_error ... ok
test http::tests::http_learner_feedback_rejects_invalid_kind ... ok
test mcp::tests::mcp_learner_feedback_returns_tool_error_for_invalid_kind ... ok
test http::tests::http_learner_feedback_records_pause_request ... ok
test http::tests::http_learner_feedback_records_state_report ... ok
test mcp::tests::mcp_learner_feedback_records_state_and_pause ... ok
test tests::learner_feedback_command_rejects_invalid_state_without_event ... ok
test tests::learner_feedback_commands_record_state_and_pause_events ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.11s
```

```powershell
> cargo test -p polaris-cli http_learner_feedback
running 4 tests
test http::tests::http_learner_feedback_records_state_report ... ok
test http::tests::http_learner_feedback_rejects_malformed_json_with_stable_error ... ok
test http::tests::http_learner_feedback_rejects_invalid_kind ... ok
test http::tests::http_learner_feedback_records_pause_request ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 48 filtered out; finished in 0.02s
```

```powershell
> cargo test -p polaris-cli mcp_learner_feedback
running 2 tests
test mcp::tests::mcp_learner_feedback_returns_tool_error_for_invalid_kind ... ok
test mcp::tests::mcp_learner_feedback_records_state_and_pause ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 50 filtered out; finished in 0.01s
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
```

```powershell
> cargo test --workspace
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests polaris_core
```

```powershell
> git diff --check
# exit code 0；仅输出 CRLF 提示。
```

未提交前：

```powershell
git restore docs/tickets/QUEUE.md crates/polaris-core/src/lib.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/mental_state.rs crates/polaris-core/src/report.rs crates/polaris-cli/src/main.rs crates/polaris-cli/src/http.rs crates/polaris-cli/src/mcp.rs crates/polaris-core/tests/p03i_mirror_report.rs
Remove-Item crates/polaris-core/src/learner_feedback.rs
Remove-Item crates/polaris-core/tests/p07e_learner_feedback.rs
Remove-Item docs/tickets/TICKET_P07E_LEARNER_FEEDBACK_CHANNEL.md
```

提交后：

```powershell
git revert <P07E-commit-sha>
```
