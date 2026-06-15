# P06F 校准后验化（Calibration Posterior）

状态：已完成

服务主命题环节：验证真懂 → 定位模糊

## 背景

`DATA_MODEL.md` §9 当前把幻影相判定写成硬阈值：`n>=2 ∧ calib_gap>=0.25 ∧ p<0.6`。这保留了 P01 的 EWMA 点估计，但 `docs/ENHANCEMENT_ROADMAP.md` 已把「校准后验化」列为数学深化候选：`calib_gap` 从单点估计升级为 Beta-Binomial 后验，幻影判据从硬阈值变为 `P(高估|数据)>τ`。

P03I 的镜像报告已经实现了 ln-gamma、正则不完全 Beta 与 `P(X>0.5)` 数学，并用近窗高估次数为幻影断言打置信度。本票把这条数学路径提到内核可复用模块，让相图判定和镜像报告共用同一校准证据摘要。

## 范围

1. 新增校准后验模块：
   - 新建 `crates/polaris-core/src/calibration.rs`。
   - 提供 `CalibrationPosterior { overestimates, total, alpha, beta, probability_over_half }`。
   - 提供 `posterior_from_counts(overestimates, total)` 和 `calibration_samples(conn, concept_id, limit)`。
   - 复用 P03I 的 Beta 数学，避免 `report.rs` 私有实现与相图重复。

2. 相图幻影判定接入后验门：
   - `PhaseInput` 增加 `calibration_overestimates`、`calibration_sample_count`、`calibration_probability_over_half`。
   - `PhaseParams` 增加 `phantom_confidence`，从 meta 读取 `calib.phantom_confidence`。
   - `determine_phase` 的 Phantom 判定必须同时满足：
     - `attempt_count >= calib.phantom_n`
     - `calib_gap >= calib.phantom_gap`
     - `p_known < calib.phantom_p`
     - `calibration_sample_count >= calib.phantom_n`
     - `calibration_probability_over_half >= calib.phantom_confidence`

3. 镜像报告复用同一后验摘要：
   - `calibration_phantom_candidates` 改用 `calibration_samples` 与 `posterior_from_counts`。
   - 文案保留「近 N 次作答中 K 次高估」，`stats` 增加 `probability_over_half`。

4. 参数登记：
   - 新增 `calib.phantom_confidence = 0.60`。
   - 参数分类：A 类，边界 `[0.50,0.95]`，调优途径 `Manual`。这是相图验证门，不允许夜间自调优自动抬降格线。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03e_phase
cargo test -p polaris-core --test p03i_mirror_report
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- 当 EWMA 硬阈值满足、但近窗高估后验概率低于 `calib.phantom_confidence` 时，不得进入 Phantom。
- 当 EWMA 硬阈值满足、近窗高估后验概率达到门槛时，进入 Phantom。
- 镜像报告的 calibration phantom 置信度与相图使用同一后验函数。
- `calib.phantom_confidence` 在注册表中是 A 类参数。

## 禁区

- 不改变 BKT、FSRS、MIRT、HMM 或调度权重。
- 不改变非 Phantom 相的判据。
- 不实现路线图里其他数学深化候选（逆方差融合、G_u 层级先验、相变动力学、θ AdaGrad、FSRS 个人参数拟合）。
- 不修改冻结参考仓库。

## 本轮范围（2026-06-15）

- 当前状态：QUEUE 显示 P06E 已验收，无 In Progress 票；按 Backlog 数学深化候选认领 P06F。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`。本票不得回退这些改动。
- 预计修改面：`calibration.rs`、`phase.rs`、`engine.rs`、`report.rs`、`config.rs`、`lib.rs`、`p03e_phase.rs`、`p03i_mirror_report.rs`、`QUEUE.md` 和本票文件。

## 交付记录（2026-06-15）

### 变更清单

- 新增 `crates/polaris-core/src/calibration.rs`：
  - `CalibrationSample`、`CalibrationPosterior`、`calibration_samples`、`posterior_from_samples`、`posterior_from_counts`。
  - 从 `report.rs` 迁出 Beta 数学：`ln_gamma`、`regularized_incomplete_beta`、`prob_beta_greater_half`、`prob_beta_greater`。
  - 增加后验计数与 Beta 已知值单测。
- `crates/polaris-core/src/phase.rs`：
  - `PhaseInput` 增加 `calibration_overestimates`、`calibration_sample_count`、`calibration_probability_over_half`。
  - `PhaseParams` 增加 `phantom_confidence`。
  - Phantom 判定保留原 EWMA/p_known/n 门，并新增高估后验概率门。
- `crates/polaris-core/src/engine.rs`：
  - replay 派生 phase 时，从最近校准样本计算后验摘要并填入 `PhaseInput`。
- `crates/polaris-core/src/report.rs`：
  - `calibration_phantom_candidates` 复用共享校准摘要。
  - `stats` 增加 `alpha`、`beta`、`probability_over_half`。
- `crates/polaris-core/src/breeding.rs`：
  - 改为直接从 `calibration` 模块导入 `prob_beta_greater`，不再经由 report 模块。
- `crates/polaris-core/src/config.rs`：
  - 新增 `calib.phantom_confidence = 0.60`，A 类，`Manual`，边界 `[0.50,0.95]`。
- 测试：
  - `p03e_phase.rs` 从 15 个扩展到 17 个，覆盖后验门纯函数与真实 replay 路径。
  - `p03i_mirror_report.rs` 断言镜像报告暴露并使用同一 `probability_over_half`。
- 文档：
  - 新增本票文件。
  - 新增执行计划 `docs/superpowers/plans/2026-06-15-p06f-calibration-posterior.md`。
  - `QUEUE.md` 标记 P06F 完成。

### TDD 红灯记录

```text
cargo test -p polaris-core --test p03e_phase phantom_requires_posterior_overestimate_gate
error[E0560]: struct `PhaseInput` has no field named `calibration_overestimates`
error[E0560]: struct `PhaseInput` has no field named `calibration_sample_count`
error[E0560]: struct `PhaseInput` has no field named `calibration_probability_over_half`
error[E0560]: struct `PhaseParams` has no field named `phantom_confidence`
exit 101
```

```text
cargo test -p polaris-core --test p03i_mirror_report calibration_phantom_assertion_carries_attempt_evidence_and_confidence
test calibration_phantom_assertion_carries_attempt_evidence_and_confidence ... FAILED
called `Option::unwrap()` on a `None` value
exit 101
```

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.90s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 29 passed
polaris-core unit: 70 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 5 passed
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
cargo test -p polaris-core --test p03e_phase
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p03i_mirror_report
running 14 tests
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- `determine_phase` 继续保持纯函数；数据库读取只发生在 Engine replay 层。
- `calib.phantom_confidence` 设为 A 类手动门槛，因为它改变相图验证格线，不允许夜间自调优自动修改。
- 共享 Beta 数学后，镜像报告、相图和教法育种不再通过 report 模块间接复用概率函数。
- 后验门是新增条件，不替代 `calib_gap`、`p_known` 和 `attempt_count` 原判据；这保证 P06F 只收紧 Phantom 证据门，不影响其他相。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/breeding.rs crates/polaris-core/src/config.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs crates/polaris-core/src/phase.rs crates/polaris-core/src/report.rs crates/polaris-core/tests/p03e_phase.rs crates/polaris-core/tests/p03i_mirror_report.rs docs/tickets/QUEUE.md
Remove-Item crates/polaris-core/src/calibration.rs, docs/tickets/TICKET_P06F_CALIBRATION_POSTERIOR.md, docs/superpowers/plans/2026-06-15-p06f-calibration-posterior.md
```

提交后：

```powershell
git revert <P06F-commit-sha>
```
