# P15B Tier 2 可审计任务回合契约

状态：已实现、通过验收，等待用户确认后提交。

服务主命题：验证真懂 → 针对性补缺。

## 背景

P15A 已证明 DeepTutor 可经 MCP stdio 调用 Polaris。当前 `get_next_task` 会记录一条 `next` 行为事件，`submit_evidence` 则允许外部宿主自行携带概念、题型和题面提交。两者之间没有公开、可校验的关联标识，宿主无法让 Polaris 验证“此回答对应刚刚由本地调度发出的题”，也无法通过稳定回执审计一轮教学回合。

本票只为 Tier 2 外部导师补齐一条严格而可选的回合入口：

```text
get_next_task(session)
  -> task_event_id + task + teaching_instruction
submit_task_response(session, task_event_id, response, confidence)
  -> engine-owned scoring receipt
```

旧 `submit_evidence` 保持为通用证据入口；它不能被破坏，也不强制历史宿主迁移。

## 范围

1. `get_next_task` 返回本次已记录 `next` 行为事件的稳定 `task_event_id`。该事件继续由 `behavior_events` 记录，无新增表。
2. MCP 新增 `submit_task_response`：
   - 必填 `session`、`task_event_id`、`response`、`confidence`。
   - 从对应 `next` 事件复原 `concept_id`、`task_type` 与 `prompt`，宿主不得重新声明或覆写这些字段。
   - 校验回执存在、类型为 `next`、属于该 session，且尚未被本入口提交。
   - 成功后复用现有 `Engine::submit`，并写一条关联 `task_event_id` 与 `attempt_id` 的 `behavior_events(type='tier2_submission')` 审计事件。
   - 返回现有 `attempt_id`、`provisional_score`、`degraded`，并回显 `task_event_id`。
3. 补齐 MCP 工具 schema、`docs/API_CONTRACT.md` 的稳定契约说明及测试。

## 禁区

- 不新增数据库表或迁移；审计只复用 `behavior_events`。
- 不改 `attempts`、掌握度 fold、评分、LLM 队列、MRT、HMM、相图或任务选择算法。
- 不要求现有 `submit_evidence` 带回执，也不改变其 API 语义。
- 不新增 HTTP/CLI 对等入口；本票仅覆盖 Tier 2 MCP 会话。
- 不把外部 AI 判断、阅读记录或回执本身视为掌握度证据；只有 `response + confidence` 继续进入引擎评分路径。

## 预计修改面

- `crates/polaris-core/src/engine/task_selection.rs`：返回已写入的 next 行为事件 id，并增加回合提交关联的核心 facade。
- `crates/polaris-cli/src/mcp.rs`：输出回执、新增严格 MCP 工具及其 schema。
- `crates/polaris-cli/src/mcp.rs` 测试：覆盖正常回合、串 session、未知/非 next 回执、重复回执及旧入口兼容。
- `scripts/mcp_real_use_smoke.ps1`：用真实 stdio 子进程覆盖一次严格回合。
- `docs/API_CONTRACT.md`、本票与 `docs/tickets/QUEUE.md`。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项验收：

```powershell
cargo test -p polaris-cli mcp_audited_task_turn -- --nocapture
cargo test -p polaris-cli mcp_submit_evidence_records_attempt -- --nocapture
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -DbPath target\p15b-mcp-turn.sqlite -TranscriptPath target\p15b-mcp-turn-transcript.txt
git diff --check
```

验收断言：

- 同一 `get_next_task(session)` 返回的 `task_event_id` 可被 `submit_task_response` 成功提交，回答仍走 engine-owned scoring。
- `task_event_id` 不存在、不是 `next`、会话不匹配或已经提交时，拒绝且不创建 attempt、不改变 mastery。
- 审计事件带同一 `task_event_id` 与生成的 `attempt_id`。
- 原 `submit_evidence` MCP 路径仍能提交 attempt，且不要求 `task_event_id`。
- 新路径不新增迁移/表，不改变评分和调度结果。

## 回滚方式

回滚本票提交即可恢复。若手工回滚，删除本票文件，撤销 `QUEUE.md`、`API_CONTRACT.md`、`task_selection.rs`、MCP 与相关测试中的 P15B 变更；不需要回滚数据库迁移或删除业务数据。

## 本轮范围

- 按上述范围实现 MCP 严格任务回合契约。
- 不处理真实效果试点、来源定位、材料回跳、宿主 UI、LLM 健康面板或其他票外改进。

## 交付记录（2026-08-03）

- `get_next_task` 现在返回持久化 `next` 行为事件的 `task_event_id`。
- 新增 MCP `submit_task_response`：仅接受同一 session、未用过的回执；由事件复原概念、题型和题面，忽略宿主伪造的同名字段；回答仍复用 `Engine::submit`。
- 成功提交额外写入 `behavior_events(type='tier2_submission')`，payload 关联 `task_event_id` 与 `attempt_id`；未新增表或迁移。
- 更新 MCP schema、稳定 API 合同与真实 stdio smoke。单元测试覆盖正常回合、字段覆写无效、未知/跨 session/非 next 回执拒绝、重复回执拒绝与旧 `submit_evidence` 兼容。
- 未改评分、掌握度 fold、LLM 队列、调度、MRT、HMM、相图、HTTP 或 CLI 对等入口。

## 验收记录（2026-08-03）

```text
> cargo fmt --check
(exit 0)

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s

> cargo test -p polaris-cli mcp_audited_task_turn -- --nocapture
running 3 tests
test mcp::tests::mcp_audited_task_turn_rejects_unknown_or_cross_session_receipts_without_attempts ... ok
test mcp::tests::mcp_audited_task_turn_rejects_replayed_receipt_without_second_attempt ... ok
test mcp::tests::mcp_audited_task_turn_restores_issued_task_and_records_linked_receipt ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out

> cargo test -p polaris-cli mcp_submit_evidence_records_attempt -- --nocapture
running 1 test
test mcp::tests::mcp_submit_evidence_records_attempt ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out

> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -DbPath target\p15b-mcp-turn.sqlite -TranscriptPath target\p15b-mcp-turn-transcript.txt
task_event_id: fe78633b-4b42-4494-bb99-4520d02ee316
task_concept: ownership
turn_attempt_id: 39c81ca0-f54d-4ad1-992b-ac717021d8b9
P14B MCP real-use smoke passed.

> cargo test --workspace
running 99 tests
test result: ok. 99 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 80 tests
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
其余 workspace integration suites 均通过；最终 doc-tests：0 passed；0 failed。

> git diff --check
(exit 0; no output)
```

## 当前状态

- 阻塞点：无。
- 下一步建议：等待用户确认后，仅暂存并提交本票文件；不得混入既有 `.gitignore`、漫画文档、视觉文件、SQLite 或编辑器目录改动。

## 提交前复核补修（2026-08-03）

- 逐文件复核发现：原实现的“未提交检查 → `Engine::submit` → 审计事件”没有处于同一事务；若审计事件写入失败，可能留下无法关联的 attempt，并让同一回执再次提交。
- `submit_task_response` 现以 `BEGIN IMMEDIATE` 包住回执校验、引擎提交和 `tier2_submission` 审计写入；任一步失败都会回滚，不新增表或迁移，不改评分与调度语义。
- 新增故障注入测试：临时 trigger 强制拒绝审计事件，断言 attempt 为 0；移除 trigger 后同一回执仍可正常提交。

复核后的真实验收输出：

```text
> cargo fmt --check
(exit 0)

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.31s

> cargo test -p polaris-cli mcp_audited_task_turn -- --nocapture
running 4 tests
test mcp::tests::mcp_audited_task_turn_rejects_unknown_or_cross_session_receipts_without_attempts ... ok
test mcp::tests::mcp_audited_task_turn_rejects_replayed_receipt_without_second_attempt ... ok
test mcp::tests::mcp_audited_task_turn_restores_issued_task_and_records_linked_receipt ... ok
test mcp::tests::mcp_audited_task_turn_rolls_back_attempt_when_audit_write_fails ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out

> cargo test -p polaris-cli mcp_submit_evidence_records_attempt -- --nocapture
running 1 test
test mcp::tests::mcp_submit_evidence_records_attempt ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out

> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -DbPath target\p15b-mcp-turn-final.sqlite -TranscriptPath target\p15b-mcp-turn-final-transcript.txt
task_event_id: 39f1d4c3-0f55-47f9-a566-b3dc0ba64511
task_concept: ownership
turn_attempt_id: f584a911-1c99-4287-af52-e9e5cdd22214
P14B MCP real-use smoke passed.

> cargo test --workspace
polaris-cli: 100 passed; 0 failed
polaris-core: 80 passed; 0 failed
其余 workspace integration suites、性能预算与 doc-tests 全部通过；0 failed。

> git diff --check
(exit 0; no output)
```
