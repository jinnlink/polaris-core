# TICKET P06I: G_u 层级 Beta 超先验 shadow gate

状态：已通过验收（2026-06-17）

服务环节：定位模糊 -> 针对性补缺。该票只增强 F4 误解语法的可验证先验评估，不改变 G_u 生命周期、评分提示词、调度、报告或默认产品行为。

## 背景

P03H 已实现个人误解语法 G_u 的自动归纳、验证、激活、解决与退役；P06A/P10A 已让 G_u 状态可被 MCP 和信任面板读取。当前缺口是：每条 G_u 规则仍以平坦 `Beta(1,1)` 起步，系统尚未评估“同 pattern、相关概念邻域”的历史证据是否能作为可验证的层级先验。

增强路线图把“G_u 层级 Beta 超先验”列为数学深化候选：同 pattern 跨概念簇共享 Beta 超先验，新概念装入时 `gu_risk` 概率化；但在未过验证门前只能作为 shadow 统计，不进入生产行为。

## 本轮范围

1. 新增只读 G_u 层级先验 shadow 模块，数据源仅限现有 `gu_rules`、`attempts.grader_json.pattern_tags`、`concepts`、`edges`。
2. 对每条可评估 G_u 规则，构造两套对照先验：
   - flat baseline：现行平坦 `Beta(1,1)`。
   - hierarchical shadow：同 pattern 的既有规则概念 + 当前规则概念的一跳图谱邻域，在 holdout 起点之前的证据，折算为有上限的 pseudo-count。
3. 用当前规则 `last_seen` 之后、`gu.window_days` 内的相关 attempts 做时间序 holdout，输出 flat vs hierarchical 的 logloss / Brier / 命中率摘要。
4. 输出 shadow 状态：`no_data`、`insufficient_data`、`shadow_ready`。样本不足时必须显式 skipped，不得制造结论。
5. 注册 A 类手动参数：`gu_prior.min_shadow_rules`、`gu_prior.min_holdout_attempts`、`gu_prior.max_prior_strength`。
6. 暴露一个 Engine 只读 facade，供后续 CLI/HTTP/MCP 票复用。本票不接入默认用户界面。
7. 更新 `DATA_MODEL.md` 与 `PARAMETERS.md`，说明该票的 shadow 语义、数据源、验证门和不改变产品行为。

## 禁区

- 不改 schema / DDL / `PRAGMA user_version`。
- 不改变 `gu_rules.status`、`alpha`、`beta`、`correct_streak`、`consumed_at` 等生命周期字段。
- 不改变 `run_gu_induction`、`active_gu_rules_for_concept`、`misconception_active`、grader prompt 注入、调度、MRT、镜像报告、trust panel、HTTP/MCP 默认输出。
- 不引入 LLM、人格、学习风格、临床标签或任何不可证伪标签。
- 不新增域特定逻辑；不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不把 shadow 统计用于生产行为决策；未过门结果只能表述为“证据不足/假设”。

## 验收命令

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p06i_gu_hierarchical_beta_shadow
cargo test -p polaris-core parameter_registry_contains_gu_prior_shadow_gates
cargo test --workspace
git diff --check
```

## 测试要求

- 空库或无 G_u 规则返回 `no_data`，且无写入副作用。
- 无同 pattern / 邻域历史证据时，hierarchical prior 必须退化为 `Beta(1,1)`，指标与 flat baseline 一致。
- 有同 pattern / 邻域历史证据时，hierarchical prior 的 pseudo-count 被 `gu_prior.max_prior_strength` 上限约束，输出稳定可复现。
- holdout 样本不足时返回 `insufficient_data` / validation skipped。
- holdout 样本足够时输出 flat 与 hierarchical 两套 logloss / Brier / accuracy，并给出 `passed` 只读判断。
- 运行 shadow summary 不得修改 `gu_rules`、`attempts`、`edges`、`mastery_states`。

## 回滚方式

删除本票新增模块、测试、Engine facade 和文档补充；恢复 `QUEUE.md` 中 P06I 状态即可。本票不改 schema，不需要数据迁移回滚。

## 本轮开工记录（2026-06-17）

- 当前状态：P06H 已提交（`2c483c3`），QUEUE 无未完成正式票。
- 既有脏文件：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/visuals/` 等与本票无关，不纳入本票。
- 子 agent 研究结论：产品价值审查与实现风险审查均推荐先做 G_u 层级 Beta 超先验，而不是 FSRS 个人参数拟合；原因是 G_u 更直接服务“为什么错、补哪里”，且可压成只读 shadow gate，不改默认行为。
- 预计修改面：`crates/polaris-core/src/gu_prior.rs`、`crates/polaris-core/src/lib.rs`、`crates/polaris-core/src/engine.rs`、`crates/polaris-core/tests/p06i_gu_hierarchical_beta_shadow.rs`、`crates/polaris-core/src/config.rs`、`docs/DATA_MODEL.md`、`docs/PARAMETERS.md`、`docs/tickets/QUEUE.md`。

## 交付记录（2026-06-17）

### 变更清单

- 新增 `crates/polaris-core/src/gu_prior.rs`：只读计算 G_u flat `Beta(1,1)` vs hierarchical shadow prior，输出规则级和聚合级 sequential predictive logloss / Brier / accuracy。
- 新增 `Engine::gu_prior_shadow()` facade，供后续 CLI/HTTP/MCP 票复用；未接入默认产品输出。
- 新增 `crates/polaris-core/tests/p06i_gu_hierarchical_beta_shadow.rs`：覆盖空库、退化为 flat、bounded pseudo-count、holdout 不足、未来 rule/attempt/grade/edge/concept 泄漏防线、holdout 窗口边界、只读 `total_changes()`。
- 注册并文档化 `gu_prior.min_shadow_rules`、`gu_prior.min_holdout_attempts`、`gu_prior.max_prior_strength` 三个 A 类 Manual shadow gate 参数。
- 更新 `docs/DATA_MODEL.md`、`docs/PARAMETERS.md`、`docs/tickets/QUEUE.md`。

### 子 agent 审查处理

- 产品与实现研究 agent 均建议 P06I 优先于 FSRS 个人参数拟合：G_u 更贴近“为什么错、补哪里”，且能压成只读 shadow gate。
- 规格审查发现 `graded_at` 与未来图谱结构泄漏风险；已增加 `COALESCE(graded_at, created_at) < cutoff`、`edges.created_at < cutoff`、`concepts.created_at < cutoff` 过滤，并补回归测试。
- 质量审查建议补 holdout 下界、只读写入计数；已补 `created_at == last_seen` 不进 holdout 与 `total_changes()` 前后不变。
- 最终窄复审确认 same-pattern future concept 泄漏已闭合；无必须修复项。

### 验收输出

```powershell
> cargo fmt --check
# exit 0，无输出
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 11s
```

说明：默认 `target/` 在 Windows 上出现 `.rmeta` 写入拒绝访问；按既有处理方式使用临时 `CARGO_TARGET_DIR` 复跑 clippy，代码诊断全绿。

```powershell
> cargo test -p polaris-core --test p06i_gu_hierarchical_beta_shadow
running 7 tests
test empty_database_returns_no_data_without_writes ... ok
test holdout_respects_gu_window_days_inclusive_upper_bound ... ok
test insufficient_holdout_skips_validation_without_claiming_success ... ok
test no_source_evidence_degenerates_to_flat_beta_prior ... ok
test same_pattern_source_uses_only_rules_known_before_target_holdout ... ok
test source_evidence_builds_bounded_hierarchical_prior_without_mutating_gu_rules ... ok
test source_evidence_excludes_future_grades_and_future_edges ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

```powershell
> cargo test -p polaris-core parameter_registry_contains_gu_prior_shadow_gates
test config::tests::parameter_registry_contains_gu_prior_shadow_gates ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 75 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core params_doc_keys_match_registry
test config::tests::params_doc_keys_match_registry ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 75 filtered out; finished in 0.00s
```

```powershell
> cargo test --workspace
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
...
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Doc-tests polaris_core
# exit 0
```

```powershell
> git diff --check
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/config.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/DATA_MODEL.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/PARAMETERS.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
# exit 0，无 whitespace error
```
