# TICKET P06H: 相变动力学 shadow gate

状态：已通过验收（2026-06-17）

服务环节：定位模糊 -> 针对性补缺。该票只增强 F2 相图的可验证统计，不改变当前相判据、调度、报告或默认产品行为。

## 背景

P03E 已给出每个概念的静态相判定，P07A/P07D 已把相图接入产品语义与行动闭环。当前缺口是：系统能记录 `phase_transition` 事件，但还没有把“相如何迁移”整理成可审计的 Tier 0 shadow 统计。

增强路线图把“相变动力学”列为数学深化候选：用相轨迹估计 Markov 转移矩阵，并输出脆弱到活跃/迁移/生成的期望证据步数；在未过验证门前只作为 shadow 输出。

## 本轮范围

1. 新增只读相变动力学模块，数据源仅限 `behavior_events` 中 `type='phase_transition'` 的事件。
2. 解析 payload 中的 `from` / `to`，使用现有 `Phase::parse` 与 `Phase::ALL`，未知相或 malformed JSON 只计入 ignored，不报错。
3. 基于 `Phase::ALL` 输出 8x8 计数矩阵与行归一化概率矩阵，包括 `undetermined`。不沿用路线图里旧的“7 相”表述。
4. 输出 shadow 状态：`no_data`、`insufficient_data`、`shadow_ready`。样本不足时不得制造结论。
5. 输出到 `transfer|generation` 目标集合的期望相变步数；不可达、吸收失败或矩阵奇异时返回 `None`。
6. 增加 holdout 验证摘要：相轨迹预测必须和静态基线分开记录；样本不足时显式 skipped。
7. 暴露一个 Engine 只读 facade，供后续 CLI/HTTP/MCP 票复用。本票不接入用户默认界面。
8. 更新 `DATA_MODEL.md`，说明该票的 shadow 语义、数据源和不改变产品行为。

## 禁区

- 不改 schema / DDL / `PRAGMA user_version`。
- 不改 `determine_phase`、`mastery_states.phase` 语义、调度、MRT、镜像报告、trust panel、HTTP/MCP 默认输出。
- 不引入学习风格、人格、MBTI 或任何不可证伪标签。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不把 shadow 统计用于生产行为决策。

## 验收命令

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p06h_phase_dynamics
cargo test --workspace
git diff --check
```

## 测试要求

- 空库返回 `no_data`，矩阵稳定且无 panic。
- 正常事件能得到确定的 8x8 counts / probabilities。
- malformed payload、未知相名被 ignored。
- `phantom -> settling -> transfer` 这类可达路径返回有限期望步数。
- 不可达目标返回 `None`。
- holdout 样本不足时 skipped；样本足够时输出稳定的基线与 Markov 指标。

## 回滚方式

删除本票新增模块、测试、Engine facade 和 `DATA_MODEL.md` 补充段；恢复 `QUEUE.md` 中 P06H 状态即可。本票不改 schema，不需要数据迁移回滚。

## 本轮开工记录（2026-06-17）

- 当前状态：P03O 已提交（`8e1f264`），QUEUE 无 In Progress。
- 既有脏文件：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/visuals/` 等与本票无关，不纳入本票。
- 子 agent 研究结论：相变动力学比 FSRS 个人参数拟合、G_u 层级先验更适合作为下一张票；风险低、边界清晰、可作为 shadow gate。
- 预计修改面：`crates/polaris-core/src/phase_dynamics.rs`、`crates/polaris-core/src/lib.rs`、`crates/polaris-core/src/engine.rs`、`crates/polaris-core/tests/p06h_phase_dynamics.rs`、`docs/DATA_MODEL.md`、`docs/tickets/QUEUE.md`。

## 交付记录（2026-06-17）

### 变更清单

- 新增 `crates/polaris-core/src/phase_dynamics.rs`：从 `behavior_events(type='phase_transition')` 只读构建 8x8 相迁移 counts/probabilities、ignored 计数、shadow 状态、目标相期望步数与 holdout 验证摘要。
- 新增 `Engine::phase_dynamics()` facade，供后续 CLI/HTTP/MCP 票复用；未接入默认产品输出。
- 新增 `crates/polaris-core/tests/p06h_phase_dynamics.rs`，覆盖空库、malformed/unknown ignored、8x8 矩阵、可达/不可达期望步数、holdout 验证、Engine facade 与 JSON snake_case。
- 新增参数注册与文档：`phase_dynamics.min_shadow_ready_transitions`、`phase_dynamics.min_validation_transitions`、`phase_dynamics.holdout_frac`，均为 A 类、Manual 路径并按 registry bounds clamp。
- 更新 `docs/DATA_MODEL.md`、`docs/PARAMETERS.md`、`docs/tickets/QUEUE.md`，记录 shadow 语义、验收边界和本票状态。

### 子 agent 审查处理

- 规格审查子 agent 指出“阈值不应硬编码”“JSON enum 需 snake_case”“Phase 字段应稳定序列化为字符串”。已改为参数注册 + bounds clamp，并为状态、验证状态、Phase 字段补测试；复审无必须修复项。
- 代码质量审查子 agent 指出“holdout 中训练集未见 from 时不应按静态基线计为 Markov 命中”。已改为 unseen-from 不计 Markov hit、logloss 使用 epsilon 惩罚，并补对应测试；复审无必须修复项。

### 验收输出

```powershell
> cargo fmt --check
# exit 0，无输出
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.58s
```

```powershell
> cargo test -p polaris-core --test p06h_phase_dynamics
running 11 tests
test empty_returns_no_data_and_stable_zero_matrix ... ok
test deterministic_counts_probabilities_ignored_payloads_and_engine_facade ... ok
test expected_steps_finite_for_reachable_chain ... ok
test expected_steps_none_for_target_reachable_but_non_target_absorbing_risk ... ok
test expected_steps_none_for_unreachable_loop ... ok
test holdout_validation_does_not_count_unseen_from_rows_as_markov_hits ... ok
test holdout_validation_markov_beats_static_on_deterministic_path ... ok
test insufficient_data_json_uses_snake_case_and_phase_strings ... ok
test min_shadow_ready_can_be_overridden_from_meta ... ok
test validation_params_can_be_overridden_from_meta ... ok
test validation_params_upper_bounds_are_clamped ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```powershell
> cargo test -p polaris-core parameter_registry_contains_p06h_phase_dynamics_gates
test config::tests::parameter_registry_contains_p06h_phase_dynamics_gates ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 89 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core params_doc_keys_match_registry
test config::tests::params_doc_keys_match_registry ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 89 filtered out; finished in 0.00s
```

```powershell
> cargo test --workspace
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
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
