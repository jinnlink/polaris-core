# TICKET P05C — ingest 适配器插件化

## 状态

已实现并提交

## 服务主命题

验证真懂：外部工具（识屏、浏览器、课程平台等）只能把证据和作答送入统一事件源，评分与掌握度仍由 Polaris Core evidence-bound 管线处理。

## 设计锚点

- `SPEC.md` §3：外部 AI 的判断只能作为证据 ingest，不得直接改掌握度或调度。
- `SPEC.md` §4：Domain Pack 纯声明式；域逻辑不得写进内核代码。
- `MASTER_PLAN.md`：识屏、浏览器扩展等移出内核，降为独立进程/插件，经 HTTP/MCP/公开入口接入。
- `docs/COURSE_INTEGRATION_PROTOCOL.md`：`ingest.toml` 是协议预留；领域适配器可以读取它，但必须通过公开入口提交 evidence 或 attempt，不得直接写 `mastery_states`。

## 范围

1. 定义 P05C v1 的标准适配器输出：JSON Lines，每行一个事件。
2. 支持两类事件：
   - `evidence`：写入 `evidence_items`，用于课程材料、上下文或外部证据。
   - `attempt`：调用 `Engine::submit`，触发现有 provisional/final grading、mastery fold、HMM/G_u 管线。
3. 扩展 CLI：
   - 保留既有 `polaris ingest --text ...`。
   - 新增 `polaris ingest --adapter-command <cmd> [--adapter-arg <arg> ...]`，运行独立进程并导入其 stdout JSONL。
4. 测试证明：
   - 外部 `final_score` / `external_score` 字段不会被信任。
   - 未知事件类型会被拒绝。
   - `attempt` 通过 `Engine::submit` 进入统一评分口径。

## 禁区

- 不实现识屏、浏览器扩展、课程平台爬虫等具体适配器。
- 不把 adapter 代码或域特定解析逻辑写进 core crate。
- 不让外部事件直接写 `mastery_states`、`theta`、`moves_effects`、调度权重或最终掌握度。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并写回输出：

```powershell
cargo fmt --check
cargo test -p polaris-cli
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

若默认 target 遭遇 Windows 文件锁，可追加同参数隔离 target clippy，但必须保留默认失败原文。

## 回滚方式

删除本票新增的适配器导入逻辑、CLI 参数、测试与协议文档改动；保留既有 `polaris ingest --text` 行为。

## 本轮范围（2026-06-13）

- 只做独立进程适配器 stdout JSONL 导入入口。
- 不做任何具体传感器/浏览器插件实现。

## 交付记录（2026-06-13）

### 变更清单

- 扩展 `polaris ingest`：
  - 保留 `--text` 直接写入 evidence 的旧行为。
  - 新增 `--adapter-command <cmd>` 与可重复 `--adapter-arg <arg>`，运行独立进程并导入 stdout JSONL。
- 新增 P05C JSONL 导入逻辑：
  - `evidence` 事件写入 `evidence_items`。
  - `attempt` 事件调用 `Engine::submit`，进入统一评分、mastery fold、HMM 与 G_u 管线。
  - 导入前先完整解析/校验 JSONL；未知事件类型直接拒绝，且不留下前序部分写入。
- 更新 `docs/COURSE_INTEGRATION_PROTOCOL.md`，写明 P05C adapter JSONL 协议与禁区。
- 新增 CLI 单元测试，覆盖 adapter 参数解析、evidence/attempt 导入、外部评分字段不被信任、未知事件整批拒绝、LLM 环境隔离。

### 技术选择

- 适配器是独立进程，Polaris 只读 stdout JSONL；stderr 只用于失败诊断。
- core crate 不引入域特定 adapter 逻辑；P05C 只在 CLI 层新增导入入口。
- JSONL 采用先解析校验、后落库的两阶段导入；解析阶段失败不会写入任何 evidence/attempt。
- `attempt` 导入不读取 `final_score` / `external_score`，而是交给 `Engine::submit` 重新评分。

### 验收输出

```text
> cargo fmt --check
exit 0
```

```text
> cargo test -p polaris-cli
running 12 tests
test tests::parses_required_command_set ... ok
test tests::adapter_jsonl_rejects_unknown_event_types ... ok
test tests::adapter_jsonl_ingests_evidence_and_attempt_without_trusting_external_score ... ok
...
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```text
> cargo test --workspace
首段：
running 12 tests
test tests::adapter_jsonl_rejects_unknown_event_types ... ok
test tests::adapter_jsonl_ingests_evidence_and_attempt_without_trusting_external_score ... ok
...
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

末段：
running 5 tests
test breeding_parameters_are_governance_gates ... ok
test preregistration_writes_audit_and_keeps_candidate_out_of_admitted_library ... ok
test admission_uses_frozen_preregistration_gates_not_current_meta ... ok
test candidate_admits_only_after_posterior_beats_incumbent_with_minimum_n ... ok
test admitted_move_retires_when_effect_decays_below_incumbent ... ok

Doc-tests polaris_core
exit 0
```

```text
> cargo clippy --workspace --all-targets -- -D warnings
默认 target 失败于 Windows 文件锁：
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)

> $env:CARGO_TARGET_DIR="$env:TEMP\polaris-core-target-p05c-clippy"; cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s
exit 0
```

### 代码审查处理

- 子 agent 只读审查发现：JSONL 边读边写，未知事件会导致前序 evidence/attempt 已经落库，与“拒绝整次导入”不一致。
- 修复：新增 `parse_adapter_jsonl` 与内部 `AdapterEvent`，先完整解析/校验所有行，再第二阶段写库或调用 `Engine::submit`。
- 回归：`adapter_jsonl_rejects_unknown_event_types` 改为先给一条合法 evidence，再给未知 `mastery`，断言失败后 `browser-fixture` evidence 数仍为 0。
- 子 agent 还指出：adapter attempt 测试可能受真实 LLM env 影响。已新增 `EnvGuard` 临时移除 `POLARIS_LLM_FAST_*` / `POLARIS_LLM_STRONG_*`，测试结束后恢复。
- 复验：`cargo test -p polaris-cli` 12/12；`cargo test --workspace` 通过；隔离 target clippy 通过。

```text
> git diff --check
仅有既存/仓库换行提示：
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-cli/src/main.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/COURSE_INTEGRATION_PROTOCOL.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it

> rg -n "[ \t]+$" crates\polaris-cli\src\main.rs docs\COURSE_INTEGRATION_PROTOCOL.md docs\tickets\TICKET_P05C_INGEST_ADAPTERS.md
无输出
```

### 阻塞与裁决记录

- P05C 无阻塞。
- 默认 target clippy 仍受 Windows 文件锁影响；按票据约定使用隔离 target 同参数通过。

### 回滚方式

删除 `polaris ingest --adapter-command/--adapter-arg` 分支、JSONL 导入 helpers、P05C CLI 测试、`docs/COURSE_INTEGRATION_PROTOCOL.md` 的 P05C 说明、QUEUE/P05C 票据改动；保留既有 `polaris ingest --text`。

## AI 交接记录（2026-06-13）

- 当前状态：P05C 已实现并提交。
- 已完成：票据、CLI adapter JSONL 入口、两阶段导入、协议文档、测试、子 agent 审查反馈修复、验收。
- 未完成：无。
- 已跑验证：`cargo fmt --check`、`cargo test -p polaris-cli`、`cargo test --workspace`、隔离 target `cargo clippy --workspace --all-targets -- -D warnings`。
- 未跑验证及原因：默认 target clippy 因 Windows 文件锁失败，已记录原文并用隔离 target 同参数通过。
- 阻塞点：无。
- 下一步建议：按 QUEUE/用户指定继续。
