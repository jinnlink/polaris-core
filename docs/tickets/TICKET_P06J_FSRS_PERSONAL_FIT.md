# P06J FSRS 个人参数拟合

状态：已通过验收（2026-06-17）

服务环节：定位模糊 → 针对性补缺（让 R 保持度按个人复习史校准，减少过早/过晚复习）

## 背景

`DATA_MODEL.md` 已把 `fsrs.w[0..16]` 登记为 C 类、Fit 路由：默认值只是 Polaris 移植初始化，未来应按个人复习史拟合遗忘曲线。P03J 已实现 B 类 Replay 自调优，但它只允许触碰 B 类且 route=Replay 的参数；`fsrs.w` 不属于 P03J 白名单，必须走单独的 C 类显式拟合通道。

## 范围

1. 新增 FSRS 个人拟合模块：
   - 只读取 `final_score IS NOT NULL` 的历史 attempt；不用 provisional 分数训练个人遗忘曲线。
   - 时间序留出：最后 `fsrs_fit.holdout_frac` 的可预测复习作为 holdout；预测必须发生在 fold 当前 attempt 之前。
   - 指标：FSRS retrievability 对二值 recall 成功的 holdout logloss；`Rating::Again` 为失败，其余为成功。
   - 候选：从当前 `fsrs.w` 出发做确定性的轻量 coordinate search；训练段选候选，holdout 段只做对拍。
   - 接受门：`current_holdout_logloss - candidate_holdout_logloss >= fsrs_fit.accept_margin` 才写入 `meta('fsrs.w')`。
2. 全程审计：
   - 被评估但不过门时写 `param_tuning_runs(param='fsrs.w', status='rejected')`。
   - 过门时写 `status='accepted'`，`old_value/new_value` 为 17 项 JSON。
   - 样本不足或 holdout 可预测样本不足时返回 skipped，不写审计行。
3. 状态一致性：
   - 接受新 `fsrs.w` 后，立即对已有 attempt 的所有相关概念重放，刷新 `mastery_states.fsrs_json` 与 `next_due_at`。
4. 引擎与 CLI：
   - 新增 `Engine::fit_fsrs_personal_params()`。
   - 新增 CLI `polaris fsrs-fit [--json]`，输出 accepted/rejected/skipped、指标改善、训练/留出样本与重放概念数。
5. 参数登记与文档：
   - 新增 `fsrs_fit.min_attempts`、`fsrs_fit.min_holdout_predictions`、`fsrs_fit.holdout_frac`、`fsrs_fit.accept_margin`，均为 A 类 Manual 验证门。
   - 更新 `docs/DATA_MODEL.md` 与 `docs/PARAMETERS.md`。

## 禁区

- 不改 P01 移植的 FSRS 基础公式。
- 不把 `fsrs.w` 加入 P03J 的 B 类夜间自调优白名单。
- 不调 `fsrs.r_again/r_hard/r_good`；这些仍是 MRT/手动路径。
- 不调任何 A 类门槛；门槛只能用户手改。
- 不引入外部 optimizer 依赖，不联网，不触碰冻结参考库。
- 样本不足、指标不过门时不得改变 `fsrs.w`、调度或报告话术。

## 验收命令

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p06j_fsrs_personal_fit
cargo test -p polaris-core parameter_registry_contains_fsrs_fit_gates
cargo test -p polaris-cli fsrs_fit
cargo test --workspace
git diff --check
```

## 回滚方式

```powershell
git checkout -- crates/polaris-core/src/fsrs.rs crates/polaris-core/src/fsrs_fit.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/engine/submit_pipeline.rs crates/polaris-core/src/lib.rs crates/polaris-core/src/config.rs crates/polaris-cli/src/main.rs crates/polaris-core/tests/p06j_fsrs_personal_fit.rs docs/DATA_MODEL.md docs/PARAMETERS.md docs/tickets/QUEUE.md docs/tickets/TICKET_P06J_FSRS_PERSONAL_FIT.md
```

## AI 交接记录（2026-06-17）

- 当前状态：P06J 仍为 In Progress；核心实现和首批测试已经落地，尚未完成全量验收、最终自审和提交。
- 已完成：新增 `fsrs_fit` 模块；新增 `Engine::fit_fsrs_personal_params()`；新增 CLI `polaris fsrs-fit [--json]`；新增 P06J 集成测试；补充 `fsrs_fit.*` 参数注册和 `DATA_MODEL.md`、`PARAMETERS.md`、`ENHANCEMENT_ROADMAP.md`、`QUEUE.md` 文档更新。
- 已实跑验证：TDD 首轮失败符合预期；`cargo fmt --check` 通过；`cargo test -p polaris-core --test p06j_fsrs_personal_fit` 通过；`cargo test -p polaris-core parameter_registry_contains_fsrs_fit_gates` 通过。
- 未完成验证：`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test -p polaris-cli fsrs_fit`、`cargo test -p polaris-core params_doc_keys_match_registry`、`cargo test -p polaris-core fsrs_matches_typescript_reference_sequences`、`cargo test --workspace`、`git diff --check`。
- 注意事项：Windows 下不要并行跑共享同一个 `CARGO_TARGET_DIR` 的 Cargo 命令；此前并行测试出现过 `.cargo-lock` / fingerprint 访问冲突。建议串行执行，并使用 `$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-target-p06j-seq'`。
- 下一步建议：先审当前 diff，必要时补 `doctor_report`、确定性、holdout 泄漏边界测试；再串行跑完验收命令，把真实输出写入本票尾部，更新 `QUEUE.md` 状态，最后按中文提交规范 commit。

## 交付记录（2026-06-17）

### 变更清单

- 新增 `crates/polaris-core/src/fsrs_fit.rs`：显式 FSRS 个人参数拟合入口，仅读取 `final_score IS NOT NULL` 历史，按 prequential replay 切分 train / holdout，用 holdout logloss 门控 `fsrs.w` 写入。
- 新增 `Engine::fit_fsrs_personal_params()` 与 CLI `polaris fsrs-fit [--json]`；评估、审计、`fsrs.w` 写入和 accepted 后重放统一放在 `BEGIN IMMEDIATE` 事务内，避免旧快照写入。
- accepted 时写 `param_tuning_runs(param='fsrs.w', metric='fsrs_holdout_logloss', status='accepted')` 并重放所有已有 scored attempts 的概念；rejected 只写 rejected 审计；skipped 不写审计、不改 `meta`。
- `FsrsFitSummary` 保留审计用 `old_value/new_value` 字符串，同时新增 `old_weights/candidate_weights` 结构化数组；CLI rejected 文案改为 `kept=... candidate=...`，避免误读为已写入。
- 注册 `fsrs_fit.min_attempts`、`fsrs_fit.min_holdout_predictions`、`fsrs_fit.holdout_frac`、`fsrs_fit.accept_margin` 为 A 类 Manual 门槛，更新 `DATA_MODEL.md`、`PARAMETERS.md`、`ENHANCEMENT_ROADMAP.md`、`QUEUE.md`。
- 新增 P06J 测试覆盖：样本不足 skipped、provisional-only 不参与、首条复习不算可预测样本、accepted 写入/审计/重放、accepted 后 doctor clean、rejected 不改写、P03J 不触碰 `fsrs.w`、只读评估确定性、holdout 不影响候选搜索、skipped helper no-op、stale `fsrs.w` CAS 防护、CLI rejected 文案。

### 子 agent 审查处理

- 规格审查：未发现 P06J 核心规格阻断；指出票外脏文件不可纳入本票提交，并建议收窄 `apply_fsrs_fit_summary`。已将 helper 改为 `pub(crate)`，并对 skipped 直接 no-op。
- 质量审查：指出评估和写入不在同一快照、skipped holdout 计数误导、CLI rejected 文案易误读、缺 future leakage 测试。已将评估到 replay 纳入同一事务，修正 skipped 预测计数，补结构化 JSON 权重和 future leakage 测试。
- 未采纳项：未把内部搜索域常量注册为用户参数；当前它们是 optimizer 内部数值保护，不属于用户治理门槛，避免扩大 A 类参数面。

### 验收输出

```powershell
> cargo fmt --check
# exit 0，无输出
```

```powershell
> cargo test -p polaris-core --test p06j_fsrs_personal_fit
running 9 tests
test insufficient_final_history_skips_without_audit_or_meta_change ... ok
test first_reviews_do_not_count_as_holdout_predictions ... ok
test provisional_scores_are_not_used_for_personal_fsrs_fit ... ok
test p03j_param_tuning_still_never_touches_fsrs_w ... ok
test rejected_fit_keeps_fsrs_w_and_does_not_replay_mastery_states ... ok
test accepted_fit_updates_fsrs_w_audits_and_replays_existing_concepts ... ok
test accepted_fit_leaves_doctor_clean_and_does_not_touch_shadow_tables ... ok
test fsrs_fit_evaluation_is_deterministic_for_same_database_state ... ok
test holdout_outcomes_do_not_influence_candidate_search ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```powershell
> cargo test -p polaris-core fsrs_fit
running 4 tests
test fsrs_fit::tests::fsrs_w_is_class_c_fit_not_b_replay ... ok
test config::tests::parameter_registry_contains_fsrs_fit_gates ... ok
test fsrs_fit::tests::accepted_fit_application_rejects_stale_fsrs_w_snapshot ... ok
test fsrs_fit::tests::skipped_fit_summary_application_is_noop ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 76 filtered out
```

```powershell
> cargo test -p polaris-cli fsrs_fit
running 2 tests
test tests::fsrs_fit_text_marks_rejected_value_as_candidate ... ok
test tests::fsrs_fit_json_flag_parses ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 70 filtered out
```

```powershell
> cargo test -p polaris-core parameter_registry_contains_fsrs_fit_gates
test config::tests::parameter_registry_contains_fsrs_fit_gates ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out
```

```powershell
> cargo test -p polaris-core params_doc_keys_match_registry
test config::tests::params_doc_keys_match_registry ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out
```

```powershell
> cargo test -p polaris-core fsrs_matches_typescript_reference_sequences
test fsrs::tests::fsrs_matches_typescript_reference_sequences ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.35s
```

```powershell
> cargo test --workspace
test result: ok. 72 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
...
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Doc-tests polaris_core
# exit 0
```

```powershell
> git diff --check
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-cli/src/main.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/config.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine/submit_pipeline.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/fsrs.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/DATA_MODEL.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/ENHANCEMENT_ROADMAP.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/PARAMETERS.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
# exit 0，无 whitespace error
```
