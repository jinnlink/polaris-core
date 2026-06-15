# P06F 校准后验化实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 把 Phantom 相和镜像报告的「高估」判断统一到 Beta-Binomial 后验概率门。

**架构：** 新增 `calibration.rs` 承担校准样本读取与 Beta 后验摘要。`phase.rs` 继续保持纯函数，只接收 replay 计算好的后验字段；`report.rs` 复用同一摘要生成断言置信度。

**技术栈：** Rust、rusqlite、serde、proptest、cargo test。

---

## 文件结构

- 创建：`crates/polaris-core/src/calibration.rs`
  - 职责：校准样本读取、Beta 数学、后验概率摘要。
- 修改：`crates/polaris-core/src/lib.rs`
  - 职责：导出 `calibration` 模块。
- 修改：`crates/polaris-core/src/phase.rs`
  - 职责：`PhaseInput` 和 `PhaseParams` 增加后验字段；Phantom 判定读取概率门。
- 修改：`crates/polaris-core/src/engine.rs`
  - 职责：replay 时填充每个概念的校准后验摘要。
- 修改：`crates/polaris-core/src/report.rs`
  - 职责：镜像报告复用 `calibration.rs` 后验摘要，保留现有红线过滤。
- 修改：`crates/polaris-core/src/config.rs`
  - 职责：登记 `calib.phantom_confidence`。
- 修改：`crates/polaris-core/tests/p03e_phase.rs`
  - 职责：新增后验门红灯/绿灯测试并更新夹具。
- 修改：`crates/polaris-core/tests/p03i_mirror_report.rs`
  - 职责：断言报告置信度来自共享后验摘要。
- 修改：`docs/tickets/QUEUE.md`
  - 职责：认领 P06F。

## 任务

### 任务 1：红灯测试

- [ ] 在 `p03e_phase.rs` 中给 `PhaseInput` 夹具增加校准后验字段。
- [ ] 新增 `phantom_requires_posterior_overestimate_gate`：

```rust
let mut input = base_phase_input();
input.p_known = 0.55;
input.calib_gap = 0.30;
input.attempt_count = 4;
input.calibration_sample_count = 4;
input.calibration_overestimates = 2;
input.calibration_probability_over_half = 0.50;
assert_eq!(determine_phase(&input, &phase_params()), Phase::Undetermined);

input.calibration_overestimates = 4;
input.calibration_probability_over_half = 0.9375;
assert_eq!(determine_phase(&input, &phase_params()), Phase::Phantom);
```

- [ ] 运行：

```powershell
cargo test -p polaris-core --test p03e_phase phantom_requires_posterior_overestimate_gate
```

预期：编译失败或测试失败，因为字段和参数尚未实现。

### 任务 2：最小实现

- [ ] 新增 `calibration.rs`：

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationPosterior {
    pub overestimates: usize,
    pub total: usize,
    pub alpha: f64,
    pub beta: f64,
    pub probability_over_half: f64,
}
```

- [ ] 将 `ln_gamma`、`regularized_incomplete_beta`、`prob_beta_greater_half` 从 `report.rs` 移到 `calibration.rs`，保持函数名和单测。
- [ ] 在 `phase.rs` 中增加 `phantom_confidence` 与校准后验字段。
- [ ] 在 `engine.rs` 的 `phase_input` 中读取 `calibration_samples(conn, concept_id, 12)` 并填充后验字段。
- [ ] 在 `report.rs` 中复用 `calibration_samples` 和 `posterior_from_samples`。

### 任务 3：参数与报告测试

- [ ] 在 `config.rs` 登记 `calib.phantom_confidence` 为 A 类 `Manual`。
- [ ] 在 `p03e_phase.rs` 增加 `engine_uses_posterior_gate_for_phantom`，覆盖真实 replay 路径。
- [ ] 在 `p03i_mirror_report.rs` 增加或更新断言，检查 `stats["probability_over_half"] == assertion.confidence`。
- [ ] 运行：

```powershell
cargo test -p polaris-core --test p03e_phase
cargo test -p polaris-core --test p03i_mirror_report
```

预期：全部通过。

### 任务 4：验收与交付记录

- [ ] 运行完整验收：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03e_phase
cargo test -p polaris-core --test p03i_mirror_report
git diff --check
```

- [ ] 把真实输出写入 `docs/tickets/TICKET_P06F_CALIBRATION_POSTERIOR.md` 票尾。
- [ ] 将 `docs/tickets/QUEUE.md` 中 P06F 标为已实现并通过验收。
