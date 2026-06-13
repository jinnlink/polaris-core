# TICKET P05B — 教法育种引擎（F5）

## 状态

已实现并提交

## 服务主命题

针对性补缺（方法库扩展必须先验证真懂）：新教法只能作为候选假设进入预登记 MRT，对个体后验胜过在位者后才准入库；准入后若效应衰减必须自动退役。

## 设计锚点

- `SPEC.md` §2：教学策略引擎包含 moves 库 + F1/F2/F3 + MRT 因果个人化（+ F5 育种）。
- `SPEC.md` §3：育种 move 也必须过验证门，不过门只能是假设，不得进产品话术与默认行为。
- `MASTER_PLAN.md` F5：机制原语组合候选微方法，预登记 MRT 测试，`τ` 后验以 >0.8 概率优于在位者才准入库；准入后持续监控，效应衰减自动退役。
- `DATA_MODEL.md` §8：MRT 日志写 `prereg_id`；Thompson/Beta 后验用于 move 效应。

## 范围

1. 新增 core 内部的育种候选生命周期：`preregistered → admitted → retired`。
2. 新增持久化表保存候选 move、在位者 move、上下文桶、机制原语、预登记 JSON、后验比较和生命周期状态。
3. 新增 core/Engine API：
   - 预登记候选 move，并写入 `mrt_log` 审计；
   - 记录候选/在位者 MRT 成败样本，更新 `moves_effects`；
   - 评估候选准入和准入后退役；
   - 查询已准入的育种 move。
4. 新增配置登记：`breeding.admit_p`、`breeding.retire_p`、`breeding.min_n`，均为 A 类治理门槛，手动调整。

## 禁区

- 不在 core 中调用 LLM 生成候选；LLM 只可在上层按本 API 提供候选 payload。
- 不把未准入候选接入默认调度或 `next_task`。
- 不写任何领域特定逻辑。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并写回输出：

```powershell
cargo fmt --check
cargo test -p polaris-core --test p05b_breeding
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

若默认 target 遭遇 Windows 文件锁，可追加同参数隔离 target clippy，但必须保留默认失败原文。

## 回滚方式

删除本票新增的 `breeding` 模块、测试、迁移表、配置登记、Engine API 与 QUEUE/票据改动；已有 `moves_effects` 与 `mrt_log` 表不回滚。

## 本轮范围（2026-06-13）

- 先以 core 可审计骨架实现 F5 v1：预登记、MRT 证据记录、Beta 后验比较、准入与退役。
- 不做 LLM 候选生成，不改变现有默认教学调度。

## 交付记录（2026-06-13）

### 变更清单

- 新增 `crates/polaris-core/src/breeding.rs`：候选 move 预登记、MRT 样本记录、`moves_effects` 更新、Beta 后验胜率评估、准入与退役。
- 新增 `bred_moves` 表与 `idx_bred_moves_status_context` 索引；迁移测试已覆盖。
- 新增 `Engine` 育种 API：`preregister_bred_move`、`record_bred_move_outcome`、`evaluate_bred_moves`、`admitted_bred_moves`。
- 新增 A 类治理参数：`breeding.admit_p=0.80`、`breeding.retire_p=0.50`、`breeding.min_n=6`。
- 更新 `docs/DATA_MODEL.md` 与 `docs/tickets/QUEUE.md`。
- 新增 `crates/polaris-core/tests/p05b_breeding.rs`，覆盖预登记审计、未准入不入库、预登记冻结门槛、最小样本门、后验准入、效应衰减退役、配置门。

### 技术选择

- core 不生成候选教法，只接收上层/LLM 提供的候选 payload，并强制写入预登记审计。
- `moves_effects` 保存 Beta(1,1)+样本计数；评估时叠加 `thompson.prior_n` 的中性先验，并复用 `report::prob_beta_greater` 计算 `P(τ_candidate > τ_incumbent)`。
- 未准入候选不接入 `next_task` 或默认调度，只能通过 `admitted_bred_moves(context_hash)` 查询。

### 验收输出

```text
> cargo fmt --check
exit 0
```

```text
> cargo test -p polaris-core --test p05b_breeding
running 5 tests
test breeding_parameters_are_governance_gates ... ok
test preregistration_writes_audit_and_keeps_candidate_out_of_admitted_library ... ok
test admission_uses_frozen_preregistration_gates_not_current_meta ... ok
test candidate_admits_only_after_posterior_beats_incumbent_with_minimum_n ... ok
test admitted_move_retires_when_effect_decays_below_incumbent ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

```text
> cargo test --workspace
首次运行失败于既有 P03C geometry 偶发点：
thread 'geometry_candidates_use_hnsw_and_combined_scores' ... schema:raii candidate
后续两个失败为 ENV_LOCK PoisonError 连锁失败。

排查：
git diff -- crates\polaris-core\src\geometry.rs crates\polaris-core\tests\p03c_geometry.rs
无输出（本票未改 geometry）。

> cargo test -p polaris-core --test p03c_geometry
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

> cargo test --workspace
最终复跑通过；关键末段：
running 5 tests
test english_pack_validates_expected_cefr_shape ... ok
test english_pack_initializes_and_schedules_domain_concepts ... ok
test cefr_prerequisite_gate_keeps_c1_c2_out_until_intermediate_ready ... ok
test failed_english_attempt_with_misconception_raises_repair_priority ... ok
test english_and_rust_packs_share_submit_grade_mastery_shape ... ok

running 5 tests
test breeding_parameters_are_governance_gates ... ok
test preregistration_writes_audit_and_keeps_candidate_out_of_admitted_library ... ok
test admission_uses_frozen_preregistration_gates_not_current_meta ... ok
test candidate_admits_only_after_posterior_beats_incumbent_with_minimum_n ... ok
test admitted_move_retires_when_effect_decays_below_incumbent ... ok

Doc-tests polaris_core
exit 0
```

```text
> cargo clippy --workspace --all-targets -- -D warnings
默认 target 失败于 Windows 文件锁：
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)

> $env:CARGO_TARGET_DIR="$env:TEMP\polaris-core-target-p05b-clippy"; cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.18s
exit 0
```

### 代码审查处理

- 子 agent 只读审查发现：准入评估使用当前 meta，而非预登记时写入 `prereg_json` 的 `min_n`/`admit_p`；已新增红绿回归 `admission_uses_frozen_preregistration_gates_not_current_meta`。
- 红灯输出：`assertion left == right failed; left: 1 right: 0`，说明预登记后放宽 meta 会提前准入。
- 修复：`evaluate_bred_moves` 读取 `prereg_json` 中冻结的 `min_n`/`admit_p`；`preregister_bred_move` 与 `record_bred_move_outcome` 使用 SQLite transaction 让候选/审计、样本/审计成组写入。
- 复验：新增回归单测通过；完整 `cargo test -p polaris-core --test p05b_breeding` 为 5/5；`cargo test --workspace` 复验通过；隔离 target clippy 通过。
- `.gitignore` 为票外既有改动，未纳入本票实现范围。

```text
> git diff --check
仅有既存/仓库换行提示：
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/config.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/db.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/DATA_MODEL.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it

> rg -n "[ \t]+$" crates\polaris-core\src\breeding.rs crates\polaris-core\tests\p05b_breeding.rs docs\tickets\TICKET_P05B_PEDAGOGY_BREEDING.md
无输出
```

### 阻塞与裁决记录

- P05B 无阻塞。
- `cargo test --workspace` 首次命中 P03C geometry 既有偶发失败；单跑 P03C 与复跑 workspace 均通过。该问题已在 QUEUE Backlog 记录，未在本票内顺手修复。
- 默认 target clippy 仍受 Windows 文件锁影响；按票据约定使用隔离 target 同参数通过。

### 回滚方式

删除 `crates/polaris-core/src/breeding.rs`、`crates/polaris-core/tests/p05b_breeding.rs`；从 `lib.rs`、`engine.rs`、`config.rs`、`db.rs`、`docs/DATA_MODEL.md`、`docs/tickets/QUEUE.md` 移除本票新增内容；删除本票文档。已有 `moves_effects` 与 `mrt_log` 不回滚。

## AI 交接记录（2026-06-13）

- 当前状态：P05B 已实现并提交。
- 已完成：票据、测试、迁移、配置、Engine API、DATA_MODEL 更新、子 agent 审查反馈修复、验收记录。
- 未完成：未接入默认调度（按本票禁区保留）。
- 已跑验证：`cargo fmt --check`、`cargo test -p polaris-core --test p05b_breeding`、`cargo test -p polaris-core --test p03c_geometry`、`cargo test --workspace`（复跑通过；审查修复后再次通过）、隔离 target `cargo clippy --workspace --all-targets -- -D warnings`。
- 未跑验证及原因：默认 target clippy 因 Windows 文件锁失败，已记录原文并用隔离 target 同参数通过。
- 阻塞点：无 P05B 阻塞。
- 下一步建议：按 QUEUE 推进 P05C 或用户指定的新票。
