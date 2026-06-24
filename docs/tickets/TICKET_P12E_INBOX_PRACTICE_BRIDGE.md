# TICKET P12E：Inbox Practice Bridge

状态：已实现、通过验收并提交。

服务主命题：验证真懂 → 定位模糊 → 针对性补缺。

## 背景

P12C 已能把外部资料保存为 raw capture，P12D 已能让学生或 AI IDE 把条目标记为 `practice_ready`。P12E 的任务是补上最小练习桥：从 `practice_ready` 条目生成可回答的 prompt 草案；只有学生提交回答并采集反馈前 `self_confidence` 后，才调用现有 `Engine::submit` 进入掌握度事实源。

## 范围

- 基于 capture 的 evidence 文本生成 deterministic prompt 草案。
- prompt 草案只面向已有候选概念；没有候选概念或候选概念不存在时返回可操作错误。
- 学生作答后必须提供 `self_confidence`（1..=5），再调用现有 `Engine::submit`。
- 成功提交后把 capture 状态置为 `practiced`，并记录可审计的桥接事件。
- CLI / HTTP / MCP 提供入口，输出学生可读文案。

## 禁区

- 不允许 raw evidence 直接改掌握度。
- 不接受外部 AI 的 `score`、`final_score`、`mastery` 作为掌握度权威。
- 不自动新增概念、不写 overlay pack、不编辑正式 pack（留给 P12F）。
- 不修改 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并粘贴输出：

```powershell
cargo test -p polaris-core --test p12e_inbox_practice_bridge
cargo test -p polaris-cli p12e
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 回滚方式

- 删除 P12E 新增练习桥模块/函数和测试。
- 从 CLI / HTTP / MCP 中移除 P12E commands/routes/tools。
- 恢复 `docs/API_CONTRACT.md`、`README.md`、`docs/AI_IDE_USAGE.md`、`docs/tickets/QUEUE.md` 与本票状态。
- 不需要数据库迁移回滚；本票复用 `capture_queue`、`attempts`、`behavior_events`。

## 本轮范围（2026-06-24）

- 已确认 QUEUE 无 In Progress，用户明确要求“推进”。
- 将 P12E 从候选拆成正式票并标记 In Progress。
- 预计修改面：P12E 核心 bridge、Engine facade、CLI/HTTP/MCP 出口、API 合同、README/AI IDE 使用说明、P12E 测试。

## 交付记录（2026-06-24）

### 变更清单

- 新增 `inbox_practice` 核心模块：从 `practice_ready` capture 生成确定性小题草稿；要求已有候选概念；draft 不写 attempt、mastery 或 grade queue。
- `Engine` 新增 `draft_inbox_practice` 与 `submit_inbox_practice` facade；提交时校验 `self_confidence`、session、response，复用现有 `Engine::submit`，成功后将 capture 标记为 `practiced` 并写入 `behavior_events(type='inbox_practice')`。
- CLI 新增 `polaris inbox practice --capture <id> [--json]` 与 `polaris inbox submit --capture <id> --response <text> --confidence <1..5> [--session <id>] [--json]`。
- HTTP 新增 `POST /inbox/practice` 与 `POST /inbox/practice/submit`。
- MCP 新增 `draft_inbox_practice` 与 `submit_inbox_practice` tools；工具描述明确外部 AI 分数字段不作为掌握度权威。
- 更新 `docs/API_CONTRACT.md`、`README.md`、`docs/AI_IDE_USAGE.md`，补齐 AI IDE 中间层从 capture 到 inbox 小题提交的用法。
- 新增/扩展 core、CLI、HTTP、MCP 测试，覆盖 draft 不产生掌握事实、submit 产生 attempt/mastery/grade_queue、invalid confidence 拒绝、外部 `external_score/final_score` 不落库。

### Claude 协商记录

按用户要求尝试通过 `codeagent-wrapper --backend claude` 做只读审查：

- 默认 Claude：失败，`API Error: 403 预扣费额度失败, 用户剩余额度: ＄0.370466, 需要预扣费额度: ＄0.399066`。
- `--model claude-3-5-haiku-20241022`：失败，`503 ... 无可用渠道（distributor）`。
- `--model claude-sonnet-4-5`：失败，`503 ... 无可用渠道（distributor）`。

因此本票没有可采纳的 Claude 审查输出；继续以本地红绿测试、接口合约测试和 SPEC §6 基线作为验收依据。

### 验收输出

> cargo test -p polaris-core --test p12e_inbox_practice_bridge

```text
running 4 tests
test inbox_practice_rejects_pending_or_unmapped_capture_without_mastery_facts ... ok
test submit_inbox_practice_rejects_invalid_confidence ... ok
test draft_inbox_practice_builds_prompt_without_creating_mastery_facts ... ok
test submit_inbox_practice_records_attempt_and_marks_capture_practiced ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

> cargo test -p polaris-cli p12e

```text
running 4 tests
test tests::p12e_inbox_commands_parse_practice_and_submit ... ok
test http::tests::p12e_http_inbox_practice_drafts_and_submits_without_trusting_external_score ... ok
test mcp::tests::p12e_mcp_drafts_and_submits_inbox_practice_without_external_score ... ok
test tests::p12e_inbox_submit_command_records_attempt_and_practiced_status ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 85 filtered out; finished in 0.19s
```

> cargo test -p polaris-cli mcp_contract

```text
running 5 tests
test mcp::tests::mcp_contract_document_names_stable_surface_and_policy ... ok
test mcp::tests::mcp_contract_initialize_keeps_stable_handshake_fields ... ok
test mcp::tests::mcp_contract_lists_stable_tools_resources_and_templates ... ok
test mcp::tests::mcp_contract_resource_reads_keep_stable_top_level_fields ... ok
test mcp::tests::mcp_contract_errors_keep_stable_shape ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 84 filtered out; finished in 0.03s
```

> cargo fmt --check

```text
```

> cargo clippy --workspace --all-targets -- -D warnings

```text
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.00s
```

> cargo test --workspace

```text
test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.87s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.43s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 回滚方式

- 删除 `crates/polaris-core/src/inbox_practice.rs` 与 `crates/polaris-core/tests/p12e_inbox_practice_bridge.rs`。
- 从 `crates/polaris-core/src/lib.rs`、`crates/polaris-core/src/engine.rs` 移除 P12E facade。
- 从 `crates/polaris-cli/src/main.rs` 移除 `inbox practice` / `inbox submit` 命令与测试。
- 从 `crates/polaris-cli/src/http.rs` 移除 `/inbox/practice`、`/inbox/practice/submit` 路由与测试。
- 从 `crates/polaris-cli/src/mcp.rs` 移除 `draft_inbox_practice`、`submit_inbox_practice` tools 与测试。
- 恢复 `docs/API_CONTRACT.md`、`README.md`、`docs/AI_IDE_USAGE.md`、`docs/tickets/QUEUE.md` 与本票状态。
- 不需要数据库迁移回滚；本票没有新增表或 schema。
