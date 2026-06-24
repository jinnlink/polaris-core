# TICKET P12D：Learner Inbox v1

状态：已实现、通过验收并提交

服务主命题：验证真懂 → 定位模糊 → 针对性补缺。

## 背景

P12C 已把外部学习材料写入 `evidence_items` + `capture_queue(status='pending')`，并明确 `recorded_only` 不影响掌握度。P12D 要把这层缓冲变成学生和 AI IDE 可用的学习收件箱：能看到刚保存的材料，能用轻动作处理它，但仍不能把 raw evidence 直接算作掌握。

## 范围

- 只读列出 `pending` / `mapped` / `practice_ready` 收件箱条目。
- 支持 `accept` / `defer` / `ignore` / `archive` 动作。
- CLI / HTTP / MCP 输出学生可读动作。
- `accept` 只把条目标成 `practice_ready`，为 P12E 练习桥接做准备；本票不生成 prompt、不创建 attempt、不评分。

## 禁区

- 不展示内部参数、θ、p_known、SQLite 细节。
- 不让学生直接编辑 pack、TOML、概念图谱。
- 不把 raw evidence、外部 AI 评分或学生自报直接写入 `attempts`、`mastery_states`、`grade_queue`。
- 不修改 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并粘贴输出：

```powershell
cargo test -p polaris-core --test p12d_learner_inbox
cargo test -p polaris-cli p12d
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 回滚方式

- 删除新增 learner inbox 模块、入口和测试。
- 恢复 `docs/API_CONTRACT.md`、`docs/tickets/QUEUE.md` 与本票状态。
- 不需要数据库迁移回滚；本票复用 P12C `capture_queue` 表。

## 本轮范围（2026-06-24）

- 已确认 QUEUE 无 In Progress，用户明确要求继续开发。
- 将 P12D 从候选拆成正式票并标记 In Progress。
- 预计修改面：`capture_queue` 上层读取/状态动作、Engine facade、CLI/HTTP/MCP 出口、API 合同、P12D 测试。

## AI 交付记录（2026-06-24 09:57 +08:00）

### 变更清单

- 新增 `learner_inbox` 核心模块，复用 P12C `capture_queue`，提供默认 open 状态列表和 `accept/defer/ignore/archive` 动作。
- `accept` 仅把条目标记为 `practice_ready`；所有动作均返回 `effect=recorded_only`，不写 `attempts`、`mastery_states`、`grade_queue`。
- 新增 CLI：`polaris inbox list`、`polaris inbox act --capture <id> --action <accept|defer|ignore|archive>`。
- 新增 HTTP：`GET /inbox`、`POST /inbox/action`。
- 新增 MCP tools：`list_learner_inbox`、`act_on_learner_inbox_item`。
- 更新 `docs/API_CONTRACT.md`、`README.md`、`docs/AI_IDE_USAGE.md`，补充学习收件箱用法和 AI IDE 调用边界。

### 验收输出

```powershell
> cargo test -p polaris-core --test p12d_learner_inbox
running 5 tests
test learner_inbox_lists_open_captures_with_student_readable_actions ... ok
test learner_inbox_accept_marks_practice_ready_without_creating_mastery_facts ... ok
test learner_inbox_mapped_item_uses_concept_name_without_exposing_raw_candidate_ids ... ok
test learner_inbox_ignore_and_archive_hide_items_from_default_open_list ... ok
test learner_inbox_can_filter_statuses_and_limits_results ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```powershell
> cargo test -p polaris-cli p12d
running 4 tests
test tests::p12d_inbox_commands_parse_list_and_act ... ok
test mcp::tests::p12d_mcp_lists_and_updates_learner_inbox_without_mastery_facts ... ok
test http::tests::p12d_http_inbox_lists_and_updates_capture_without_attempt_or_mastery ... ok
test tests::p12d_inbox_accept_command_marks_practice_ready_without_attempt_or_mastery ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 81 filtered out
```

```powershell
> cargo fmt --check
# 通过，无输出
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.18s
```

```powershell
> cargo test --workspace
test result: ok. 85 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

### 技术选择说明

- 本票不新增 schema，避免把 P12D 变成数据库迁移票。
- 学生可见 payload 不暴露 `candidate_concept_ids_json`、`p_known`、`theta`；若候选概念存在于 pack，只显示概念名作为 `concept_hint`。
- MCP 按读写拆分为两个 tool，便于 AI IDE 先看收件箱，再执行轻动作。

### 回滚方式

- 删除 `crates/polaris-core/src/learner_inbox.rs` 与 `crates/polaris-core/tests/p12d_learner_inbox.rs`。
- 从 `Engine`、CLI、HTTP、MCP 中移除 learner inbox wrapper、commands/routes/tools 与测试。
- 恢复 `docs/API_CONTRACT.md`、`README.md`、`docs/AI_IDE_USAGE.md`、`docs/tickets/QUEUE.md` 与本票状态。
- 无数据库迁移需要回滚。

### 当前状态

- 已完成：P12D 全部范围。
- 未完成：无。
- 阻塞点：无。
- 下一步建议：用户确认后 commit；后续如继续开发，应由用户裁决是否将 P12E Inbox Practice Bridge 转正式票。
