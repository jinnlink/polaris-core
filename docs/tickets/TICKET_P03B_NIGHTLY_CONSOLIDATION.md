# P03B 夜间巩固 v1

状态：已完成并提交

服务主命题：定位模糊 → 针对性补缺

## 背景

P03A 已启用 θ/q 潜因子快通路。P03B 开始做慢通路：把 graded attempts 的残差折叠进 `residual_stats`，夜间快照 θ，寻找共塌候选簇，并用验证门决定是否可以进入系统默认行为。

本票的核心是“可审计 + 不过门不生效”。在当前没有 LLM 溯因与 q 重拟合成熟实现前，候选只能写入 `consolidation_runs`，默认拒绝，不得修改 q 或新增维度。

## 范围

1. 新增夜间巩固模块：
   - 计算 90 天窗口内 graded attempts 的 `residual = final_score - p_hat`。
   - 按周写入 `residual_stats(concept_id, week, mean_resid, n)`。
   - 依赖 `attempts.theta_version` 与 `theta_history`；若历史缺失且版本等于当前 theta，则允许用当前 theta。
2. 新增 θ 夜间快照：
   - 将当前 `theta.version` 写入 `theta_history`。
   - 对当前 θ 应用 `mirt.shrink`。
   - `theta.version += 1`。
3. 候选簇检测：
   - 概念覆盖周数 <4 跳过。
   - 公共周 ≥4 且 Pearson 相关 ≥0.5 视为相关边。
   - 连通簇大小 ≥3 形成候选。
4. 审计门：
   - 写 `consolidation_runs`。
   - 当前无 LLM/q 重拟合试装时，`holdout_delta=0.0`，状态为 `rejected`。
   - 不改 q、不新增维度、不把候选进入产品默认行为。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03b_consolidation
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不调用 LLM，不生成已验证新维度/图式。
- 不做 Q 重拟合、维度合并、HNSW/嵌入、HMM、hazard、MRT、HTTP/UI。
- 候选不过 holdout 门时不得改 q、theta 维数或调度默认行为。
- 不修改冻结参考仓库。

## 交付记录

### 变更清单

- 新增 `consolidation` 夜间巩固模块：
  - 夜间快照当前 `theta.version` 到 `theta_history`。
  - 按 `mirt.shrink` 收缩当前 θ，并将 `theta.version` 加 1。
  - 计算 90 天窗口内 final graded attempts 的 `residual = final_score - p_hat`。
  - 按 ISO 周写入 `residual_stats(concept_id, week, mean_resid, n)`。
- `p_hat` 使用 `attempts.theta_version` 对应的 `theta_history`；仅当缺历史且版本等于当前 `theta.version` 时允许用当前 θ。
- 新增候选簇检测：
  - 覆盖周数 `<4` 的概念跳过。
  - 共同周 `>=4` 且 Pearson 相关 `>=0.5` 建边。
  - 连通簇大小 `>=3` 生成 `candidate_latent_dimension` proposal。
- 审计门在 P03B 阶段固定拒收：
  - `holdout_delta=0.0`。
  - `accepted=false`。
  - `consolidation_runs.status='rejected'`。
  - 即使 `consol.accept_margin=0.0` 也不会 accepted。
- 暴露 `Engine::run_nightly_consolidation()` 作为内核入口。
- 新增 P03B 集成测试与 ISO week-year 边界单测。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.18s
exit 0
```

```text
cargo test --workspace
polaris-cli: 9 passed
polaris-core unit: 45 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 5 passed
p03b_consolidation: 3 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-core --test p03b_consolidation
3 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-core consolidation::tests
2 passed; 0 failed
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 子 agent 审查

审查 agent：Kierkegaard（`019eb555-ac45-7333-9b10-e788583cc68b`）。

首轮发现并已修复：

- Important：当前无 trial 时仍可能因 `consol.accept_margin=0.0` 误 accepted。
  - 修复：P03B 阶段固定 `accepted=false/status='rejected'`。
  - 回归：候选簇测试将 `consol.accept_margin` 设为 `0.0`，仍断言 rejected。
- Important：`strftime('%Y-W%W')` 不是 ISO week-year。
  - 修复：改为 Rust 本地 `iso_week_label` 计算 ISO 周。
  - 回归：覆盖 `2024-12-30 -> 2025-W01`、`2027-01-01 -> 2026-W53`。

复审结论：

- Critical：无。
- Important：无。
- Minor：无。
- 可以提交。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/src/consolidation.rs crates/polaris-core/tests/p03b_consolidation.rs docs/superpowers/plans/2026-06-11-p03b-nightly-consolidation.md docs/tickets/TICKET_P03B_NIGHTLY_CONSOLIDATION.md
```

提交后：

```powershell
git revert <P03B-commit-sha>
```
