# P02A 类型化超图

服务主命题环节：**定位模糊**。

## 依据

- `SPEC.md`：单票制、Tier 0 确定性、参数三类制、验收基线。
- `docs/DATA_MODEL.md`：`concepts.kind='schema'` 激活图式；`edges.type` 支持 `prerequisite|confusion|component_of|instantiates|maps_to`；`struct(a,b)` 使用 2-hop 类型化邻域，`score = 匹配边数 / max(|E_a|, |E_b|)`，达到 0.4 进入 `maps_to` 候选。
- `docs/MASTER_PLAN.md`：Phase 2 激活符号层超图；节点分层为 L0 域概念和 L1+ schema；`maps_to` 边必须带对齐说明。

## 本轮范围

1. Pack 校验激活类型化图谱：
   - `concepts.kind` 仅允许 `concept`、`schema`。
   - `edges.type` 仅允许 `prerequisite`、`confusion`、`component_of`、`instantiates`、`maps_to`。
   - `instantiates`、`component_of`、`maps_to` 与既有引用完整性一起校验。
2. Rust 内核提供类型化图谱结构映射能力：
   - 读取指定节点的 2-hop 类型化邻域。
   - 计算确定性的 `struct(a,b)` 分数。
   - 嵌入缺失时只做保守的确定性配对：根节点强制配对，其他节点仅在 id 相同或可解析嵌入相似时参与配对。
3. 达到阈值时写入 `maps_to` 候选边：
   - 阈值使用 `meta('graph.struct_threshold')`，默认 `0.40`，A 类、手动调整；这是对 `DATA_MODEL.md` 已写定常数的登记，不改变公式。
   - 写入的边 `provenance='engine'`，`alignment_json` 包含 `score`、`matched_edges`、`total_edges`、`requires_llm_verification=true`。

## 禁区

- 不实现 P02B：不做前置传播诊断，不做 confusion 辨析题接口。
- 不实现 P02C：不启动 MCP server，不开放外部导师指令。
- 不实现 P03C 几何层：不调用 embedding API，不引入 HNSW，不做候选发现流水线。
- 不改 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不把 Rust pack 改成手写图式本体；本票只激活 schema/edge 语义与计算能力。

## 验收

必须真实运行并把输出贴到本票尾部：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

当前票额外验收：

```powershell
cargo test -p polaris-core --test p02a_graph
cargo run -p polaris-cli -- pack validate packs/rust
```

期望：

- `pack validate` 仍通过既有 Rust pack。
- 新增测试覆盖非法 `concepts.kind`、非法 `edges.type`、2-hop 结构映射得分、低于阈值不写入 `maps_to`、达到阈值写入带 `alignment_json` 的候选边。

## 回滚方式

回滚本票提交即可；若未提交，撤回以下范围：

- `docs/tickets/TICKET_P02A_TYPED_HYPERGRAPH.md`
- `docs/tickets/QUEUE.md`
- `crates/polaris-core/src/graph.rs`
- `crates/polaris-core/src/lib.rs`
- `crates/polaris-core/src/config.rs`
- `crates/polaris-core/src/error.rs`
- `crates/polaris-core/src/pack.rs`
- `crates/polaris-core/src/engine.rs`
- 相关测试改动

## 交付记录

### 变更清单

- 新增 `crates/polaris-core/src/graph.rs`：
  - 校验 `concept|schema` 与 5 类合法边类型。
  - 读取有向 2-hop 类型化邻域，按 `(src,dst,type)` 去重。
  - 计算确定性 `struct(a,b)` 分数。
  - `maps_to` 候选边写入 `alignment_json`，并标记 `requires_llm_verification=true`。
- `Engine` 暴露 `structural_mapping_score` 与 `upsert_maps_to_candidate`。
- Pack 校验拒绝非法 `concepts.kind` 与非法 `edges.type`；`init_pack` 透传 `alignment_json`。
- 参数登记处新增 `graph.struct_threshold = 0.40`，A 类、手动调整。
- 新增 `crates/polaris-core/tests/p02a_graph.rs`，覆盖结构映射与阈值写边行为。

### 验收输出

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.40s
```

说明：普通沙箱重跑 Clippy 时遇到 Windows `target\debug\deps\*.rmeta` 写入拒绝访问；使用已批准的 `cargo clippy` 提升权限后通过。最终一次普通沙箱失败点为 `libp02a_graph-*.rmeta` 写入拒绝访问。

```powershell
> cargo test --workspace
running 2 tests
test tests::parses_required_command_set ... ok
test tests::behavior_observation_reads_latency_and_hint_count_since_last_next ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 41 tests
test config::tests::parameter_registry_contains_all_data_model_section_10_keys ... ok
test config::tests::parameter_registry_contains_p02a_graph_threshold ... ok
test citation::tests::rejects_citation_to_unrelated_evidence ... ok
test citation::tests::rejects_long_quote ... ok
test citation::tests::rejects_non_substring_quote ... ok
test citation::tests::accepts_quote_that_is_allowed_evidence_substring ... ok
test citation::tests::rejects_short_quote ... ok
test config::tests::parameter_registry_contains_p01_constants ... ok
test fsrs::tests::retrievability_and_interval_match_reference ... ok
test graph::tests::validates_concept_kinds_and_edge_types ... ok
test fsrs::tests::score_maps_to_rating_thresholds_from_data_model ... ok
test mastery::tests::bkt_updates_correct_wrong_dead_zone_and_explain_guess ... ok
test fsrs::tests::fsrs_matches_typescript_reference_sequences ... ok
test mastery::tests::calibration_updates_gap_and_skips_brier_in_dead_zone ... ok
test mastery::tests::fold_all_orders_by_created_at_not_elapsed_days ... ok
test mastery::tests::incremental_fold_matches_full_replay_for_final_score_arrival ... ok
test pack::tests::rejects_misconception_with_missing_concept_reference ... ok
test db::tests::migration_creates_p01_tables_and_default_meta ... ok
test grader::tests::accepted_grade_validates_citations_and_updates_attempt ... ok
test grader::tests::missing_llm_config_degrades_to_heuristic_and_queues_retry ... ok
test pack::tests::rejects_unknown_concept_kind ... ok
test grader::tests::invalid_grade_citation_degrades_and_queues_retry ... ok
test grader::tests::heuristic_score_reads_meta_values ... ok
test scheduler::tests::misconception_active_respects_window_and_later_success ... ok
test scheduler::tests::misconception_and_new_concept_terms_follow_data_model ... ok
test pack::tests::rejects_unknown_edge_type ... ok
test scheduler::tests::high_positive_calibration_gap_can_raise_priority ... ok
test pack::tests::validates_builtin_rust_pack_shape ... ok
test scheduler::tests::ties_sort_by_seed_order_then_id ... ok
test tests::crate_exports_version ... ok
test engine::tests::final_score_replay_preserves_provisional_history ... ok
test engine::tests::replay_uses_attempt_created_at_for_fsrs_elapsed_days ... ok
test engine::tests::submit_without_llm_records_provisional_mastery_and_retry_queue ... ok
test engine::tests::grade_pending_processes_queued_attempts ... ok
test engine::tests::init_pack_installs_concepts_and_next_returns_first_open_concept ... ok
test engine::tests::next_task_uses_engine_misconception_window_semantics ... ok
test engine::tests::integration_seed_flow_prioritizes_high_confidence_low_final_score ... ok
test scheduler::tests::misconception_active_property_matches_window_and_later_success_semantics ... ok
test mastery::tests::fold_all_is_deterministic_for_generated_attempts ... ok
test mastery::tests::replay_after_final_matches_full_replay_for_generated_attempts ... ok
test db::tests::file_database_uses_wal ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

running 3 tests
test structural_mapping_requires_typed_edge_match ... ok
test structural_mapping_scores_typed_two_hop_overlap ... ok
test maps_to_candidate_is_written_only_after_threshold ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.43s
     Running unittests src\main.rs (target\debug\deps\polaris-6ba60a5ff05aa4a7.exe)
     Running unittests src\lib.rs (target\debug\deps\polaris_core-3b87d3222cf159b1.exe)
     Running tests\p02a_graph.rs (target\debug\deps\p02a_graph-23af5d85695a72af.exe)
   Doc-tests polaris_core
```

```powershell
> cargo test -p polaris-core --test p02a_graph
running 3 tests
test structural_mapping_requires_typed_edge_match ... ok
test structural_mapping_scores_typed_two_hop_overlap ... ok
test maps_to_candidate_is_written_only_after_threshold ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.68s
     Running tests\p02a_graph.rs (target\debug\deps\p02a_graph-cc1752b1df337efd.exe)
```

```powershell
> cargo run -p polaris-cli -- pack validate packs/rust
pack ok: concepts=24 prerequisites=21 misconceptions=11
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.40s
     Running `target\debug\polaris.exe pack validate packs/rust`
```

### 审查记录

- 子 agent `Bacon` 已做只读 P02A 审查：无阻塞问题；确认未越过 P02A 范围。
- 已采纳审查建议：
  - 回滚清单补入 `crates/polaris-core/src/error.rs`。
  - `p02a_graph` 增加 `maps_to` 写入边 `src/dst/type/weight/provenance` 断言。

### 阻塞与裁决记录

无。

### 技术选择说明

- 2-hop 邻域按**有向边向外**展开；这样不会因为两个 schema 共享组件而把对方 schema 吸入本节点邻域。
- 结构边按 `(src,dst,type)` 去重，避免重复记录同一语义边导致分数被抬高。
- `maps_to` 候选计算时不把既有 `maps_to` 边纳入结构邻域，避免候选边反过来污染自己的分数。
- 嵌入缺失时只匹配根节点和 id 相同节点；若后续已有 embedding BLOB，则按 f32 小端向量解析后用正 cosine 相似度参与贪心配对。
