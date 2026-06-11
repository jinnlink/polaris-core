# P03A MIRT 潜因子层实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 启用 P03A 的潜因子快通路：概念 q、学习者 θ、MIRT 预测、final-score 在线更新与 BKT-MIRT 融合。

**架构：** `polaris-core::mirt` 承担数学与 SQLite 状态读写；`Engine` 在 pack 初始化和 final score 到达时调用它。夜间巩固、Q 重拟合、残差聚类和留出验证门不进入本票。

**技术栈：** Rust 2021、rusqlite、f32 little-endian BLOB、现有 meta 参数登记。

---

## 文件结构

- 创建 `crates/polaris-core/src/mirt.rs`：MIRT 参数、向量 BLOB、预测、θ 更新、融合。
- 修改 `crates/polaris-core/src/lib.rs`：导出 `mirt`。
- 修改 `crates/polaris-core/src/engine.rs`：pack 初始化 q/theta；final-score 成功后更新 θ；提供预测/融合读取 API。
- 创建 `crates/polaris-core/tests/p03a_mirt.rs`：P03A 集成测试。
- 修改 `docs/tickets/QUEUE.md`、`docs/tickets/TICKET_P03A_MIRT_LATENT.md`：状态与交付记录。

## 任务 1：红灯测试

- [ ] **步骤 1：写失败测试**

创建 `crates/polaris-core/tests/p03a_mirt.rs`，覆盖：

- pack 初始化后每个概念有 q BLOB，theta 有 K 维零向量与 version=1。
- `apply_final_score` 后 theta 发生更新，attempt 写 `theta_version=1`。
- `fused_p_known` 对 attempt_count=0 更接近 MIRT，对 attempt_count 增大更接近 BKT。

- [ ] **步骤 2：运行红灯**

运行：`cargo test -p polaris-core --test p03a_mirt`

预期：FAIL，缺少 `mirt` 模块与 Engine API。

## 任务 2：实现 MIRT 核心

- [ ] **步骤 1：实现向量 BLOB 与参数**

读取 `latent.k`、`latent.k_max`、`mirt.eta`、`mirt.step_cap`、`mirt.fuse_n0`、`mirt.d.<task_type>`。

- [ ] **步骤 2：实现预测与更新**

实现：

```rust
p_hat = sigmoid(dot(q, theta) - b_difficulty - d_t)
theta[k] += clamp(eta * (score - p_hat) * q[k], -step_cap, step_cap)
```

- [ ] **步骤 3：运行核心测试**

运行：`cargo test -p polaris-core --test p03a_mirt`

预期：进入 Engine 集成缺口。

## 任务 3：接入 Engine

- [ ] **步骤 1：pack 初始化**

`init_pack` 事务内为没有 q 的概念写 deterministic one-hot q；事务后确保 `theta` 存在。

- [ ] **步骤 2：final-score 路径**

在 `apply_final_score`、`submit` final 成功、`process_queued_attempts` final 成功后调用 MIRT 更新，并写 `attempts.theta_version`。

- [ ] **步骤 3：公开读取 API**

`Engine::latent_prediction(concept_id, task_type)` 与 `Engine::fused_p_known(concept_id, task_type)`。

- [ ] **步骤 4：运行绿灯**

运行：`cargo test -p polaris-core --test p03a_mirt`

预期：PASS。

## 任务 4：全量验收与审查

- [ ] **步骤 1：全量验收**

运行：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03a_mirt
git diff --check
```

- [ ] **步骤 2：子 agent 审查**

审查重点：

- θ 是否只在 final score 成功路径更新。
- provisional/degraded 是否没有污染 θ。
- 融合公式是否按 `n/(n+n0)`。
- 是否越界到 P03B/P03C。

- [ ] **步骤 3：修复问题并填写交付记录**

修复 Critical/Important 后重跑相关测试与全量验收。
