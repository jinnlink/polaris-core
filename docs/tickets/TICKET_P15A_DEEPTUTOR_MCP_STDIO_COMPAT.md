# TICKET P15A：DeepTutor MCP stdio 双帧兼容

状态：已实现、通过验收并提交

服务主命题：验证真懂 → 定位模糊 → 针对性补缺；让 DeepTutor 通过 Tier 2 MCP 复用 Polaris 的证据、调度与学习者镜像闭环。

## 背景

Polaris 当前 `polaris mcp` 只接受并返回 `Content-Length` 帧；DeepTutor v1.5.7 使用当前 Python MCP SDK，其 stdio transport 使用一行一个 JSON-RPC 消息的 JSON Lines 帧。两边工具契约兼容，但传输帧不兼容，导致 DeepTutor 无法完成初始化。

本票只让同一个 stdio server 兼容两种帧。收到哪种帧，就用同一种帧回复，避免破坏已接入的 AI IDE 与 P14B smoke。

## 范围

- `polaris mcp` 输入支持：
  - 现有 `Content-Length: N\r\n\r\n<body>`。
  - 当前 MCP SDK 使用的单行 JSON + 换行。
- 每条有响应的请求使用该请求的输入帧格式回复。
- 保留通知无响应、工具/资源契约与错误形状。
- 增加两种帧的单元/集成测试。
- 更新 `docs/API_CONTRACT.md`，说明 stdio 帧兼容范围。
- 用 DeepTutor 所使用的 Python MCP SDK 对真实 `polaris.exe` 完成 `initialize`、`tools/list` 和只读工具调用。

## 禁区

- 不新增、删除或重命名 MCP tool/resource/prompt。
- 不修改 MCP 业务 payload、评分、掌握度、相图、调度、报告或学习算法。
- 不修改数据库 schema、迁移或参数。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不把 DeepTutor 特定逻辑写进内核；兼容能力属于通用 MCP stdio transport。

## 验收

必须真实运行：

```powershell
cargo test -p polaris-cli mcp_stdio
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

额外端到端检查：使用 DeepTutor 同版本 Python MCP SDK 启动真实 `polaris.exe`，完成：

- `initialize` 返回 `serverInfo.name == polaris-core`。
- `tools/list` 能发现稳定工具。
- `get_ai_interaction_profile` 返回可解析 JSON 文本。
- 进程正常关闭，stderr 无协议污染。

## 回滚方式

- 恢复 `crates/polaris-cli/src/mcp.rs`、`crates/polaris-core/tests/p05a1_algorithms.rs` 与 `docs/API_CONTRACT.md`。
- 删除本票并恢复 `docs/tickets/QUEUE.md` 状态。
- 本票无数据迁移，不需要数据库回滚。

## 实施与验证记录（2026-08-01）

已实现：

- `polaris mcp` 可读取 `Content-Length` 与 JSON Lines 两种 stdio 帧，并按单条请求的输入格式回复。
- 新增双帧 round-trip 与同帧回复测试；工具、资源、通知和业务 payload 未改。
- `docs/API_CONTRACT.md` 已记录传输兼容承诺。

已通过：

- `cargo test -p polaris-cli mcp_stdio`：3/3 通过。
- `powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1`：旧 `Content-Length` 实战通过。
- DeepTutor 同环境 `mcp==1.29.0` 启动真实 `polaris.exe`：`initialize`、22 tools、`get_ai_interaction_profile` 和干净 stderr 通过。
- DeepTutor v1.5.7 后端实际启动后，`/api/v1/settings/mcp` 报告 `polaris` 为 `connected`，并暴露 22 个命名空间化工具。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`git diff --check`：通过。
- `cargo test --workspace -- --skip failed_attempt_with_misconception_raises_repair_priority`：其余全量回归通过。

历史验证阻塞（已解除）：

- 原样执行 `cargo test --workspace` 时，既有 `crates/polaris-core/tests/p05a1_algorithms.rs::failed_attempt_with_misconception_raises_repair_priority` 稳定失败：期望 `greedy`，实际为 `complexity_basics`。
- 本票未修改 `crates/polaris-core/**`、调度算法、pack 或学习参数；该失败与 MCP 传输写集独立，因此未越过本票禁区修复。
- 2026-08-03 经用户裁决稳定化该测试的时间夹具后，原样 `cargo test --workspace` 已全绿；未修改生产调度逻辑。

## 本轮范围（2026-08-01）

- 用户明确要求部署个人 DeepTutor + Polaris 学习闭环，构成本票立项裁决。
- 已有非本票改动与未跟踪文件保持原样，不回退、不混入。
- 预计修改面：`crates/polaris-cli/src/mcp.rs`、`docs/API_CONTRACT.md`、`docs/tickets/QUEUE.md` 和本票文件。
- 测试门：先增加 JSON Lines 红灯测试，再实现双帧读取/同帧回复；最后跑 P14B 旧帧 smoke 与 DeepTutor SDK 新帧真实连接。

## 用户裁决与续跑记录（2026-08-03）

- 用户选择直接稳定化阻塞验收的测试夹具，不另开正式票。
- 根因：`p05a1_algorithms` 把误解尝试时间写死为 `2026-06-13`；当前日期已超出 `sched.mis_window_days=14`，导致“活跃误解”前提失效。
- 裁决后的额外修改面仅限 `crates/polaris-core/tests/p05a1_algorithms.rs`：改用 SQLite 当前 UTC 时间；不修改生产调度逻辑、参数或数据模型。

## 交付记录（2026-08-03）

### 变更清单

- `crates/polaris-cli/src/mcp.rs`：stdio 输入识别 `Content-Length` / JSON Lines，并按请求帧格式回复；新增 3 个 `mcp_stdio` 测试。
- `docs/API_CONTRACT.md`：登记双帧兼容、同帧回复、通知无响应和 stdout 纯协议约束。
- `crates/polaris-core/tests/p05a1_algorithms.rs`：误解窗口测试改用 SQLite 当前 UTC 时间，消除固定日期漂移；生产逻辑不变。
- `docs/tickets/QUEUE.md` 与本票：收敛状态、裁决、验收与回滚记录。

### 验收实跑输出

`cargo test -p polaris-core --test p05a1_algorithms failed_attempt_with_misconception_raises_repair_priority -- --exact`

```text
running 1 test
test failed_attempt_with_misconception_raises_repair_priority ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.02s
```

`cargo test -p polaris-cli mcp_stdio`

```text
running 3 tests
test mcp::tests::mcp_stdio_json_lines_round_trips_json_rpc ... ok
test mcp::tests::mcp_stdio_frame_round_trips_json_rpc ... ok
test mcp::tests::mcp_stdio_replies_using_request_framing ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 93 filtered out; finished in 0.02s
```

`powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1`

```text
capture_id: d3a38104-17cd-49e9-9334-019558f8767e
attempt_id: c1b5e3c1-ae28-4d9f-ba13-58b1cca761bf
P14B MCP real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14b-mcp-real-use-transcript.txt
```

DeepTutor v1.5.7 虚拟环境 `mcp==1.29.0` 对真实 `target\debug\polaris.exe`：

```text
serverInfo.name=polaris-core
tools=22
profile_json=ok keys=correction_style,custom_notes,explanation_depth,guidance,intervention_frequency,persona,proactivity,verbosity,version
stderr=clean
process=closed
```

`cargo fmt --check`

```text
exit code: 0（无输出）
```

`cargo clippy --workspace --all-targets -- -D warnings`

```text
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.96s
```

`cargo test --workspace`

```text
polaris-cli: test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s
polaris-core: test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
p05a1_algorithms: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
其余 workspace 集成测试与 doc-tests 全部通过；命令 exit code: 0，未跳过用例。
```

`git diff --check`

```text
exit code: 0；仅输出工作区既有 LF→CRLF 提示，无 whitespace error。
```

### 阻塞与技术选择

- 当前无阻塞。
- 双帧兼容只在 CLI stdio transport 层实现，不引入 DeepTutor 特定分支，不改变 MCP 工具、资源、payload 或内核行为。
- 测试夹具复用仓库既有的 `strftime('%Y-%m-%dT%H:%M:%SZ','now')` 形状，使“14 天内活跃误解”前提与运行日期一致。

### 回滚

- 按“回滚方式”恢复 3 个实现/测试文件与队列/票文档即可；无 schema 或用户数据迁移。
