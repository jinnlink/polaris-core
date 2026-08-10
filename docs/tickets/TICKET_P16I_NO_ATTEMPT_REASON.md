# P16I 未作答原因（Don't-Know as First-Class Signal）

状态：已实现、通过验收并提交（`7cbbd0b`）；依赖 P01、P02B。P16H 已提交（`81cd25f`）。

服务主命题：定位模糊 → 针对性补缺。

## 背景

掌握度通路上只有一个 `score` 标量。「我完全看不懂这题在问什么」和「我懂但边界用错了」被折算成同一个低分。

但两者的教学处方完全相反：前者要前置修复 + 工作样例，后者要辨析 + 反驳式对照。当前把「看不懂」当成「答错」喂进 BKT，产生两个错误——错误拉低 `p_known`，以及触发错误的修复策略。

`hint` 与 `abandon` 已经写 `behavior_events`，但不进入评分与诊断通路，因此这个区分在教学侧不可见。

## 范围

1. `attempts` 增加 `no_attempt_reason TEXT NULL`，枚举：`not_understood_prompt`、`no_recall`、`out_of_time`、`skipped`。`NULL` 表示正常作答。
2. 提交入口（CLI / HTTP / MCP）可选传该字段，非法枚举拒绝写入。
3. **关键约束**：带 `no_attempt_reason` 的 attempt **不进 mastery fold**——不更新 `p_known`、不更新 θ、不更新校准、不更新 FSRS。它只写 `evidence_items` + `behavior_events` + `attempts` 行。理由：「我不知道」不是作答证据，把它当低分会污染掌握度与校准。
4. 消费点**只有一个**：`teaching_instruction`。最近一次为 `not_understood_prompt` 时，focus 转前置修复或降目标深度，而不是同深度重出题。
5. `polaris diagnose` 输出该信号。
6. `session_summaries`（若 P16H 已落地）把它计入卡住点判据。

## 分层理由

本票只改「给 AI 的处方」，不改「排哪个概念」。

`no_attempt_reason` 显然也应该影响调度效用 `U(c)`，但那是改调度公式，必须带留出验证门。留后续票，本票不做。这样本票零公式改动，而教学处方立刻正确。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p16i_no_attempt_reason
cargo test --workspace
```

专项要求：

- 带 `no_attempt_reason` 的提交后，`mastery_states` 的 `p_known`、`calib_gap`、`attempt_count`、θ 全部不变（逐字段断言）。
- 非法枚举被拒绝且不产生任何写入。
- `not_understood_prompt` 后 `teaching_instruction` 的 focus 与目标深度确实改变，有对照用例。
- 正常作答路径行为完全不变（回归断言）。

## 禁区

- 不改 BKT、θ、校准、FSRS、相图、G_u。
- 不改 `U(c)` 或任何调度权重。
- 不把该信号当成「答错」。
- 不自动推断未作答原因，必须由调用方显式传入。
- 不修改冻结仓库。

## 回滚

删除 `attempts.no_attempt_reason` 列与相关分支；`teaching_instruction` 恢复原判定；移除测试。

## 开工前复述（2026-08-09）

- 范围：attempts 增加显式未作答枚举；CLI/HTTP/MCP 可选提交；未作答只留 evidence/event/attempt，不进入 mastery、θ、校准或 FSRS；仅 teaching instruction、diagnose 与 P16H 卡点消费。
- 禁区：不推断原因，不把它折成低分，不改 BKT/θ/校准/FSRS/相图/G_u，不改调度效用与候选顺序。
- 验收命令：票内专项、`cargo fmt --check`、workspace Clippy `-D warnings`、`cargo test --workspace`、`git diff --check`。
- 预计修改面：schema v5、提交 DTO/流水线、teaching/diagnose/session、CLI/HTTP/MCP 与 API/DATA_MODEL、专项和迁移回归、QUEUE 与本票。

## AI 交付记录（2026-08-09）

- schema 升至 v5，`attempts.no_attempt_reason` 使用数据库 CHECK 与 Core 枚举双层约束；稳定值为 `not_understood_prompt | no_recall | out_of_time | skipped`。
- 新增原子未作答提交：只写 session、evidence、`behavior_events(type='no_attempt')` 与无分数 attempt；不写 grade queue、mental state、mastery、θ、校准或 FSRS，且禁止后续 `apply_final_score` 补打分。
- 调度候选尝试计数与相图延迟样本显式排除未作答行；专项对照证明 next concept 不变，不修改 `U(c)`。
- `not_understood_prompt` 把教学处方降到 recall 工作样例；有未满足前置时转前置修复。diagnose 暴露最新原因，但不伪装为 latest failure。
- CLI `submit --no-attempt-reason`、HTTP `/evidence`、MCP `submit_evidence` 与严格 `submit_task_response` 均支持显式原因；非法值在任何写入前拒绝。
- P16H 小结新增向后兼容的 `no_attempt_count`，把未作答计入卡点但不计为“作答次数”。

### 最终验收输出

```text
> cargo test -p polaris-core --test p16i_no_attempt_reason
running 5 tests
test schema_v5_registers_the_nullable_enum_column ... ok
test invalid_reason_is_rejected_before_any_write ... ok
test no_attempt_is_recorded_without_changing_mastery_or_theta ... ok
test prompt_not_understood_changes_instruction_and_diagnosis_without_changing_schedule ... ok
test normal_submission_and_session_stuck_behavior_remain_explicit ... ok
test result: ok. 5 passed; 0 failed

> cargo fmt --check
exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> cargo test --workspace
polaris-cli: 113 passed; polaris-core: 81 passed
p02b_diagnosis: 4 passed; p16h_session_closeout: 3 passed
p16i_no_attempt_reason: 5 passed
all discovered suites: exit 0

> git diff --check
exit 0
```

### 真实 CLI 冒烟

```text
polaris submit ... --no-attempt-reason not_understood_prompt
attempt: <id> no_attempt_reason=not_understood_prompt
polaris diagnose --concept ownership
latest_failed: false
latest_no_attempt_reason: not_understood_prompt
polaris session close --session cli-no-attempt
会话 cli-no-attempt 已收口：0 次作答，触及 1 个概念。
最需要补缺：ownership
... 未作答 1 次。
```

- 回滚：执行 `git revert 7cbbd0b` 移除提交分支、处方与出口；已升级真实库不做破坏性降级，使用升级前备份恢复。旧二进制按 P11A 拒绝写入 schema v5。
