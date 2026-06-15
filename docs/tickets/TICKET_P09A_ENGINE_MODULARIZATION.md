# P09A engine.rs 模块化拆分

状态：已完成

服务主命题环节：全环节（可演进性保障）

## 背景

`crates/polaris-core/src/engine.rs` 已承载 2300+ 行代码：对外 facade、选题/交错调度、提交与评分回放、心智状态记录、相图派生、以及一组单元测试混在同一文件。Phase 7+ 产品形态、多 Pack 承载与信任面板都会继续触达这些路径；如果继续在 god-file 上叠功能，回归面会越来越难审。

本票是**纯模块化重构**：拆文件、搬测试、保留行为。它不改变公式、数据库、参数、调度策略或 public API。

## 范围

### 1. 目标模块

新增目录 `crates/polaris-core/src/engine/`，保留 `crates/polaris-core/src/engine.rs` 作为薄 facade 与类型集中出口。

```text
engine.rs
  ├── engine/task_selection.rs
  ├── engine/submit_pipeline.rs
  └── engine/mental_state.rs
```

拆分边界：

- `engine/task_selection.rs`
  - 负责 `next_task`、`record_next_task_event`、`get_interleaved_batch`。
  - 搬入 ranked candidates、batch strategy、review diversity、expected success、prerequisite/misconception 查询等选题辅助函数。
  - 不改变 `NextTask`、`TaskAssignment`、`BatchStrategy` 的语义。
- `engine/submit_pipeline.rs`
  - 负责 `submit_provisional`、`submit`、`apply_final_score`、`grade_pending`、`grade_pending_with_static_response`。
  - 搬入 provisional attempt 写入、final replay、grade queue、pending count/retry、FSRS next due 等提交/回放辅助函数。
  - `replay_concept_after` 及其 phase 派生 helper 跟随本模块：`phase_input`、`phase_history_summary`、`context_counts`、`relevant_task_latency_ratio`、`relevant_task_latencies`、`stored_phase`、`record_phase_transition`。`phase.rs` 仍只保留纯函数 `determine_phase` 与相图类型/参数。
  - 不改变 optimistic update、evidence-bound grading、retry queue 或 theta_version 语义。
- `engine/mental_state.rs`
  - 负责 submit/final replay 过程中需要的心智状态观测、snapshot、hazard/state gate 读取、state posterior 查询。
  - 搬入 `mental_state_observation`、`final_mental_state_observation`、`record_mental_state_snapshot` 及其特征工程辅助函数。
  - 不改变 HMM、hazard、state gate 或镜像报告消费语义。

`init_pack` 暂留 `engine.rs` facade。它不属于选题、提交或心智状态三组；若 P08A 多 Pack 切换使它继续膨胀，再单独开 `engine/pack_init.rs` 小票。

### 2. public 导出表

`engine.rs` 继续公开下列类型：

- `Engine`
- `NextTask`
- `TaskAssignment`
- `SubmitInput`
- `SubmitReceipt`
- `GradePendingSummary`
- `StoredMasteryState`

新增子模块内部需要共享的类型：

- `RankedTaskCandidate`：`pub(crate)`，仅 task selection 内部或 facade 辅助使用。
- `BatchStrategy`：`pub(crate)`，仅 task selection 内部使用。
- `MentalStateRecord<'a>`：`pub(crate)`，仅 mental state/submit pipeline 之间共享。

子模块函数原则：

- 对外现有 `Engine` 方法保持 `pub fn` 原签名。
- 子模块内 helper 默认 `pub(super)` 或私有；只有跨子模块调用时才升为 `pub(crate)`。
- 不新增对 crate 外公开的 engine 子模块 API。

### 3. `engine.rs` 薄 facade 保留 API

以下 `Engine` public 方法签名必须逐一保留：

- 基础：`new`、`conn`
- 图谱/诊断：`structural_mapping_score`、`upsert_maps_to_candidate`、`diagnose_concept`
- 状态/目标：`status_snapshot`、`create_goal`、`goal_snapshot`、`update_goal_dimension_value`、`goal_progress`、`refresh_goal_milestones`
- 教学/潜因子：`teaching_instruction`、`latent_prediction`、`fused_p_known`
- 夜间/报告/归纳/育种：`run_nightly_consolidation`、`run_param_tuning`、`run_mental_dynamics_fit`、`run_mirror_report`、`run_mirror_report_with_narrative`、`run_mirror_report_with_static_narrative`、`latest_mirror_report`、`record_report_feedback`、`run_gu_induction`、`active_gu_rules_for_concept`、`preregister_bred_move`、`record_bred_move_outcome`、`evaluate_bred_moves`、`admitted_bred_moves`
- 几何：`refresh_missing_embeddings`、`refresh_missing_embeddings_with_provider`、`geometry_candidates`、`upsert_geometry_maps_to_candidates`
- pack/选题：`init_pack`、`next_task`、`record_next_task_event`、`get_interleaved_batch`
- 提交/评分：`submit_provisional`、`submit`、`apply_final_score`、`grade_pending`、`grade_pending_with_static_response`
- 查询：`mastery_state`、`concept_phase`

### 4. 测试搬家映射

把 `engine.rs` 内联测试搬到 integration tests，避免生产文件继续膨胀：

- `tests/engine_submit_pipeline.rs`
  - `submit_without_llm_records_provisional_mastery_and_retry_queue`
  - `submit_provisional_records_mastery_and_queues_retry`
  - `final_score_replay_preserves_provisional_history`
  - `replay_uses_attempt_created_at_for_fsrs_elapsed_days`
  - `grade_pending_processes_queued_attempts`
- `tests/engine_task_selection.rs`
  - `init_pack_installs_concepts_and_next_returns_first_open_concept`
  - `integration_seed_flow_prioritizes_high_confidence_low_final_score`
  - `next_task_uses_engine_misconception_window_semantics`

共享测试 helper：

- 新增 `tests/common/mod.rs`，放置 `workspace_pack_path` 等跨 integration test 复用的 helper。
- 两个 engine integration test 文件通过 `mod common;` 引入，避免重复定义。

测试搬迁必须只调整 imports/helper 位置，不改变断言语义。

### 5. 依赖方向图

```text
engine.rs (Engine facade/types)
  ├── task_selection
  │     └── reads submit/phase/mirt/scheduler state through &Connection
  ├── submit_pipeline
  │     ├── calls mental_state for observations/snapshots
  │     └── calls mastery/mirt/phase/grader/pedagogy
  └── mental_state
        └── reads attempts/behavior_events/hazard/state_gate through &Connection
```

依赖规则：

- `task_selection` 不依赖 `submit_pipeline`。
- `mental_state` 不依赖 `task_selection`。
- `submit_pipeline` 可以依赖 `mental_state`。
- 子模块不得互相重新定义业务类型；共享类型从 `engine.rs` 引入。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

补充哨兵测试：

```powershell
cargo test -p polaris-core --test p03a_mirt
cargo test -p polaris-core --test p03g_interleaved
cargo test -p polaris-core --test p04e_simulation
cargo test -p polaris-core --test p06e_performance_budget
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- `Engine` public 方法签名保持不变。
- `cargo test --workspace` 的通过数只因测试搬家发生位置变化，不因断言删除而减少覆盖。
- `engine.rs` 不再包含 `#[cfg(test)] mod tests`。
- `engine.rs` facade 行数 ≤ 600。
- `engine/task_selection.rs`、`engine/submit_pipeline.rs`、`engine/mental_state.rs` 各自行数 ≤ 1000。
- 不修改 `DATA_MODEL.md`、数据库 DDL、参数登记或公式。
- 不新增 P07/P08/P10 的产品行为。

## 禁区

- 不改变选题排序、交错 batch、MRT 随机化、相图判定、HMM/hazard、BKT/FSRS/MIRT、G_u、breeding 或 mirror report 行为。
- 不重命名 `Engine` 对外类型和 public 方法。
- 不做性能优化、参数整理、CLI 扩展或产品文案。
- 不修改冻结参考仓库。
- 不把产品经理的 `PRODUCT_ROADMAP.md` / `ENHANCEMENT_ROADMAP.md` 改动混入本票提交。

## 本轮范围（2026-06-15）

- 当前状态：P06F 已提交（`417e87c`），P06G 已提交（`5718aa2`）。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、产品经理的 `docs/PRODUCT_ROADMAP.md`、`docs/ENHANCEMENT_ROADMAP.md` 与 `QUEUE.md` 产品形态轴线补充。本票不得回退或混入这些改动。
- 实现期间不修改 `docs/` 下任何文件，除本票、`QUEUE.md` 状态/正式票行，以及可选的 `ENHANCEMENT_ROADMAP.md` P09A 完成标记。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/engine.rs docs/tickets/QUEUE.md docs/tickets/TICKET_P09A_ENGINE_MODULARIZATION.md
Remove-Item -Recurse crates/polaris-core/src/engine
Remove-Item crates/polaris-core/tests/engine_submit_pipeline.rs, crates/polaris-core/tests/engine_task_selection.rs
Remove-Item -Recurse crates/polaris-core/tests/common
```

提交后：

```powershell
git revert <P09A-commit-sha>
```

## 交付记录（2026-06-15）

### 变更清单

- `crates/polaris-core/src/engine.rs`：
  - 保留 `Engine` facade、公共类型、基础委托方法与 `init_pack`。
  - 增加 `mod task_selection`、`mod submit_pipeline`、`mod mental_state`。
  - 移除内联 `#[cfg(test)] mod tests`。
- `crates/polaris-core/src/engine/task_selection.rs`：
  - 搬入 `next_task`、`record_next_task_event`、`get_interleaved_batch` 与选题/batch helper。
  - 搬入 prerequisite 与 misconception 查询 helper。
- `crates/polaris-core/src/engine/submit_pipeline.rs`：
  - 搬入 submit/provisional/final score/grade queue/replay/phase 派生 helper。
  - `replay_concept_after` 及 phase 派生 helper 跟随 submit pipeline。
- `crates/polaris-core/src/engine/mental_state.rs`：
  - 搬入心智状态观测、snapshot、hazard/state gate 读取与 payload 解析 helper。
  - 将跨模块使用的 `MentalStateRecord` 与少数 helper 暴露为 `pub(crate)`。
- 测试：
  - 新增 `crates/polaris-core/tests/common/mod.rs` 共享 `workspace_pack_path`。
  - 新增 `engine_submit_pipeline.rs`，承接 5 个 submit/replay/grade queue 测试。
  - 新增 `engine_task_selection.rs`，承接 3 个选题测试。
  - `replay_uses_attempt_created_at_for_fsrs_elapsed_days` 改走 public submit/apply_final_score 路径，不再调用私有 helper。
- 文档：
  - `QUEUE.md` 标记 P09A In Progress/正式票。
  - 本票补齐产品经理审查 6 条建议与交付记录。

### 行数指标

```text
engine.rs: 376
engine/task_selection.rs: 586
engine/submit_pipeline.rs: 624
engine/mental_state.rs: 451
```

满足：facade ≤ 600，三个子模块各自 ≤ 1000。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.01s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 29 passed
polaris-core unit: 63 passed
engine_submit_pipeline: 5 passed
engine_task_selection: 3 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 7 passed
p03b_consolidation: 3 passed
p03c_geometry: 8 passed
p03d_mental_state: 10 passed
p03e_phase: 17 passed
p03f_moves: 7 passed
p03g_interleaved: 9 passed
p03h_gu_induction: 8 passed
p03i_mirror_report: 14 passed
p03j_param_tuning: 8 passed
p03k_mental_fit: 7 passed
p03m_latent_dims: 3 passed
p04a_desktop_status: 1 passed
p04c_mrt: 8 passed
p04d_goals: 6 passed
p04e_simulation: 3 passed
p05a1_algorithms: 5 passed
p05a_english: 5 passed
p05b_breeding: 5 passed
p06c_property_expansion: 3 passed
p06d_mirror_report_narrative: 6 passed
p06e_performance_budget: 4 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-core --test p03a_mirt
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p03g_interleaved
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p04e_simulation
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p06e_performance_budget
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- 采用 `impl Engine` 分散到子模块的方式，保持 `Engine` public API 与调用方路径不变。
- `init_pack` 留在 facade，避免在 P09A 中提前引入 P08A 的 pack lifecycle 抽象。
- phase 派生 helper 跟随 replay 留在 submit pipeline，避免把带数据库读取的逻辑放入纯函数 `phase.rs`。
- integration tests 覆盖原 engine 内联测试；core unit 测试数量下降 8 个，对应新增 integration tests 5+3 个，断言未删除。

### 待审事项

- 按产品经理要求：提交前需要产品/架构审查本轮未提交 diff。
