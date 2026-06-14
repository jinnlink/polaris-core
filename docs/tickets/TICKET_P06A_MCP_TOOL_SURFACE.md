# P06A MCP 工具面补全

状态：已完成

服务主命题：验证真懂 → 定位模糊

## 背景

`docs/ENHANCEMENT_ROADMAP.md` 的强化轴线提案指出：MCP server 仍停在 P02C/P03G 工具集，只暴露 `get_next_task`、`get_interleaved_batch`、`submit_evidence`、`get_teaching_instruction`。P03E-I 已经具备相图、G_u、镜像报告等能力，但 Tier 2 外部导师还不能通过 MCP 直接读取这些状态与审计入口。

本票只做工具面补全：把已有 Engine 能力暴露给 MCP，不改变内核公式、表结构、验证门或调度行为。

## 本轮范围

1. 新增 MCP 工具：
   - `get_phase_snapshot`：返回当前相图分布与概念相状态，复用 `status_snapshot`。
   - `get_active_gu_rules`：按概念返回已验证/活跃的 G_u 规则，复用 `active_gu_rules_for_concept`。
   - `run_mirror_report`：生成并持久化镜像报告，复用 `run_mirror_report`。
   - `get_latest_mirror_report`：读取最近一次镜像报告，未生成时返回 `null`。
   - `mark_report_assertion_inaccurate`：把报告断言标记为“不准”，复用 `record_report_feedback`。
2. 更新 MCP 工具列表与输入 schema。
3. 增加 MCP 层回归测试，验证工具可列出、可调用、错误边界稳定。

## 验收

必须通过：

```powershell
cargo test -p polaris-cli mcp_lists_polaris_tools_and_status_resource
cargo test -p polaris-cli mcp_phase_gu_and_mirror_report_tools
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不新增表，不改迁移，不改 DATA_MODEL 公式。
- 不做镜像报告 Tier 1 LLM 润色；报告仍是 P03I 的 evidence-bound JSON 形态。
- 不让外部 AI 评分直接改掌握度；`submit_evidence` 口径不变。
- 不改 HTTP API，不改 UI。
- 不处理 `.gitignore`、`.cursor/`、`docs/visuals/` 等票外改动。
- 不修改 frozen 参考仓库。

## 当前状态

- P03N 已提交：`3732420 fix(P03N): 稳定几何候选池排序`。
- 当前工作区仍有票外旧改动：`.gitignore`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`。
- 已确认现有 MCP 工具为 `get_next_task`、`get_interleaved_batch`、`submit_evidence`、`get_teaching_instruction`。

## 交付记录

### 变更清单

- `crates/polaris-cli/src/mcp.rs` 新增 5 个 MCP 工具：
  - `get_phase_snapshot`：返回 `status_snapshot`，包含 `phase_counts` 与概念相状态。
  - `get_active_gu_rules`：按 `concept_id`/`concept` 返回 G_u 规则。
  - `run_mirror_report`：生成并持久化 P03I 镜像报告 JSON。
  - `get_latest_mirror_report`：返回最近报告，缺失时返回 `{ "report": null }`。
  - `mark_report_assertion_inaccurate`：记录报告断言“不准”反馈，写入 `behavior_events`。
- 更新 `tools/list` schema，明确外部 AI 不能通过这些工具改掌握度或绕过验证门。
- 新增 MCP 回归测试：
  - 工具列表必须包含新增工具。
  - 相图快照、G_u 规则、镜像报告生成/读取/标不准均可通过 `tools/call` 完成。
  - 缺少概念参数、缺少目标报告时返回 `isError: true`，且不写入 `report_feedback`。

### 红灯记录

```text
cargo test -p polaris-cli mcp_
running 8 tests
mcp::tests::mcp_lists_polaris_tools_and_status_resource ... FAILED
mcp::tests::mcp_phase_gu_and_mirror_report_tools ... FAILED

left: ["get_next_task", "get_interleaved_batch", "submit_evidence", "get_teaching_instruction"]
right: ["get_next_task", "get_interleaved_batch", "get_phase_snapshot", "get_active_gu_rules", "run_mirror_report", "get_latest_mirror_report", "mark_report_assertion_inaccurate", "submit_evidence", "get_teaching_instruction"]

mcp_phase_gu_and_mirror_report_tools: called `Result::unwrap()` on an `Err` value: Error("expected value", line: 1, column: 1)
6 passed; 2 failed
exit 1
```

### 验收输出

```text
cargo test -p polaris-cli mcp_
8 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-cli mcp_lists_polaris_tools_and_status_resource
1 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-cli mcp_phase_gu_and_mirror_report_tools
1 passed; 0 failed
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
failed: Windows target 目录文件锁，报错为 target/debug/deps/libpolaris_core-*.rmeta 写入被拒绝（os error 5），并伴随 incremental 目录 GC warning。
exit 1
```

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.16s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 24 passed
polaris-core unit: 67 passed
integration suites: all passed, including p03c_geometry 8 passed, p03m_latent_dims 3 passed, p05b_breeding 5 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

### 技术选择

- 只在 `McpSession` 里做工具包装，所有业务逻辑继续走 `Engine` 已有方法。
- `get_active_gu_rules` 复用现有引擎语义；如果规则处于 `validated`，引擎会按既有生命周期把它消费为 `active`。
- `mark_report_assertion_inaccurate` 只记录反馈事件，不改历史报告、不改 mastery state。

### 子 agent 审查

Faraday（`019ec519-4259-7d30-b27c-fc8045053d49`）只读审查结论：

- Critical：无。
- Important：无。
- Minor 1：建议补测 MCP 负向错误边界。已采纳：新增缺 `concept_id/concept`、缺报告时的 `isError` 断言，并确认不会新增 `report_feedback`。
- Minor 2：提交边界必须只包含 P06A 三个文件。提交时只 stage `crates/polaris-cli/src/mcp.rs`、`docs/tickets/QUEUE.md`、`docs/tickets/TICKET_P06A_MCP_TOOL_SURFACE.md`。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-cli/src/mcp.rs docs/tickets/QUEUE.md
git clean -f docs/tickets/TICKET_P06A_MCP_TOOL_SURFACE.md
```

提交后：

```powershell
git revert <P06A-commit-sha>
```
