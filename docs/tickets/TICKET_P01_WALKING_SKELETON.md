# P01 — Walking Skeleton（Rust 内核最小闭环）

**目标**：单 pack（rust）、零集成，跑通完整学习闭环：
ingest 证据 → Tier 1 评分（strict-citation、异步、乐观更新）→ 掌握度更新（R / p_known / C 观测层）→ 按缺口选下一任务 → 提交 → 回写。

**服务主命题的哪一环**：全部三环的最小可验证版本。

## 范围（做这些，且只做这些）

1. **Cargo workspace**：`crates/polaris-core`（引擎库）+ `crates/polaris-cli`（bin，命令名 `polaris`）。依赖：rusqlite(bundled)、serde/serde_json、thiserror、clap、reqwest + tokio（或 blocking，二选一并在票尾说明理由）、toml。
2. **SQLite v1 迁移**：严格按 `docs/DATA_MODEL.md` 的 P01 表（含预留空字段 q/embedding/kind 等）。WAL。库路径默认 `%USERPROFILE%\.polaris-core\core.db`，env `POLARIS_CORE_DB` 覆盖。
3. **FSRS 移植**：从 `C:\MyProject\Polaris\apps\web\src\lib\fsrs.ts` 1:1 移植（w[0..16]、initStability/initDifficulty/nextDifficulty/retrievability/nextRecallStability/nextForgetStability/stabilityToInterval/calculateNextDue）。单测对拍 ≥5 条序列（在注释里写明每条期望值如何从 ts 版推得）。score→rating 映射按 DATA_MODEL。
4. **观测层掌握度（事件溯源语义，DATA_MODEL §0）**：attempts 为事实源，mastery_states 为确定性 fold 的物化视图；provisional 增量 fold，final 到达对该概念**全量重放**。BKT（含 p_init）+ 校准（self_confidence 必须在看到反馈前采集；Brier EWMA、calib_gap EWMA）+ R（elapsed_days 定义见 DATA_MODEL §3）。公式与全部参数严格按 DATA_MODEL。**config 模块为每个参数携带（默认值, 边界, 类型 A/B/C, 调优途径）四元组**（DATA_MODEL §10）——这是后续 P03H 自调优不改代码的前提；代码任何地方不得出现裸常数。
5. **行为观测**：sessions + behavior_events；CLI 自动记录 latency（`next` 到 `submit` 的间隔）、hint 计数（`polaris hint` 命令，只给低信息提示语模板）、放弃（`polaris abandon`）、时段。
6. **rust pack 最小版**：`packs/rust/{pack.toml, concepts.toml, misconceptions.toml, rubric.md, moves.toml}`。
   - ≥24 个概念（须含：ownership、borrowing、lifetimes、Result/Option、traits、泛型、迭代器、模式匹配、模块系统、错误处理、智能指针、String/&str、Vec/切片、闭包），内容可参考 The Rust Book 章节结构与 `C:\MyProject\Learned\rust-mastery-lab\domain\rust\*.toml`；
   - ≥15 条 prerequisite 边、≥10 条误解（含经典款："lifetime 标注延长存活时间"、"String 和 &str 可互换"、"clone 总是性能问题"）；
   - `polaris pack validate packs/rust`：校验文件齐全、引用完整（边两端概念存在、误解挂到概念）、字段合法。
7. **Tier 1 grader**：env `POLARIS_LLM_FAST_*`/`POLARIS_LLM_STRONG_*`（BASE_URL/MODEL/API_KEY）。返回 `{score∈[0,1], depth, misconception_id?, citations[]}`；strict-citation 校验失败→重试一次→降级（启发式 + 入 grade_queue）；`polaris grade-pending` 重试队列。**乐观更新**：提交即 provisional，评分到达后回填并打印修正差异。rubric.md 内容注入评分 prompt。
8. **调度/选题**：`U(c)` 按 DATA_MODEL；`polaris next` 输出任务（概念 + 任务型 + 三行理由：选它因为/证据是/现在做什么）。任务文本生成：有 LLM 时由 FAST 模型按 moves.toml 模板现生成，无 LLM 时用模板字面量。
9. **CLI 全集**：`polaris init | ingest | next | submit | hint | abandon | status | grade-pending | pack validate`。`status`：概念表（R / p_known / calib_gap / 粗相位标记：幻影⚠ 或正常）+ 今日 due 数。
10. **集成测试**（必须自动化）：种子流 ≥6 概念、≥10 attempts（含"高自信低分"样例），断言：
    (a) 高自信低分概念被 `next` 优先（U 排序正确）；
    (b) provisional→final 修正生效且历史不被覆盖；
    (c) FSRS due 随 rating 正确推进；
    (d) LLM env 缺失时全流程照常（降级路径），grade_queue 有记录。

## 禁区（本票绝不做——发现自己在做这些立即停）

θ/MIRT/Q 拟合、嵌入/HNSW、图式归纳/夜间巩固、HMM/hazard/镜像报告、MRT、MCP server、HTTP API、Tauri/任何 UI、第二个 pack、育种、目标引擎、planner、识屏/浏览器集成。schema 预留字段建好即可，不写其逻辑。

## 验收

- SPEC §6 基线全绿（fmt / clippy -D warnings / test --workspace），输出粘贴票尾。
- 测试覆盖：FSRS 对拍≥5、BKT≥4（对/错/中间区/参数读 meta）、校准≥3、strict-citation≥4（通过/过短/过长/非子串）、U(c) 排序≥2（含**平手决定性**：seed_order→id）、pack 校验≥2（合法/缺引用）、集成流 1。
- **属性测试（必须）**：(a) 增量 fold == 全量重放，对任意 attempt 序列（含 provisional→final 乱序到达）；(b) 同输入同输出（决定性）；(c) misconception_active 的 14 天窗口与"其后无≥0.75"语义。
- README quickstart 实跑记录贴票尾。
- **回滚**：删除 `crates/`、`packs/` 新增文件即可（无外部副作用，DB 在用户目录可手动删）。

## 交付记录（完成时填写）

- 变更清单：
  - 新增 Cargo workspace：`crates/polaris-core`、`crates/polaris-cli`。
  - 新增 SQLite P01 迁移、WAL 打开、meta 参数登记处。
  - 1:1 移植 `C:\MyProject\Polaris\apps\web\src\lib\fsrs.ts` 的 FSRS 参数与公式，并加入 5 条 TS 对拍序列。
  - 实现 attempts → mastery_states 的确定性 fold、BKT、校准、FSRS 状态、provisional→final 全量重放。
  - 实现 strict-citation 校验、无 LLM 降级评分与 `grade_queue`。
  - 新增声明式 `packs/rust`：24 个概念、21 条 prerequisite、11 条误解、rubric、moves。
  - 实现 U(c) 调度、误解窗口语义测试、engine 编排和 CLI 命令集。
  - 新增 `docs/AI_RUNBOOK.md` 与 P01 实现计划，支持新窗口续跑。
- 验收输出：

  `cargo fmt --check`

  ```text
  exit 0（无 stdout）
  ```

  `cargo clippy --workspace --all-targets -- -D warnings`

  ```text
  Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
  Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.68s
  ```

  `cargo test --workspace`

  ```text
  running 1 test
  test tests::parses_required_command_set ... ok

  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  running 27 tests
  test citation::tests::rejects_citation_to_unrelated_evidence ... ok
  test citation::tests::rejects_short_quote ... ok
  test citation::tests::accepts_quote_that_is_allowed_evidence_substring ... ok
  test citation::tests::rejects_long_quote ... ok
  test citation::tests::rejects_non_substring_quote ... ok
  test fsrs::tests::retrievability_and_interval_match_reference ... ok
  test fsrs::tests::score_maps_to_rating_thresholds_from_data_model ... ok
  test config::tests::parameter_registry_contains_p01_constants ... ok
  test fsrs::tests::fsrs_matches_typescript_reference_sequences ... ok
  test mastery::tests::calibration_updates_gap_and_skips_brier_in_dead_zone ... ok
  test scheduler::tests::misconception_active_respects_window_and_later_success ... ok
  test scheduler::tests::high_positive_calibration_gap_can_raise_priority ... ok
  test mastery::tests::bkt_updates_correct_wrong_dead_zone_and_explain_guess ... ok
  test scheduler::tests::misconception_and_new_concept_terms_follow_data_model ... ok
  test scheduler::tests::ties_sort_by_seed_order_then_id ... ok
  test tests::crate_exports_version ... ok
  test mastery::tests::incremental_fold_matches_full_replay_for_final_score_arrival ... ok
  test pack::tests::validates_builtin_rust_pack_shape ... ok
  test db::tests::migration_creates_p01_tables_and_default_meta ... ok
  test grader::tests::missing_llm_config_degrades_to_heuristic_and_queues_retry ... ok
  test pack::tests::rejects_misconception_with_missing_concept_reference ... ok
  test engine::tests::submit_without_llm_records_provisional_mastery_and_retry_queue ... ok
  test engine::tests::final_score_replay_preserves_provisional_history ... ok
  test engine::tests::init_pack_installs_concepts_and_next_returns_first_open_concept ... ok
  test engine::tests::next_task_uses_engine_misconception_window_semantics ... ok
  test engine::tests::integration_seed_flow_prioritizes_high_confidence_low_final_score ... ok
  test db::tests::file_database_uses_wal ... ok

  test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

  Doc-tests polaris_core
  ```

  README quickstart 实跑：

  ```text
  cargo run -p polaris-cli -- pack validate packs/rust
  pack ok: concepts=24 prerequisites=21 misconceptions=11

  cargo run -p polaris-cli -- --db target\p01-quickstart.db init --pack packs/rust
  initialized

  cargo run -p polaris-cli -- --db target\p01-quickstart.db next
  concept: ownership
  task_type: recall
  prompt: 用自己的话说明 Ownership 的核心约束。
  选它因为：当前效用最高。
  证据是：U=0.100，按 FSRS/校准/误解/新概念门槛计算。
  现在做什么：完成一个 recall 任务。

  cargo run -p polaris-cli -- --db target\p01-quickstart.db submit --concept ownership --response "Ownership controls which binding can drop a value." --confidence 4
  attempt: d8a35c47-d7ab-462f-9c74-ba6f84c288e8 provisional_score=0.700 degraded=true

  cargo run -p polaris-cli -- --db target\p01-quickstart.db status
  due_today=0
  ownership	Ownership	R=1.000	p_known=0.200	calib_gap=0.015	phase=正常

  cargo run -p polaris-cli -- --db target\p01-quickstart.db grade-pending
  processed=0 pending=1
  ```

- 阻塞与裁决记录：
  - `cargo clippy --workspace --all-targets -- -D warnings` 在普通沙箱内写 `target\debug\deps\*.rmeta` 时出现 Windows `拒绝访问 (os error 5)`，未进入代码诊断。检查目录属性和进程后，按沙箱规则提升权限重跑同一条命令，验收通过。
- 技术选择说明（如 tokio vs blocking）：
  - 选择 `reqwest` blocking feature。P01 CLI 是同步命令行工具，Tier 1 缺失时必须立即降级并排队；blocking 能减少 tokio runtime 面积，异步后台执行留到后续票。
  - 为 UUIDv4 文本 ID 增加 `uuid` 依赖；其余核心依赖按票面要求使用。
  - 回滚：删除 `Cargo.toml`、`Cargo.lock`、`crates/`、`packs/`、`docs/superpowers/plans/2026-06-11-p01-walking-skeleton.md`，并还原 `README.md`、`AGENTS.md`、`docs/AI_RUNBOOK.md`、`docs/tickets/QUEUE.md`、本票交付记录。测试 quickstart DB 位于 `target\p01-quickstart.db`，可手动删除；默认用户 DB 未被写入。

## 子 agent 审查补修记录（2026-06-11）

- 审查来源：
  - Ptolemy：DATA_MODEL/DDL/公式一致性审查。
  - Plato：CLI/engine/pack/P01 闭环审查。
  - Pauli：测试与文档验收审查。
- 补修变更：
  - `meta` 参数读取改为运行时路径：BKT、FSRS、scheduler、strict-citation、provisional score、status 幻影阈值均从 DB `meta` 读取。
  - 补齐 DATA_MODEL “后续激活（建表即可）”表：`theta`、`theta_history`、`residual_stats`、`consolidation_runs`、`moves_effects`、`mrt_log`、`param_tuning_runs`。
  - 修正 fold/FSRS 时间语义：按 `created_at ASC, id ASC` 回放，按相邻 attempt 的真实日期计算 `elapsed_days`，`next_due_at` 存真实 UTC 04:00 ISO 时间。
  - Tier 1 grader 接入 `POLARIS_LLM_FAST_*` / `POLARIS_LLM_STRONG_*`，解析 `{score, depth, misconception_id?, citations[]}`，strict-citation 校验失败重试后降级入队；`grade-pending` 真实处理队列。
  - `rubric.md` 随 pack 初始化写入 `meta('pack.<id>.rubric')`，真实 LLM prompt 注入 rubric 与 allowed evidence id/text。
  - CLI `next` 记录行为事件；`submit` 自动计算 next→submit latency 与 hint_count；`status` 使用真实 R 衰减和 due 时间。
  - 补属性测试：任意序列 final 乱序到达后重放等价、同输入同输出、misconception_active 14 天窗口与后续成功清除语义。
- 验收输出：

  `cargo fmt --check`

  ```text
  exit 0（无 stdout）
  ```

  `cargo test --workspace`

  ```text
  running 2 tests
  test tests::parses_required_command_set ... ok
  test tests::behavior_observation_reads_latency_and_hint_count_since_last_next ... ok

  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

  running 37 tests
  ...
  test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

  Doc-tests polaris_core
  ```

  `cargo clippy --workspace --all-targets -- -D warnings`

  ```text
  Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
  Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s
  ```

  README quickstart 实跑（使用 `target\p01-followup-quickstart.db`）：

  ```text
  cargo run -p polaris-cli -- pack validate packs/rust
  pack ok: concepts=24 prerequisites=21 misconceptions=11

  cargo run -p polaris-cli -- --db target\p01-followup-quickstart.db init --pack packs/rust
  initialized

  cargo run -p polaris-cli -- --db target\p01-followup-quickstart.db next --session quickstart
  concept: ownership
  task_type: recall
  prompt: 用自己的话说明 Ownership 的核心约束。
  选它因为：当前效用最高。
  证据是：U=0.100，按 FSRS/校准/误解/新概念门槛计算。
  现在做什么：完成一个 recall 任务。

  cargo run -p polaris-cli -- --db target\p01-followup-quickstart.db submit --concept ownership --response "Ownership controls which binding can drop a value." --confidence 4 --session quickstart
  attempt: a233e9b8-7db5-4735-a292-f331b09a021c provisional_score=0.700 degraded=true

  cargo run -p polaris-cli -- --db target\p01-followup-quickstart.db status
  due_today=0
  ownership	Ownership	R=1.000	p_known=0.200	calib_gap=0.015	phase=正常
  moves	Moves	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  borrowing	Borrowing	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  mutable_borrowing	Mutable borrowing	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  lifetimes	Lifetimes	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  references	References	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  string_str	String and &str	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  vec_slices	Vec and slices	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  option_result	Result and Option	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  error_handling	Error handling	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  pattern_matching	Pattern matching	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  traits	Traits	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  generics	Generics	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  iterators	Iterators	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  closures	Closures	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  modules	Module system	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  smart_pointers	Smart pointers	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  box_rc_arc	Box, Rc, and Arc	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  interior_mutability	Interior mutability	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  drop	Drop	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  copy_clone	Copy and Clone	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  concurrency	Concurrency	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  send_sync	Send and Sync	R=-	p_known=0.200	calib_gap=0.000	phase=正常
  async_await	Async and await	R=-	p_known=0.200	calib_gap=0.000	phase=正常

  cargo run -p polaris-cli -- --db target\p01-followup-quickstart.db grade-pending
  processed=0 pending=1
  ```

- 阻塞与裁决记录：
  - `cargo clippy --workspace --all-targets -- -D warnings` 在普通沙箱内写 `target\debug\deps\*.rmeta` 时再次出现 Windows `拒绝访问 (os error 5)`；按沙箱规则提升权限重跑同一条命令，验收通过。
- 回滚：
  - 本次补修可通过回退 follow-up commit 撤销；quickstart DB 位于 `target\p01-followup-quickstart.db`，可手动删除。

## AI 交接记录（2026-06-11 开工）

- 当前状态：P01 已实现并完成子 agent 审查补修；`docs/tickets/QUEUE.md` 已标为完成，P02 未认领。
- 已完成：Rust workspace、SQLite 迁移、pack、FSRS/BKT/校准、Tier 1 grader 降级与队列、CLI 闭环、属性测试、README quickstart 与票尾验收记录。
- 已跑验证：`cargo fmt --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 均通过；quickstart 使用 `target\p01-followup-quickstart.db` 实跑。
- 阻塞点：无。
- 下一步建议：等待本次 follow-up commit 后，再按单票制认领 P02。
