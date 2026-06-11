# P02B 图谱感知诊断

服务主命题环节：**定位模糊 → 针对性补缺**。

## 依据

- `SPEC.md`：Schedule-first、Evidence-bound、Tier 0 确定性；不打开 MCP、HTTP、UI。
- `docs/DATA_MODEL.md`：`edges.type` 包含 `prerequisite` 与 `confusion`；`sched.prereq_p` 是前置达标阈值；`bkt.cut_lo` 是失败阈值。
- `docs/MASTER_PLAN.md`：Phase 2 包含图谱感知诊断；失败后应能区分“目标概念本身不会”与“前置概念未达标”；confusion 边用于辨别学习。
- `docs/tickets/QUEUE.md`：P02B 范围为“前置传播（X 失败但前置 Y 未达标 → 诊断 Y）、confusion 边辨析题接口”。

## 本轮范围

1. Rust 内核新增图谱诊断接口：
   - 输入目标概念 `X`。
   - 读取 `X` 最近一次有分数 attempt；若 `score <= meta('bkt.cut_lo')`，视为最近失败。
   - 若最近失败且存在 `prerequisite` 入边 `Y -> X`，并且 `Y` 的 `p_known < meta('sched.prereq_p')`，诊断焦点为最低 `p_known` 的前置概念 `Y`。
   - 诊断结果列出所有未达标前置概念，排序确定性：`p_known ASC, edge.weight DESC, concept_id ASC`。
2. confusion 边辨析题接口：
   - 读取与目标概念相连的 `confusion` 边。
   - 生成确定性的 `discriminate` 任务描述，要求区分目标概念与易混概念的边界、反例和识别线索。
   - confusion 边不直接改变掌握度或调度排序，只作为诊断接口输出。
3. CLI 暴露只读诊断命令：
   - `polaris diagnose --concept <id>`。
   - 输出最近失败状态、前置诊断焦点、未达标前置列表和辨析任务。

## 禁区

- 不实现 P02C：不启动 MCP server，不下发外部导师指令。
- 不改调度器权重，不把诊断结果直接写入 `next_task` 排序。
- 不引入 LLM、不生成 evidence-bound 报告；本票只做 Tier 0 本地图谱诊断。
- 不新增数据库表；复用 P01/P02A 已有 `concepts`、`edges`、`attempts`、`mastery_states`。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并把输出贴到本票尾部：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

当前票额外验收：

```powershell
cargo test -p polaris-core --test p02b_diagnosis
```

期望：

- 目标概念最近失败且前置未达标时，诊断焦点指向前置概念。
- 最近没有失败时，不触发前置传播焦点。
- 多个未达标前置概念按确定性规则排序。
- confusion 边能生成 `discriminate` 辨析任务。
- CLI 能解析 `diagnose --concept <id>`。

## 回滚方式

回滚本票提交即可；若未提交，撤回以下范围：

- `docs/tickets/TICKET_P02B_GRAPH_AWARE_DIAGNOSIS.md`
- `docs/tickets/QUEUE.md`
- `docs/superpowers/plans/2026-06-11-p02b-graph-aware-diagnosis.md`
- `crates/polaris-core/src/diagnosis.rs`
- `crates/polaris-core/src/lib.rs`
- `crates/polaris-core/src/engine.rs`
- `crates/polaris-cli/src/main.rs`
- `crates/polaris-core/tests/p02b_diagnosis.rs`

## 交付记录

### 变更清单

- 新增 `crates/polaris-core/src/diagnosis.rs`：
  - 读取目标概念最近一次有分数 attempt。
  - 使用 `meta('bkt.cut_lo')` 判断最近失败。
  - 使用 `meta('sched.prereq_p')` 识别未达标前置概念。
  - 失败且前置未达标时，输出 `prerequisite_gap` 诊断焦点。
  - 读取相邻 `confusion` 边并生成 `discriminate` 辨析任务。
- `Engine` 新增 `diagnose_concept` 薄封装。
- CLI 新增只读命令 `diagnose --concept <id>`。
- 新增只读 DB 打开路径，`diagnose` 不执行迁移、不创建缺失数据库。
- 新增 `crates/polaris-core/tests/p02b_diagnosis.rs`，覆盖前置传播、非失败不传播、确定性排序和 confusion 辨析任务。
- 新增 CLI 测试，确认 `diagnose` 对缺失 DB 返回错误且不创建文件。
- 更新 `docs/tickets/QUEUE.md`，修正 P02A 已提交状态并标记 P02B 验收后待 commit。

### 验收输出

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.36s
```

说明：普通沙箱重跑 Clippy 时遇到 Windows `target\debug\deps\*.rmeta` 写入拒绝访问；使用已批准的 `cargo clippy` 提升权限后通过。

```powershell
> cargo test --workspace
running 3 tests
test tests::diagnose_does_not_create_missing_database ... ok
test tests::parses_required_command_set ... ok
test tests::behavior_observation_reads_latency_and_hint_count_since_last_next ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 42 tests
test config::tests::parameter_registry_contains_p01_constants ... ok
test config::tests::parameter_registry_contains_all_data_model_section_10_keys ... ok
test config::tests::parameter_registry_contains_p02a_graph_threshold ... ok
test diagnosis::tests::focus_kind_is_stable_for_prerequisite_gap ... ok
test citation::tests::rejects_citation_to_unrelated_evidence ... ok
test citation::tests::rejects_short_quote ... ok
test citation::tests::rejects_long_quote ... ok
test citation::tests::rejects_non_substring_quote ... ok
test citation::tests::accepts_quote_that_is_allowed_evidence_substring ... ok
test fsrs::tests::retrievability_and_interval_match_reference ... ok
test graph::tests::validates_concept_kinds_and_edge_types ... ok
test fsrs::tests::score_maps_to_rating_thresholds_from_data_model ... ok
test fsrs::tests::fsrs_matches_typescript_reference_sequences ... ok
test mastery::tests::calibration_updates_gap_and_skips_brier_in_dead_zone ... ok
test mastery::tests::bkt_updates_correct_wrong_dead_zone_and_explain_guess ... ok
test mastery::tests::fold_all_orders_by_created_at_not_elapsed_days ... ok
test mastery::tests::incremental_fold_matches_full_replay_for_final_score_arrival ... ok
test pack::tests::rejects_misconception_with_missing_concept_reference ... ok
test pack::tests::rejects_unknown_concept_kind ... ok
test grader::tests::accepted_grade_validates_citations_and_updates_attempt ... ok
test grader::tests::heuristic_score_reads_meta_values ... ok
test grader::tests::missing_llm_config_degrades_to_heuristic_and_queues_retry ... ok
test scheduler::tests::high_positive_calibration_gap_can_raise_priority ... ok
test db::tests::migration_creates_p01_tables_and_default_meta ... ok
test scheduler::tests::misconception_active_respects_window_and_later_success ... ok
test grader::tests::invalid_grade_citation_degrades_and_queues_retry ... ok
test scheduler::tests::misconception_and_new_concept_terms_follow_data_model ... ok
test scheduler::tests::ties_sort_by_seed_order_then_id ... ok
test tests::crate_exports_version ... ok
test pack::tests::validates_builtin_rust_pack_shape ... ok
test pack::tests::rejects_unknown_edge_type ... ok
test engine::tests::grade_pending_processes_queued_attempts ... ok
test engine::tests::replay_uses_attempt_created_at_for_fsrs_elapsed_days ... ok
test engine::tests::submit_without_llm_records_provisional_mastery_and_retry_queue ... ok
test engine::tests::final_score_replay_preserves_provisional_history ... ok
test engine::tests::init_pack_installs_concepts_and_next_returns_first_open_concept ... ok
test engine::tests::next_task_uses_engine_misconception_window_semantics ... ok
test scheduler::tests::misconception_active_property_matches_window_and_later_success_semantics ... ok
test engine::tests::integration_seed_flow_prioritizes_high_confidence_low_final_score ... ok
test mastery::tests::fold_all_is_deterministic_for_generated_attempts ... ok
test mastery::tests::replay_after_final_matches_full_replay_for_generated_attempts ... ok
test db::tests::file_database_uses_wal ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

running 3 tests
test structural_mapping_requires_typed_edge_match ... ok
test maps_to_candidate_is_written_only_after_threshold ... ok
test structural_mapping_scores_typed_two_hop_overlap ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

running 4 tests
test non_failed_latest_attempt_does_not_propagate_prerequisite ... ok
test confusion_edge_generates_discrimination_task ... ok
test failed_target_with_unmet_prerequisite_focuses_prerequisite ... ok
test unmet_prerequisites_are_sorted_deterministically ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.79s
     Running unittests src\main.rs (target\debug\deps\polaris-6ba60a5ff05aa4a7.exe)
     Running unittests src\lib.rs (target\debug\deps\polaris_core-3b87d3222cf159b1.exe)
     Running tests\p02a_graph.rs (target\debug\deps\p02a_graph-23af5d85695a72af.exe)
     Running tests\p02b_diagnosis.rs (target\debug\deps\p02b_diagnosis-6e3b48326795ca3a.exe)
   Doc-tests polaris_core
```

```powershell
> cargo test -p polaris-core --test p02b_diagnosis
running 4 tests
test confusion_edge_generates_discrimination_task ... ok
test non_failed_latest_attempt_does_not_propagate_prerequisite ... ok
test unmet_prerequisites_are_sorted_deterministically ... ok
test failed_target_with_unmet_prerequisite_focuses_prerequisite ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.26s
     Running tests\p02b_diagnosis.rs (target\debug\deps\p02b_diagnosis-ac9f39e6fa61bf86.exe)
```

### 审查记录

- 子 agent `Beauvoir` 已做只读 P02B 审查。
- 审查发现阻塞问题：CLI `diagnose` 走普通 `open_database` 会迁移/建库，不符合只读约束。
- 已修复：
  - 新增 `open_database_read_only`，使用 SQLite read-only flags，不执行迁移。
  - `diagnose` 分支在打开可写 DB 前单独处理。
  - 新增红灯测试 `diagnose_does_not_create_missing_database`，先复现创建缺失 DB，再修到通过。

### 阻塞与裁决记录

无。

### 技术选择说明

- 本票只暴露诊断接口，不改 `next_task` 排序；调度仍由本地 scheduler 控制，避免未经验证的策略漂移。
- CLI `diagnose` 必须是只读路径；因此不复用会迁移和写默认 meta 的 `open_database`。
- 最近失败使用 `score <= meta('bkt.cut_lo')`，与 `DATA_MODEL.md` 的 BKT 错误阈值一致。
- 前置未达标使用 `meta('sched.prereq_p')`，与现有 prerequisite gate 使用同一阈值。
- 无 `mastery_states` 的前置概念按 `p_known=0.0` 处理，保持与现有 `Engine::prerequisites_met` 语义一致。
- confusion 辨析任务为确定性文本接口，不调用 LLM，不写入掌握度。
