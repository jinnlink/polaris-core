# P16I 未作答原因（Don't-Know as First-Class Signal）

状态：Queued；依赖 P01、P02B。P16D 提交前不得认领。

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
