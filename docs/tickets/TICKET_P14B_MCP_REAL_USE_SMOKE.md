# TICKET P14B：MCP 真实会话 smoke v1

状态：Completed

服务主命题：验证真懂 → 定位模糊 → 针对性补缺；同时服务“AI IDE 真能接上 Polaris 用起来”。

## 背景

P14A 已经证明 CLI 层可以完成真实学习闭环：init、AI profile、项目声明、capture、inbox、practice、submit、learner mirror。下一步要验证 AI IDE 真正会走的 MCP stdio 通道：启动 `polaris.exe --db ... mcp`，用 JSON-RPC `Content-Length` framing 调用工具，而不是只跑 CLI。

本票不改变 MCP 工具契约，不新增工具，不改学习数学；它只新增一个可复跑的 MCP 真实会话 smoke 脚本和文档入口。

## 范围

- 新增 PowerShell smoke 脚本：
  - 构建 `polaris-cli`。
  - 使用 `target\p14b-mcp-real-use.sqlite` 临时库初始化 Rust pack。
  - 启动真实 MCP 子进程：`target\debug\polaris.exe --db <db> mcp`。
  - 将 MCP 子进程工作目录设为课程项目路径，默认 `examples\project-manifests\rust-mastery-lab`。
  - 使用 JSON-RPC `Content-Length` framing 调用：
    - `initialize`
    - `tools/list`
    - `detect_project_manifest`
    - `update_ai_interaction_profile`
    - `get_ai_interaction_profile`
    - `capture_evidence`
    - `list_learner_inbox`
    - `act_on_learner_inbox_item`
    - `draft_inbox_practice`
    - `submit_inbox_practice`
    - `get_learner_mirror`
  - 自动解析 MCP tool 返回的 JSON 文本，不要求用户手抄 id。
  - 对关键字段做脚本内断言。
  - 写完整 transcript 到 `target\p14b-mcp-real-use-transcript.txt`。
- 更新真实使用 smoke 文档和 AI IDE 使用指南，说明 CLI smoke 与 MCP smoke 的区别。
- 在票尾粘贴红灯输出、脚本实跑输出、transcript 摘要、基线验收输出。

## 禁区

- 不修改 mastery、FSRS、BKT/MIRT、相图、调度、评分、schema 或 MCP 工具契约。
- 不新增 MCP tool。
- 不启动桌面 UI 或新 daemon。
- 不写用户默认数据库；脚本默认只写 `target\` 下临时库和 transcript。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并粘贴输出：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14b-learned-mcp-real-use.sqlite -TranscriptPath target\p14b-learned-mcp-real-use-transcript.txt
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

脚本输出必须包含：

- `P14B MCP real-use smoke passed.`
- transcript 路径。
- capture_id。
- attempt_id。

transcript 必须包含：

- `initialize: polaris-core`
- `tools/list`
- `project_id: rust-mastery-lab`
- `recorded_only: true`
- `status: practice_ready`
- `prompt:`
- `provisional_score: 0.7`
- `confidence_curve`

## 回滚方式

- 删除 `scripts\mcp_real_use_smoke.ps1`。
- 恢复 `docs\REAL_USE_SMOKE.md`、`docs\AI_IDE_USAGE.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14b-*.sqlite*` 与 `target\p14b-*-transcript.txt`。

## 本轮范围（2026-06-24）

- 用户要求继续推进，目标从 P14A 的 CLI 真实闭环推进到 AI IDE 实际使用的 MCP stdio 真实会话。
- 本票只验证通道与现有工具链，不扩展产品架构。

## 交付记录（2026-06-24）

### 变更清单

- 新增 `scripts\mcp_real_use_smoke.ps1`：
  - 使用 `target\p14b-mcp-real-use.sqlite` 临时库。
  - 构建 `polaris-cli`，初始化 Rust pack。
  - 启动真实 `target\debug\polaris.exe --db <db> mcp` 子进程。
  - 将 MCP 子进程 cwd 设为课程项目路径，默认 `examples\project-manifests\rust-mastery-lab`。
  - 用 JSON-RPC `Content-Length` framing 调用 `initialize`、`notifications/initialized`、`tools/list` 和 AI IDE 学习链路 tools。
  - 自动解析 MCP tool 返回的 JSON 文本，自动抽取 `capture_id` 与 `attempt_id`。
  - 将完整会话写入 `target\p14b-mcp-real-use-transcript.txt`。
  - `DbPath` 与 `TranscriptPath` 必须位于本仓库 `target\` 下，避免误删或污染用户库。
  - MCP stdout 读取带超时，stderr 异步 drain，避免子进程异常时 smoke 永久挂死。
  - `submit_inbox_practice` 断言 provisional score 接近 `0.7`，避免正数误报。
- 更新 `docs\REAL_USE_SMOKE.md`，区分 CLI smoke 和 MCP smoke。
- 更新 `docs\AI_IDE_USAGE.md`，加入 MCP stdio 自检命令和真实课程路径示例。
- 更新 `README.md`，把 MCP smoke 放进最短自检路径。

### 红灯输出

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1
The argument 'scripts\mcp_real_use_smoke.ps1' to the -File parameter does not exist.
```

### 验收输出

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1
capture_id: 8f4d470f-cd5f-420d-8da4-883296af86fe
attempt_id: ba5fc53a-48e8-4174-9719-b56d98363318
P14B MCP real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14b-mcp-real-use-transcript.txt
```

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14b-learned-mcp-real-use.sqlite -TranscriptPath target\p14b-learned-mcp-real-use-transcript.txt
capture_id: b0e485d6-e2be-46b4-b4cc-607fa7575f11
attempt_id: 8e637151-476a-4c12-b017-1220fb0f604b
P14B MCP real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14b-learned-mcp-real-use-transcript.txt
```

transcript 关键行：

```text
initialize: polaris-core
initialized notification sent
tools/list: get_next_task, get_interleaved_batch, get_phase_snapshot, ...
project_id: rust-mastery-lab
default_pack: rust
today_command: cargo run -p labctl -- today --date {today}
recorded_only: true
status: practice_ready
prompt: 请用自己的话回答：这条资料和「Ownership」有什么关系？请解释关键点，并给出一个例子或反例。
attempt_id: ba5fc53a-48e8-4174-9719-b56d98363318
provisional_score: 0.700
confidence_curve: 1
```

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo clippy --workspace --all-targets -- -D warnings

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```

```powershell
> cargo test --workspace

running 93 tests
...
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s

running 80 tests
...
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
...
running 5 tests
test update_ai_interaction_profile_persists_student_preferences ... ok
test default_ai_interaction_profile_is_balanced_and_read_only ... ok
test update_ai_interaction_profile_trims_blank_custom_notes ... ok
test update_ai_interaction_profile_rejects_invalid_values_without_mutation ... ok
test update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

说明：`cargo test --workspace` 完整输出较长，本票保留关键分组与尾部结果；命令已按验收实跑，退出码 0。

### 回滚方式

- 删除 `scripts\mcp_real_use_smoke.ps1`。
- 恢复 `README.md`、`docs\REAL_USE_SMOKE.md`、`docs\AI_IDE_USAGE.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14b-*.sqlite*` 与 `target\p14b-*-transcript.txt`。
