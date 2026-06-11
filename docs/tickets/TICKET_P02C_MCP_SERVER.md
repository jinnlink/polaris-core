# P02C MCP server

状态：已完成待用户确认 commit

服务主命题：验证真懂 → 定位模糊 → 针对性补缺

## 背景

Phase 2 已有类型化超图与图谱感知诊断。下一步打开 Tier 2 门，让 Codex / Claude Code / IDE AI 这类外部导师可以通过 MCP 读取引擎状态、领取本地调度出的任务、提交学习者证据，并获取结构化教学指令。

MCP 是外部门，不是新调度权威。调度、诊断、评分口径仍归本地 Rust 引擎。

## 范围

1. 在 CLI 增加 `polaris mcp`，通过 stdio 提供 MCP JSON-RPC 服务。
2. MCP 资源：
   - `polaris://status`：只读状态快照，返回 due_today 与概念状态。
   - `polaris://concept/{id}/diagnosis`：只读图谱诊断。
3. MCP tools：
   - `get_next_task`：按本地引擎取下一题，并记录 `next` 行为事件。
   - `submit_evidence`：提交学习者证据/尝试，复用引擎乐观落账与 grader 降级路径。
   - `get_teaching_instruction`：按概念下发结构化教学指令，字段包含 `focus`、`move`、`target`、`do`、`dont`、`anchor`。
4. 增加核心层可测试结构：
   - 状态快照函数。
   - 教学指令构造函数。
   - MCP handler 的纯函数测试，stdio 仅作薄传输层。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-cli mcp_
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不引入 HTTP API、UI、MRT、HMM、MIRT、嵌入或夜间巩固逻辑。
- MCP tools 不得直接改 mastery/scheduler 参数；外部 AI 的判断只能经 `submit_evidence` 变成证据，由引擎评分/降级。
- MCP stdio 不得向 stdout 打印调试日志，stdout 只允许协议帧。
- 不修改冻结参考仓库。

## 交付记录

### 变更清单

- 新增 `polaris mcp` 子命令，提供 stdio MCP JSON-RPC server。
- 新增 MCP tools：
  - `get_next_task`：复用本地调度并记录 `behavior_events(type='next')`。
  - `submit_evidence`：复用 `Engine::submit`，走乐观落账与 grader 降级路径；主字段为 `concept_id`，兼容 `concept`。
  - `get_teaching_instruction`：返回 `focus`、`move`、`target`、`do`、`dont`、`anchor`。
- 新增 MCP resources：
  - `polaris://status`。
  - `polaris://concept/{id}/diagnosis` resource template。
- 新增核心只读模块：
  - `status_snapshot`。
  - `teaching_instruction`。
- 为 diagnosis/status/teaching 输出结构补充 JSON 序列化。
- 新增 P02C 测试，覆盖核心教学指令、状态快照、MCP tool/resource、stdio frame、`next` 行为事件记录。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
沙箱内失败：Windows target rmeta 写入拒绝访问（os error 5）。
同命令提权重跑：
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
exit 0
```

```text
cargo test --workspace
polaris-cli: 9 passed
polaris-core unit: 42 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-cli mcp_
6 passed; 0 failed
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 子 agent 审查

审查 agent：Ohm（`019eb532-7dd1-7b51-969a-1940a358f96c`）。

结论：

- Critical：无。
- Important：无。
- stdout 污染 MCP 协议风险：未发现。
- tool 越权直接改 mastery/scheduler：未发现。
- status/diagnosis resource：只读。

审查提出 2 个 Minor，均已修复：

- `submit_evidence` 支持 `concept_id`，兼容 `concept`。
- 增加 `get_next_task` 记录 `behavior_events(type='next')` 的回归测试。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-cli/src/main.rs crates/polaris-core/src/diagnosis.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-cli/src/mcp.rs crates/polaris-core/src/status.rs crates/polaris-core/src/teaching.rs crates/polaris-core/tests/p02c_teaching.rs docs/superpowers/plans/2026-06-11-p02c-mcp-server.md docs/tickets/TICKET_P02C_MCP_SERVER.md
```

提交后：

```powershell
git revert <P02C-commit-sha>
```
