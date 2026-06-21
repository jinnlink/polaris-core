# TICKET P13A - AI IDE MCP 项目发现与学习入口

状态：**已实现、通过验收并提交**

## 背景

用户裁决后的产品方向：课程仓库不需要各自实现一套 Polaris MCP；学习时应在课程仓库里启动 AI IDE，由 AI IDE 连接同一个 Polaris MCP。AI 负责理解当前课程项目的 `p-os.toml`、课程已有教学入口和 Polaris 能力，再把学习过程中的证据、作答、状态反馈写回 Polaris。

本票只补齐 Polaris MCP 侧的通用入口，课程仓库 `C:\MyProject\Learned` 只读参考，不修改。

## 服务主命题

- 验证真懂：`submit_evidence` 继续走 engine-owned scoring；新增 `capture_evidence` 只做 raw capture，不伪造成掌握。
- 定位模糊：`get_learner_mirror` 让 AI IDE 能读取学习者镜像和近期信号。
- 针对性补缺：`detect_project_manifest` 让 AI IDE 知道当前课程项目绑定的 pack、入口命令和证据路径。

## 范围

1. MCP 新增 `detect_project_manifest` tool：
   - 输入 `path`，缺省为当前目录。
   - 向上查找 `p-os.toml`。
   - 找到时返回 `found=true`、`project_root`、`manifest_path`、`manifest`。
   - 未找到时返回 `found=false` 和查询起点，不报错。
2. MCP 新增 `capture_evidence` tool：
   - 语义与 HTTP `POST /capture` 对齐。
   - 写入 `evidence_items` + `capture_queue(status='pending')`。
   - 返回 `capture_id`、`evidence_id`、`status`、`learner_kind`、`recorded_only`、`message`。
   - 不写 `attempts`、`mastery_states`、`grade_queue`。
3. MCP 新增 `get_learner_mirror` tool：
   - 返回 `engine.learner_mirror_snapshot()` 的 JSON。
   - 稳定顶层字段与 HTTP `GET /learner-mirror` 对齐。
4. 更新 `docs/API_CONTRACT.md` 的 MCP v1 稳定工具面。
5. 增加 MCP 单元/契约测试，先红后绿。

## 禁区

- 不修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。
- 不新增每课程一个 MCP 的要求。
- 不新增独立 AI 中间层 crate / daemon；“AI IDE + Polaris MCP tools”就是本票的中间层。
- 不改变调度、评分、相图、掌握度公式或数据库表结构。
- `capture_evidence` 不接受外部 AI 评分作为 mastery 权威。

## 预计修改面

- `crates/polaris-cli/src/mcp.rs`
- `docs/API_CONTRACT.md`
- `docs/tickets/QUEUE.md`
- `docs/superpowers/plans/2026-06-21-p13a-ai-ide-mcp-project-discovery.md`

## 验收命令

红灯阶段：

```powershell
cargo test -p polaris-cli mcp_detect_project_manifest_returns_project_contract
```

绿灯与回归：

```powershell
cargo test -p polaris-cli mcp_detect_project_manifest_returns_project_contract
cargo test -p polaris-cli mcp_capture_evidence_records_pending_without_mastery
cargo test -p polaris-cli mcp_get_learner_mirror_returns_static_panel_fields
cargo test -p polaris-cli mcp_contract_document_names_stable_surface_and_policy
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 回滚方式

回滚本票对 `crates/polaris-cli/src/mcp.rs`、`docs/API_CONTRACT.md`、`docs/tickets/QUEUE.md`、本票文件和计划文件的改动即可；不涉及迁移和数据模型变更。

## AI 交付记录（2026-06-21 20:38 +08:00）

- 当前状态：已实现、通过验收并提交。
- 已完成：
  - MCP 新增 `detect_project_manifest`，从给定路径向上发现 `p-os.toml`，返回 `found/project_root/manifest_path/manifest`。
  - MCP 新增 `capture_evidence`，复用 capture queue 语义，只写 `evidence_items` + `capture_queue(status='pending')`，返回 `recorded_only=true`。
  - MCP 新增 `get_learner_mirror`，返回学习者镜像静态面板字段。
  - 更新 MCP v1 API contract，补齐工具名和关键语义。
  - 增加 MCP 工具列表、项目发现、capture recorded-only、learner mirror 和契约测试。
- 禁区遵守：
  - 未修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。
  - 未新增独立 AI 中间层、课程专属 MCP、数据库迁移、调度或掌握度公式。

### 红灯记录

```powershell
cargo test -p polaris-cli mcp_detect_project_manifest_returns_project_contract
```

输出摘录：

```text
running 1 test
test mcp::tests::mcp_detect_project_manifest_returns_project_contract ... FAILED
assertion `left != right` failed: tool returned error: "unknown tool: detect_project_manifest"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 80 filtered out
```

### 验收输出

```powershell
cargo test -p polaris-cli mcp_detect_project_manifest_returns_project_contract
```

```text
test mcp::tests::mcp_detect_project_manifest_returns_project_contract ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 80 filtered out
```

```powershell
cargo test -p polaris-cli mcp_capture_evidence_records_pending_without_mastery
```

```text
test mcp::tests::mcp_capture_evidence_records_pending_without_mastery ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 80 filtered out
```

```powershell
cargo test -p polaris-cli mcp_get_learner_mirror_returns_static_panel_fields
```

```text
test mcp::tests::mcp_get_learner_mirror_returns_static_panel_fields ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 80 filtered out
```

```powershell
cargo test -p polaris-cli mcp_contract_document_names_stable_surface_and_policy
```

```text
test mcp::tests::mcp_contract_document_names_stable_surface_and_policy ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 80 filtered out
```

```powershell
cargo test -p polaris-cli mcp_
```

```text
running 20 tests
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 61 filtered out
```

```powershell
cargo fmt --check
```

```text
Exit code: 0
```

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

首次沙箱内运行失败，原因是 `target/debug/deps/*.rmeta` 写入被拒绝访问；按环境规则授权重跑后通过：

```text
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 36.74s
```

```powershell
cargo test --workspace
```

输出摘录：

```text
test result: ok. 81 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 11s
Doc-tests polaris_core
```

### 阻塞点

- 无代码阻塞。
- `cargo clippy` 在沙箱内出现 target 写权限拒绝，授权重跑通过。

### 回滚方式

回滚本票修改文件：`crates/polaris-cli/src/mcp.rs`、`docs/API_CONTRACT.md`、`docs/tickets/QUEUE.md`、`docs/tickets/TICKET_P13A_AI_IDE_MCP_PROJECT_DISCOVERY.md`、`docs/superpowers/plans/2026-06-21-p13a-ai-ide-mcp-project-discovery.md`。
