# P03E 知识相图判定 (Knowledge Phase Diagram)

状态：已完成

服务主命题环节：验证真懂 → 定位模糊 → 针对性补缺

## 背景

DATA_MODEL §9 定义了 7 种知识相及其操作性判据。MASTER_PLAN F2 将相图视为原创理论层核心——掌握度向量落入的离散"相"决定教学策略选择。当前引擎已有幻影标记（P01 粗版），但完整的相判定逻辑尚未作为独立纯函数实现，也未接入调度器和状态资源。

本票将相判定实现为 Tier 0 纯函数（新 `phase.rs` 模块），输入掌握度向量各分量，输出离散 Phase enum。相判定是后续 F1 签名选法、F3 摩擦曲线、镜像报告和 UI 状态镜子的前置依赖。

科学锚点：Bjork 双强度理论（存储×提取，2 维先驱）→ 本系统扩至 ≥5 维可观测的离散相分类（见 `docs/COGNITIVE_SCIENCE_ANCHORS.md`）。

## 范围

1. 新增 `phase.rs` 模块，定义 `Phase` enum 与纯函数 `determine_phase`：
   - 输入结构体 `PhaseInput { p_known, R, theta_prediction, calib_gap, attempt_count, lapses, max_depth, has_transfer_success, transfer_fail_count, novel_context_success, novel_context_fail, median_latency_ratio }`。
   - 输出 `Phase` enum，7 个变体：

   | 相 | 操作性判据（DATA_MODEL §9 对齐） |
   |---|---|
   | Phantom（幻影） | n≥2 ∧ calib_gap≥0.25 ∧ p<0.6 |
   | Fluctuation（脆弱/波动） | p≥0.6 ∧ max_depth ≤ explain |
   | Settling（沉淀） | p≥0.6 ∧ max_depth ≥ apply ∧ ¬transfer_success ∧ transfer_fail < 2 |
   | Solidification（凝固） | p≥0.6 ∧ max_depth ≥ apply ∧ transfer_fail ≥ 2 ∧ ¬transfer_success |
   | Transfer（迁移/活跃） | p≥0.7 ∧ ≥1 次 transfer 成功 |
   | Generation（自动化/生成） | Transfer 条件 ∧ latency 中位 < 个人全局 25 分位 ∧ 样本≥3 |
   | Regression（回退） | 曾达 Transfer/Generation ∧ 近 lapses ≥ 2 ∧ p 跌破 0.5 |

   - 判定优先级：Regression > Phantom > Generation > Transfer > Solidification > Settling > Fluctuation。
   - 数据不足（attempt_count < 2）→ 返回 `Phase::Undetermined`（第 8 变体，不参与教学策略）。

2. 集成到掌握度更新路径：
   - `Engine::submit` 的 fold 完成后调用 `determine_phase` 并将结果写入 `mastery_states` 新字段 `phase TEXT`。
   - 相变事件写入 `behavior_events`：`type='phase_transition'`，payload 含 `{from, to, concept_id, attempt_id}`。

3. 调度器感知相：
   - `U(c)` 新增相因子项 `w_phase`，权重从 meta 读取（B 类参数，默认 0）：
     - Phantom：`+0.15`（优先修复幻影）
     - Regression：`+0.20`（优先止回退）
     - Undetermined：`+0.05`（派发探针任务取证）
   - P03E 阶段 `w_phase` 默认 0，不改变现有调度行为。只有过验证门后才上调。

4. 合意困难集成（Bjork desirable difficulties）：
   - Settling→Solidification 相变期：调度器自动选取更高难度 task_type（如从 apply 提升到 transfer），目标成功率从 0.85 降至 0.75。
   - 此行为受验证门控制：未过门时不启用。

5. 状态资源暴露：
   - `Engine::concept_phase(concept_id) -> Phase`。
   - MCP `status` resource 的 `concept_mastery` 对象新增 `phase` 字段。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03e_phase
```

额外人工检查：

```powershell
git diff --check
```

属性测试要求：
- 相单调性：持续正确作答序列下，相变方向只能是 Fluctuation→Settling→Solidification→Transfer→Generation，不得逆行（Regression 除外，需显式 lapse 触发）。
- 回退检测：连续 lapse 后必须触发 Regression。
- 决定性：相同输入始终返回相同 Phase。
- Undetermined 不影响调度排序（`w_phase` 加成最小）。

## 禁区

- 不实现镜像报告、UI 相图可视化或 MRT 摩擦曲线。
- 不让 Phase 在未过验证门时改变 `next_task` 排序（`w_phase` 默认 0）。
- 不引入新的顶层概念——Phase 使用 MASTER_PLAN F2 已有词汇。
- 不修改冻结参考仓库。

## 本轮范围（2026-06-12）

- 当前状态：P03D 已在 QUEUE 与票文件中标为已完成；P03E 已按 ENHANCEMENT_ROADMAP 优先级认领为 In Progress。
- 已有非本票改动：工作区存在 AGENTS/README/AI_RUNBOOK/MASTER_PLAN/QUEUE、增强路线图、P03F+ 票据、P05A0 票据和漫画相关文档改动；本票不得回退这些改动。
- 本票范围：新增 `phase.rs` 纯函数与 Phase enum；为 `mastery_states` 增加 `phase` 字段；在掌握度 fold 后持久化相并记录 `phase_transition` 事件；调度器新增默认 0 的 `sched.w_phase` 相因子且不改变排序；暴露 `Engine::concept_phase` 与 status/MCP 中的 phase 字段；新增 P03E 验收测试。
- 禁区：不做镜像报告、UI 相图、MRT、摩擦曲线；不修改冻结参考仓库；不让未过验证门的 Phase 改变 `next_task` 排序。
- 验收命令：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo test -p polaris-core --test p03e_phase`、`git diff --check`。

## 交付记录（2026-06-12）

### P03D 核对

- `docs/tickets/QUEUE.md` 中 P03D 已勾选并标注“已实现并通过验收”。
- `docs/tickets/TICKET_P03D_MENTAL_STATE_HMM.md` 状态为“已完成”，票尾已有变更清单、验收输出和回滚方式。
- 两者一致，因此按 ENHANCEMENT_ROADMAP 优先级认领 P03E。

### 变更清单

- 新增 `crates/polaris-core/src/phase.rs`：
  - `Phase` enum：`undetermined | phantom | fluctuation | settling | solidification | transfer | generation | regression`。
  - `Depth` enum 与 `PhaseInput`。
  - `determine_phase` 纯函数，按票据优先级执行：Regression > Phantom > Generation > Transfer > Solidification > Settling > Fluctuation；`attempt_count < 2` 返回 `Undetermined`。
- 数据与折叠：
  - `mastery_states` 增加 `phase TEXT DEFAULT 'undetermined'`，并为既有库补 `ALTER TABLE` 迁移。
  - `Engine` 在概念 replay 后派生 Phase 并持久化。
  - 相变化时写 `behavior_events.type='phase_transition'`，payload 含 `from/to/concept_id/attempt_id`。
  - 新增 `Engine::concept_phase(concept_id) -> Phase`。
- 调度：
  - `SchedulerParams` 增加 `sched.w_phase`，默认 `0.0`、B 类、MRT 路径。
  - `ScheduleCandidate` 增加 `phase`；`w_phase=0` 时不改变现有排序。
- 状态暴露：
  - `status_snapshot` 和 MCP `polaris://status` 读取持久化 `phase`。
  - CLI `status` 从持久化 `phase` 输出，不再使用 P01 粗版“幻影/正常”即时判定。
- 测试：
  - 新增 `crates/polaris-core/tests/p03e_phase.rs` 覆盖判相优先级、决定性、单调进展、phase 持久化、相变事件、status 暴露和默认不改调度。
  - 更新 P02C status 测试期望为 `undetermined`。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.95s
exit 0
```

说明：普通权限下首次 clippy 被 Windows 拒绝写入 `target/debug/deps/*.rmeta`，按权限规则使用提升权限重跑同一命令后通过；无代码 lint 错误。

```text
cargo test --workspace
polaris-cli: 9 passed
polaris-core unit: 45 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 5 passed
p03b_consolidation: 3 passed
p03c_geometry: 7 passed
p03d_mental_state: 10 passed
p03e_phase: 6 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-core --test p03e_phase
running 6 tests
test determine_phase_uses_frozen_priority_order ... ok
test scheduler_phase_factor_defaults_to_zero_and_does_not_change_order ... ok
test correct_depth_progression_does_not_move_backward ... ok
test status_snapshot_exposes_stored_phase ... ok
test engine_persists_phase_and_emits_transition_event ... ok
test determine_phase_is_deterministic ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 技术选择

- `phase` 存在 `mastery_states` 中，仍保持 `attempts + behavior_events + pack seed` 为事实源；phase 是 replay 后确定性派生的物化视图。
- 当前 schema 没有显式 `context_novel` 列；P03E v1 不扩 attempts 表，只从可选 `grader_json.context_novel=true` 派生换景成功/失败，缺失时为 0。
- Generation 的延迟判据用概念内 latency 中位 / 全局 25 分位；概念 latency 样本 <3 时不判 Generation。
- `sched.w_phase` 默认 0，因此 P03E 相信息只记录和暴露，不改变 `next_task` 排序；后续过验证门后可由单独票上调。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-cli/src/main.rs crates/polaris-core/src/config.rs crates/polaris-core/src/db.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs crates/polaris-core/src/scheduler.rs crates/polaris-core/src/status.rs crates/polaris-core/tests/p02c_teaching.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/src/phase.rs crates/polaris-core/tests/p03e_phase.rs docs/tickets/TICKET_P03E_KNOWLEDGE_PHASE_DIAGRAM.md
```

提交后：

```powershell
git revert <P03E-commit-sha>
```

## 子 agent 复查后补充交付记录（2026-06-12）

### 复查来源

- 已按用户建议使用两个子 agent 做只读检查：
  - 规格/票据一致性检查。
  - 代码正确性与测试覆盖检查。
- 采纳并修复的反馈：
  - Phantom 判据阈值从 `meta` 参数 `calib.phantom_gap`、`calib.phantom_p`、`calib.phantom_n` 读取，不再在判定函数中硬编码。
  - `Settling` 与 `Solidification` 明确要求 `!has_transfer_success`。
  - `Settling` 结合 DATA_MODEL §9：要求原情境成功 `>=2` 且新情境失败 `>=2`。
  - `Regression` 改为“曾达到 Transfer/Generation 后，近期连续 lapse >=2，且 p<0.5”，不再用累计 FSRS lapses 或一次 transfer 成功近似。
  - `Generation` 的 latency 样本与中位数改为 transfer/free_produce 相关 task_type，不再混入同概念 recall 速度；样本数 `>=3` 也按相关 task_type 统计。
  - CLI status 对非法 raw phase 走 `Phase::parse` 降级到 `undetermined`。
  - MCP status 测试补断言 `phase` 字段。
  - 补旧 schema `mastery_states` 无 `phase` 时的迁移测试。

### 补充变更清单

- `crates/polaris-core/src/phase.rs`
  - 新增 `PhaseParams` 与 `PhaseParams::from_conn`。
  - `determine_phase(input, params)` 改为显式接收相判定参数。
  - `PhaseInput` 增加 `recent_lapses`、`ever_reached_transfer_or_generation`、`relevant_task_attempt_count`、`original_context_success`。
- `crates/polaris-core/src/engine.rs`
  - replay 时从 attempt 前缀推导是否曾达 Transfer/Generation 与近期 lapse。
  - 从 `grader_json.context_novel=true` 统计新情境成功/失败；缺失该标志的成功计入原情境成功。
  - Generation latency 只统计 `depth='transfer'` 或 `task_type IN ('transfer','free_produce')` 的样本。
- `crates/polaris-cli/src/main.rs`
  - status 输出前解析 phase，非法值降级为 `undetermined`。
- `crates/polaris-cli/src/mcp.rs`
  - `polaris://status` 单测断言 `phase` 字段。
- `crates/polaris-core/tests/p03e_phase.rs`
  - P03E 专项测试从 6 个扩展到 15 个，覆盖参数化 Phantom、transfer 排除、novel context、Regression 历史条件、Generation latency task_type 桶、grade_pending transfer depth replay、旧 schema 迁移。

### 补充验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.04s
exit 0
```

说明：普通权限下 clippy 仍被 Windows 拒绝写入 `target/debug/deps/*.rmeta`，按权限规则使用提权运行同一命令后通过；失败原因为文件系统写入权限，不是 lint 错误。

```text
cargo test --workspace
polaris-cli: 9 passed
polaris-core unit: 45 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 5 passed
p03b_consolidation: 3 passed
p03c_geometry: 7 passed
p03d_mental_state: 10 passed
p03e_phase: 15 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-core --test p03e_phase
running 15 tests
test determine_phase_uses_configured_phantom_thresholds ... ok
test determine_phase_uses_frozen_priority_order ... ok
test regression_requires_recent_lapses_after_reaching_transfer_or_generation ... ok
test settling_requires_novel_context_evidence_and_no_transfer_success ... ok
test solidification_requires_no_transfer_success ... ok
test scheduler_phase_factor_defaults_to_zero_and_does_not_change_order ... ok
test correct_depth_progression_does_not_move_backward ... ok
test legacy_mastery_states_migration_adds_phase_without_losing_rows ... ok
test determine_phase_is_deterministic ... ok
test generation_latency_uses_relevant_task_type_bucket ... ok
test novel_context_failures_trigger_settling_without_transfer_success ... ok
test engine_persists_phase_and_emits_transition_event ... ok
test status_snapshot_exposes_stored_phase ... ok
test engine_uses_meta_phantom_thresholds ... ok
test grade_pending_transfer_depth_replays_phase ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 补充回滚方式

未提交前：

```powershell
git restore crates/polaris-cli/src/main.rs crates/polaris-cli/src/mcp.rs crates/polaris-core/src/config.rs crates/polaris-core/src/db.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs crates/polaris-core/src/scheduler.rs crates/polaris-core/src/status.rs crates/polaris-core/tests/p02c_teaching.rs docs/tickets/QUEUE.md docs/tickets/TICKET_P03E_KNOWLEDGE_PHASE_DIAGRAM.md
git clean -f crates/polaris-core/src/phase.rs crates/polaris-core/tests/p03e_phase.rs
```

提交后：

```powershell
git revert <P03E-commit-sha>
```
