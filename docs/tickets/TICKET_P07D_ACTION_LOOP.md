# P07D 行动闭环（相 → 任务响应策略）

状态：已实现并通过验收（待用户确认 commit）

服务主命题环节：针对性补缺

## 背景

P07A 已把相图翻译成学习者能读懂的产品语义，P07C 已让镜像报告给出 `top_signal` 与 `suggested_action`。但当前调度仍主要按 U 值、HMM 门控状态和 MRT move 选择运行：系统能识别 `phantom`、`settling`、`regression` 等相，却还没有把这些相稳定地转成“下一组任务该怎么给”的响应策略。

本票补上从相到任务的第一层闭环。它只使用已经持久化在 `mastery_states.phase` 里的相，不改变相判据、不改变掌握度公式、不新增 DDL、不把未验证的策略包装成已证实有效。

## 范围

1. Phase-aware batch strategy：
   - 扩展现有 `BatchStrategy`：
     - `PhantomChallenge`
     - `SettlingProbe`
     - `RegressionRecovery`
   - `Default`、`EasyReviews`、`Flow` 现有语义保持。
   - HMM 门控仍然有效：`strategy_enabled=false` 时不因为 HMM 状态改变策略；相策略只消费已存储相，不触发重新判相。

2. 相策略触发规则：
   - `PhantomChallenge`：
     - 当 ranked candidates 中存在 `Phase::Phantom`，且不是疲劳/无聊等 easy-review HMM 策略时触发。
     - batch 至少优先放入 1 个 phantom 概念。
     - 对 phantom 概念的任务 move 至少提升到 `transfer`（硬题确认），缺 pack 模板时走现有 fallback template。
   - `SettlingProbe`：
     - 当 ranked candidates 中存在 `Phase::Settling`，且没有 phantom 更高优先级时触发。
     - batch 至少优先放入 1 个 settling 概念。
     - 对 settling 概念优先派 `transfer`，作为新情境探针。
   - `RegressionRecovery`：
     - 当 ranked candidates 中存在 `Phase::Regression`，且没有 phantom / settling 更高优先级时触发。
     - batch 至少优先放入 1 个 regression 概念。
     - 对 regression 概念降低到 `recall` 或 `explain`，避免直接继续高摩擦迁移题。

3. Move 覆写边界：
   - 新增小函数（命名实现时就近决定），把 `candidate.phase` 与原 `SelectedMove` 合成最终 move。
   - 只在 `next_task()` 与 `assignment_for_candidate()` 两条选题出口应用，避免单题与 batch 行为分裂。
   - 不修改 `select_next_move_for_concept()` 的基础深度递进语义；相响应作为后置覆写层。

4. 验证门与审计：
   - 每个相策略对应的 `mrt_log.context_json` 必须标出：
     - `selected_by = "phase_action_loop"` 或等价稳定字符串。
     - `phase_strategy`。
     - `main_effect_hypothesis`，说明 7 天成功率或下一次成功率的预期改善。
   - 复用现有 `moves_effects` / `mrt_log` 的 7 天窗口结果记录；不新增表。
   - 这些策略可以参与当前任务选择，但报告或文档中不得声称“已验证优于默认策略”，除非后续数据过门。

5. 对外输出：
   - `TaskAssignment` 已含 `phase`、`move`、`task_type`、`expected_success`，保持结构不破坏。
   - `NextTask.reason` 文案应补一句相响应原因，例如 phantom 对应“用迁移题确认是否真懂”。
   - MCP `get_interleaved_batch` 继续复用现有字段，不新增接口。

## 预计修改面

- `crates/polaris-core/src/engine.rs`
  - 扩展 `BatchStrategy` enum。
- `crates/polaris-core/src/engine/task_selection.rs`
  - 增加相策略选择逻辑。
  - 增加相策略 slot 规则。
  - 在 `next_task()` 与 `assignment_for_candidate()` 出口应用 phase-aware move 覆写。
- `crates/polaris-core/src/pedagogy.rs`
  - 如需要，把 `selected_by` / `phase_strategy` 写入 MRT 预登记 context。
  - 不改变 `moves_effects` 结果记录语义。
- `crates/polaris-core/tests/p03g_interleaved.rs`
  - 增加 phase-aware batch 策略测试。
- `crates/polaris-core/tests/p04c_mrt.rs`
  - 增加相策略的 MRT audit context 测试。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core --test p03g_interleaved
cargo test -p polaris-core --test p04c_mrt
cargo test -p polaris-core phase_action_loop
```

专项验收要求：

- `PhantomChallenge` 能在 batch 中优先挑出 phantom 概念，且该概念任务为 `transfer`。
- `SettlingProbe` 能在无 phantom 时优先挑出 settling 概念，且该概念任务为 `transfer`。
- `RegressionRecovery` 能在无 phantom / settling 时优先挑出 regression 概念，且该概念任务不高于 `explain`。
- HMM 疲劳/无聊 easy-review 策略仍优先保护用户，不被 phase challenge 覆盖。
- `strategy_enabled=false` 的 HMM 事件不改变既有默认 batch 行为。
- `mrt_log.context_json` 为 phase 策略写入稳定的 `selected_by`、`phase_strategy` 与 `main_effect_hypothesis`。
- 85% 目标带宽调整仍运行：batch `expected_success` 均值保持在 `[0.75, 0.90]`，不足时按既有降级语义返回较短 batch。
- 不新增 DDL，不触发 LLM，不修改相图判据和掌握度公式。

## 禁区

- 不改 `determine_phase()` 判据。
- 不改 `Phase::label()` / `Phase::summary()` 产品语义。
- 不改 BKT、MIRT、FSRS、U(c) 公式。
- 不新增表、不要求历史迁移。
- 不把 phase 策略的效果直接写成已验证结论；本票只做预登记与可观测响应。
- 不修改冻结参考仓库。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md` 等预存改动。

## 开工前复述

## AI 交接记录（2026-06-16 开工）

- 当前状态：用户已裁决从 Draft 转入实现；本票为当前唯一 In Progress。
- 本轮范围：仅实现 Phase-aware batch strategy、相策略 move 后置覆写、`NextTask.reason` 相响应文案、MRT 预登记审计字段，以及票内专项测试。
- 禁区确认：不改 `determine_phase()`、`Phase::label()` / `Phase::summary()`、BKT/MIRT/FSRS/U(c) 公式；不新增 DDL；不触发 LLM；不修改冻结参考仓库；不混入 `.gitignore`、`.cursor/`、`docs/visuals/`、atlas 计划等预存改动。
- 验收命令：`cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace`；`cargo test -p polaris-core --test p03g_interleaved`；`cargo test -p polaris-core --test p04c_mrt`；`cargo test -p polaris-core phase_action_loop`。
- 预计修改面：`crates/polaris-core/src/engine.rs`、`crates/polaris-core/src/engine/task_selection.rs`、`crates/polaris-core/src/pedagogy.rs`、`crates/polaris-core/tests/p03g_interleaved.rs`、`crates/polaris-core/tests/p04c_mrt.rs`、`docs/tickets/QUEUE.md`、本票。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/engine.rs crates/polaris-core/src/engine/task_selection.rs crates/polaris-core/src/pedagogy.rs crates/polaris-core/tests/p03g_interleaved.rs crates/polaris-core/tests/p04c_mrt.rs docs/tickets/QUEUE.md
Remove-Item docs/tickets/TICKET_P07D_ACTION_LOOP.md
```

提交后：

```powershell
git revert <P07D-commit-sha>
```

## AI 交付记录（2026-06-16）

- 当前状态：已实现并通过验收；等待用户确认后 commit。
- 变更清单：
  - 扩展 `BatchStrategy`：新增 `PhantomChallenge`、`SettlingProbe`、`RegressionRecovery`。
  - `get_interleaved_batch()` 仅在 HMM 策略为 `Default` 时启用 phase action loop；`Flow` 与 `EasyReviews` 保持既有策略，疲劳/无聊保护优先。
  - `next_task()` 与 batch assignment 共用 phase-aware move 覆写：phantom/settling 派 `transfer`，regression 降到 `recall` / `explain`；Flow/EasyReviews 下禁用覆写。
  - `mrt_log.context_json` 为 phase 策略写入 `selected_by="phase_action_loop"`、`phase_strategy`、`main_effect_hypothesis`；非 action-loop 相仍走 `signature_friction`。
  - phase 策略若 85% 目标带宽不可达，降级返回较短 batch；普通 Default/Flow/EasyReviews 不被裁短。
  - 补充 p03g/p04c 专项测试，覆盖 phantom/settling/regression、HMM fatigue/bored 保护、Flow 保持、短 batch 降级、MRT audit 与 outcome 记录。
- 审批记录：
  - 规格审批：APPROVE。
  - 代码质量审批：APPROVE。
- 票外改动说明：
  - 工作区已有 `.gitignore`、`.cursor/`、`docs/visuals/`、`docs/polaris-core-comic-system-brief.md`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md` 等预存改动；本票未修改这些内容，提交 P07D 时需排除。

### 验收输出

```powershell
> cargo fmt --check
# exit code 0
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.91s
```

```powershell
> cargo test --workspace
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.40s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.56s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core --test p03g_interleaved
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

```powershell
> cargo test -p polaris-core --test p04c_mrt
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

```powershell
> cargo test -p polaris-core phase_action_loop
running 7 tests
test phase_action_loop_bored_easy_reviews_override_phase_challenge ... ok
test phase_action_loop_flow_strategy_keeps_existing_slot_shape ... ok
test phase_action_loop_settling_probe_prefers_settling_transfer_without_phantom ... ok
test phase_action_loop_returns_shorter_batch_when_target_band_is_unreachable ... ok
test phase_action_loop_phantom_challenge_prefers_phantom_transfer ... ok
test phase_action_loop_regression_recovery_prefers_regression_at_most_explain ... ok
test phase_action_loop_easy_reviews_override_phase_challenge ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.03s

running 3 tests
test phase_action_loop_next_task_easy_review_state_suppresses_phase_challenge ... ok
test phase_action_loop_success_updates_preregistered_context ... ok
test phase_action_loop_mrt_audit_context_records_phase_strategies ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.04s
```

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/engine.rs crates/polaris-core/src/engine/task_selection.rs crates/polaris-core/src/pedagogy.rs crates/polaris-core/tests/p03g_interleaved.rs crates/polaris-core/tests/p04c_mrt.rs docs/tickets/QUEUE.md
Remove-Item docs/tickets/TICKET_P07D_ACTION_LOOP.md
```

提交后：

```powershell
git revert <P07D-commit-sha>
```
