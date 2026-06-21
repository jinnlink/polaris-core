# P13A AI IDE MCP 项目发现与学习入口 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 让 AI IDE 连接一个通用 Polaris MCP 后，能发现当前课程项目、保存学习资料、读取学习者镜像。

**架构：** 不新增 AI 编排服务；MCP tool 作为薄门面调用已有 core API。`detect_project_manifest` 读 `project_manifest`，`capture_evidence` 读写 capture queue，`get_learner_mirror` 只读现有镜像快照。

**技术栈：** Rust workspace，`serde_json` MCP JSON-RPC 测试，`rusqlite` in-memory DB，`p-os.toml` 项目声明。

---

## 文件结构

- 修改：`crates/polaris-cli/src/mcp.rs`
  - 新增三个 tool 分发、tool schema、输入解析和单元测试。
- 修改：`docs/API_CONTRACT.md`
  - 把三个 tool 写入 MCP v1 稳定工具面和语义说明。
- 修改：`docs/tickets/QUEUE.md`
  - 标记 P13A 为唯一 In Progress。
- 创建：`docs/tickets/TICKET_P13A_AI_IDE_MCP_PROJECT_DISCOVERY.md`
  - 当前票范围、禁区、验收与回滚。

### 任务 1：认领正式票

- [x] **步骤 1：更新 QUEUE**

把队列状态改成 `P13A In Progress`，在 Phase 13 下新增 P13A 条目，不改已有完成票。

- [x] **步骤 2：确认单票制**

运行：

```powershell
Select-String -Path docs\tickets\QUEUE.md -Pattern "In Progress"
```

预期：只出现队列状态和 P13A 一张票。

### 任务 2：编写失败的 MCP 测试

- [x] **步骤 1：添加工具列表和契约测试断言**

在 `mcp_lists_polaris_tools_and_status_resource`、`mcp_contract_lists_stable_tools_resources_and_templates`、`mcp_contract_document_names_stable_surface_and_policy` 的 expected 列表加入：

```rust
"detect_project_manifest",
"capture_evidence",
"get_learner_mirror",
```

- [x] **步骤 2：新增项目发现测试**

在 `crates/polaris-cli/src/mcp.rs` tests 模块新增 `TestDir` helper 和测试 `mcp_detect_project_manifest_returns_project_contract`，创建 `p-os.toml` 后从子目录调用 tool，断言 `found=true`、`manifest.project_id`、`entry.today_command`、`project_root`、`manifest_path`。

- [x] **步骤 3：新增 capture evidence 测试**

新增 `mcp_capture_evidence_records_pending_without_mastery`，调用 `capture_evidence` 后断言返回 `recorded_only=true`、`status=pending`，并查询 `attempts/mastery_states/grade_queue` 仍为原数量。

- [x] **步骤 4：新增 learner mirror 测试**

新增 `mcp_get_learner_mirror_returns_static_panel_fields`，调用 `get_learner_mirror`，断言顶层包含 `generated_at`、`confidence_curve`、`phase_distribution`、`recent_assertions`。

- [x] **步骤 5：运行红灯**

运行：

```powershell
cargo test -p polaris-cli mcp_detect_project_manifest_returns_project_contract
```

预期：FAIL，原因是 `detect_project_manifest` tool 尚未实现。

### 任务 3：实现 MCP 薄封装

- [x] **步骤 1：引入 core API**

在 `mcp.rs` 顶部加入：

```rust
use polaris_core::capture_queue::{CaptureEffect, CaptureInput, LearnerCaptureKind};
use polaris_core::project_manifest::discover_project_manifest;
```

- [x] **步骤 2：扩展 tool 分发**

在 `call_tool` match 中加入：

```rust
"detect_project_manifest" => self.detect_project_manifest(arguments),
"capture_evidence" => self.capture_evidence(arguments),
"get_learner_mirror" => self.get_learner_mirror(),
```

- [x] **步骤 3：实现三个 handler**

`detect_project_manifest` 调用 `discover_project_manifest`；`capture_evidence` 按 HTTP `/capture` 相同默认值构造 `CaptureInput`；`get_learner_mirror` 调用 `engine.learner_mirror_snapshot()`。

- [x] **步骤 4：补输入 helper**

新增 `optional_string_array(value, "candidate_concept_ids") -> Result<Vec<String>, String>`，行为与 HTTP helper 对齐。

- [x] **步骤 5：补 tool schema**

在 `tool_definitions()` 加入三个工具定义，命名用 snake_case，参数描述明确默认值和限制。

### 任务 4：文档契约与验证

- [x] **步骤 1：更新 `docs/API_CONTRACT.md`**

MCP 稳定工具名加入：

```markdown
- `detect_project_manifest`
- `capture_evidence`
- `get_learner_mirror`
```

关键工具语义补充三条，与 HTTP `/capture`、`/learner-mirror` 对齐。

- [x] **步骤 2：跑聚焦测试**

运行：

```powershell
cargo test -p polaris-cli mcp_detect_project_manifest_returns_project_contract
cargo test -p polaris-cli mcp_capture_evidence_records_pending_without_mastery
cargo test -p polaris-cli mcp_get_learner_mirror_returns_static_panel_fields
cargo test -p polaris-cli mcp_contract_document_names_stable_surface_and_policy
```

预期：全部 PASS。

- [x] **步骤 3：跑 SPEC §6 基线**

运行：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

预期：全部 exit 0；若失败，记录真实输出并只修本票相关问题。

### 任务 5：交付记录

- [x] **步骤 1：更新本票尾部**

追加交付记录，包含变更清单、实跑验收输出、阻塞点、回滚方式。

- [x] **步骤 2：不提交**

按照仓库规则，等用户确认后再 commit。
