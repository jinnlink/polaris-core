# P02C MCP server 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 polaris-core 打开 Phase 2 Tier 2 MCP 门，让外部 AI 读取状态、取任务、提交证据，并拿到结构化教学指令。

**架构：** 核心 crate 只提供可测试的状态快照与教学指令构造；CLI crate 提供 MCP JSON-RPC handler 与 stdio 传输。MCP 入口复用现有 Engine，外部 AI 不获得直接改掌握度或调度参数的能力。

**技术栈：** Rust 2021、rusqlite、serde/serde_json、手写 MCP stdio JSON-RPC 帧。

---

## 文件结构

- 创建 `crates/polaris-core/src/status.rs`：生成只读状态快照，供 CLI status 与 MCP resource 复用。
- 创建 `crates/polaris-core/src/teaching.rs`：把 P02B diagnosis 转成 `focus/move/target/do/dont/anchor` 教学指令。
- 修改 `crates/polaris-core/src/lib.rs`：导出新模块。
- 修改 `crates/polaris-core/src/engine.rs`：增加 `status_snapshot` 与 `teaching_instruction` 入口。
- 创建 `crates/polaris-core/tests/p02c_teaching.rs`：覆盖状态与教学指令核心行为。
- 创建 `crates/polaris-cli/src/mcp.rs`：MCP JSON-RPC handler、tool/resource 定义、stdio 帧读写。
- 修改 `crates/polaris-cli/src/main.rs`：增加 `polaris mcp` 子命令并复用 MCP server。
- 修改 `crates/polaris-cli/Cargo.toml`：添加 serde derive。
- 修改 `docs/tickets/TICKET_P02C_MCP_SERVER.md` 与 `docs/tickets/QUEUE.md`：交付记录。

## 任务 1：核心状态与教学指令红灯测试

- [ ] **步骤 1：写失败测试**

在 `crates/polaris-core/tests/p02c_teaching.rs` 创建测试：

```rust
#[test]
fn teaching_instruction_focuses_failed_prerequisite_gap() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_two_concepts_with_prereq(&conn);
    insert_failed_attempt(&conn, "target", 0.20);
    insert_mastery(&conn, "pre", 0.10);

    let instruction = teaching_instruction(&conn, "target").unwrap();

    assert_eq!(instruction.target, "pre");
    assert_eq!(instruction.move_name, "repair_prerequisite");
    assert_eq!(instruction.focus.kind, "prerequisite_gap");
    assert!(instruction.anchor.contains("target"));
    assert!(instruction.do_text.contains("先让学习者作答"));
    assert!(instruction.dont.contains("不要直接改掌握度"));
}
```

- [ ] **步骤 2：运行红灯**

运行：`cargo test -p polaris-core --test p02c_teaching`

预期：FAIL，原因是 `teaching_instruction` / `status_snapshot` 尚不存在。

## 任务 2：实现核心层

- [ ] **步骤 1：创建 `status.rs` 与 `teaching.rs`**

实现最少结构：

```rust
pub struct StatusSnapshot { pub due_today: i64, pub concepts: Vec<ConceptStatus> }
pub struct TeachingInstruction { pub focus: TeachingFocus, pub move_name: String, pub target: String, pub do_text: String, pub dont: String, pub anchor: String }
```

- [ ] **步骤 2：Engine 暴露入口**

在 `Engine` 增加：

```rust
pub fn status_snapshot(&self) -> Result<StatusSnapshot>;
pub fn teaching_instruction(&self, concept_id: &str) -> Result<TeachingInstruction>;
```

- [ ] **步骤 3：运行绿灯**

运行：`cargo test -p polaris-core --test p02c_teaching`

预期：PASS。

## 任务 3：MCP handler 红灯测试

- [ ] **步骤 1：写 MCP 纯 handler 测试**

在 `crates/polaris-cli/src/mcp.rs` 的测试中覆盖：

```rust
#[test]
fn mcp_lists_polaris_tools_and_status_resource() {
    let response = handle_json_rpc_for_test(initialized_engine(), json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list"
    }));
    assert_tool_names(response, ["get_next_task", "submit_evidence", "get_teaching_instruction"]);
}

#[test]
fn mcp_submit_evidence_records_attempt() {
    let response = handle_json_rpc_for_test(engine_with_pack(), json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "submit_evidence", "arguments": {
            "session": "mcp-test", "concept": "ownership", "response": "Ownership controls drops.", "confidence": 4
        }}
    }));
    assert_success_content_contains(response, "attempt_id");
}
```

- [ ] **步骤 2：运行红灯**

运行：`cargo test -p polaris-cli mcp_`

预期：FAIL，原因是 `mcp` 模块和命令尚不存在。

## 任务 4：实现 MCP handler 与 stdio

- [ ] **步骤 1：实现 JSON-RPC 方法**

支持：

- `initialize`
- `notifications/initialized`
- `tools/list`
- `tools/call`
- `resources/list`
- `resources/read`

- [ ] **步骤 2：实现 tools/resources**

tools：

- `get_next_task`
- `submit_evidence`
- `get_teaching_instruction`

resources：

- `polaris://status`
- `polaris://concept/{id}/diagnosis`

- [ ] **步骤 3：实现 stdio 帧**

按 MCP stdio 写 `Content-Length: <n>\r\n\r\n<json>`；stdout 不打印非协议文本。

- [ ] **步骤 4：运行绿灯**

运行：`cargo test -p polaris-cli mcp_`

预期：PASS。

## 任务 5：验收、审查与交付记录

- [ ] **步骤 1：全量验收**

运行：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-cli mcp_
git diff --check
```

- [ ] **步骤 2：子 agent 审查**

分派审查 agent，要求检查：

- MCP 协议 stdout 是否可能被污染。
- tools 是否越权直接改掌握度/调度。
- status/diagnosis 是否只读。
- 是否遗漏错误响应 `isError` 或 JSON-RPC error。

- [ ] **步骤 3：修复审查问题并重新验收**

若发现 Critical/Important 问题，先修复再重跑相关测试和全量验收。

- [ ] **步骤 4：填写票尾交付记录**

在 `docs/tickets/TICKET_P02C_MCP_SERVER.md` 写：

- 变更清单
- 实跑输出摘要
- 子 agent 审查结论
- 回滚方式
