# P06G theta AdaGrad 步长

状态：已完成

服务主命题环节：验证真懂 → 定位模糊

## 背景

`DATA_MODEL.md` §4 当前把 MIRT 在线更新写成固定步长：

`theta <- theta + 0.05 * (y - p_hat) * q_c`

这保留了 P03A 的最小可跑版本，但数学深化候选已经把「theta 步长自适应」列为 S 级增强：固定 `mirt.eta` 改为 AdaGrad 式按维累积二阶矩，仍保持在线梯度和 `mirt.step_cap` 逐元素帽。目标是在不引入新模型的前提下，让频繁被同类证据更新的维度自动降步长，降低震荡。

## 范围

1. theta 状态增加二阶矩累积：
   - `theta` 表新增 `g2 BLOB`，长度与 `vec` 相同。
   - 新库初始化时写入零向量；旧库迁移时补列并由 `ensure_theta` 回填零向量。

2. MIRT 在线更新接入 AdaGrad：
   - 梯度仍为 `gradient_k = (y - p_hat) * q_k`。
   - 每维累积 `g2_k += gradient_k^2`。
   - 更新量改为 `delta_k = eta * gradient_k / sqrt(g2_k + epsilon)`。
   - `delta_k` 继续按 `[-mirt.step_cap, +mirt.step_cap]` 截断。
   - final score 缺失的降级提交仍不得更新 theta 或 g2。

3. 参数登记：
   - 新增 `mirt.adagrad_epsilon = 1e-8`。
   - 参数分类：B 类，边界 `[1e-12,1e-3]`，调优途径 `Replay`。
   - `mirt.eta` 继续作为初始全局步长，不改变 C 类 θ 初始化语义。

4. 文档同步：
   - 更新 `DATA_MODEL.md` §4 与 §10 的 theta 更新公式和参数登记。
   - 更新 `QUEUE.md`，确保只有 P06G 为 In Progress。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03a_mirt
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- `theta.g2` 在新库和迁移库中均存在，并与 `theta.vec` 等长。
- 首次强梯度更新可触达 `mirt.step_cap`，重复同维更新时后续步长因 g2 累积而变小。
- `theta_version` 语义保持不变：attempt 记录使用更新前的 theta version。
- 不改变 BKT、FSRS、BKT-MIRT 融合权、HMM、相图或调度权重。

## 禁区

- 不实现逆方差 BKT-MIRT 融合。
- 不实现 G_u 层级先验、相变动力学或 FSRS 个人参数拟合。
- 不改变 `theta_history` 的历史预测语义；历史仍只快照 theta vec。
- 不修改冻结参考仓库。

## 本轮范围（2026-06-15）

- 当前状态：P06F 已提交（`417e87c`），工作树剩余票外 `.gitignore`、`.cursor/`、`docs/visuals/` 等改动；本票不得回退这些改动。
- 预计修改面：`db.rs`、`mirt.rs`、`config.rs`、`p03a_mirt.rs`、`DATA_MODEL.md`、`QUEUE.md`、本票文件和执行计划。

## 交付记录（2026-06-15）

### 变更清单

- `crates/polaris-core/src/db.rs`：
  - `theta` 表新增 `g2 BLOB`。
  - 迁移流程通过 `ensure_column` 为旧库补 `g2` 列。
  - 增加旧库迁移测试，验证 `ensure_theta` 可回填等长零向量。
- `crates/polaris-core/src/mirt.rs`：
  - `MirtParams` 增加 `adagrad_epsilon`。
  - `ensure_theta` 初始化并回填 `g2`。
  - `update_theta_for_attempt` 改为每维 AdaGrad：累积 `gradient^2`，用 `eta / sqrt(g2 + epsilon)` 缩放，并继续应用 `step_cap`。
  - `theta_version` 记录语义保持不变。
- `crates/polaris-core/src/config.rs`：
  - 新增 `mirt.adagrad_epsilon = 1e-8`，B 类，`Replay`，边界 `[1e-12,1e-3]`。
- `crates/polaris-core/tests/p03a_mirt.rs`：
  - 增加 `theta.g2` 初始化测试。
  - 增加重复同维更新步长衰减测试。
- 文档：
  - `docs/DATA_MODEL.md` 同步 AdaGrad 公式与参数登记。
  - `docs/tickets/QUEUE.md` 标记 P06G 完成。
  - 新增执行计划 `docs/superpowers/plans/2026-06-15-p06g-theta-adagrad.md`。

### TDD 红灯记录

```text
cargo test -p polaris-core --test p03a_mirt init_pack_initializes_theta_adagrad_accumulator
test init_pack_initializes_theta_adagrad_accumulator ... FAILED
called `Result::unwrap()` on an `Err` value:
SqlInputError { msg: "no such column: g2", sql: "SELECT vec, g2 FROM theta WHERE id=1" }
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
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.29s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 29 passed
polaris-core unit: 71 passed
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
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- `g2` 放在 `theta` 表而不是 `theta_history`，因为它是在线优化器状态；历史预测只需要当时的 `theta.vec`。
- `g2` 夜间不随 `theta` shrink，保留 AdaGrad 的历史步长记忆，避免高频维度在每天重置后继续震荡。
- `mirt.eta` 保持全局初始步长含义；`mirt.step_cap` 仍是最终安全帽。
- 旧库迁移只补列，实际等长向量由 `ensure_theta` 回填，避免在 schema 迁移阶段依赖当前 `latent.k`。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/db.rs crates/polaris-core/src/mirt.rs crates/polaris-core/src/config.rs crates/polaris-core/tests/p03a_mirt.rs docs/DATA_MODEL.md docs/tickets/QUEUE.md
Remove-Item docs/tickets/TICKET_P06G_THETA_ADAGRAD.md, docs/superpowers/plans/2026-06-15-p06g-theta-adagrad.md
```

提交后：

```powershell
git revert <P06G-commit-sha>
```
