# P03A MIRT 潜因子层

状态：已完成并提交

服务主命题：定位模糊 → 针对性补缺

## 背景

P01 已有 BKT/校准/FSRS，P02 已有图谱与 MCP 门。P03A 启用抽象引擎的潜因子快通路：用共享 θ 向量和概念 q 载荷预测欠观测概念，并在 final score 到达时做小步在线更新。

本票只做 Tier 0 可计算部分。夜间巩固、Q 重拟合、残差聚类、留出集回滚门归 P03B。

## 范围

1. 新增 MIRT/潜因子核心模块：
   - f32 小端 BLOB 编解码。
   - 稳定 sigmoid/logit clamp。
   - `p_hat = σ(q_c·θ − b_c − d_t)`。
   - `θ ← θ + η·(y − p_hat)·q_c`，逐元素帽 `|Δθ_k| ≤ step_cap`。
   - `p_known_fused = λ·BKT + (1−λ)·p_hat`，`λ = n_c/(n_c + mirt.fuse_n0)`。
2. pack 初始化时为概念写入 q：
   - 优先保留 pack 已有 q（未来 LLM/q0 可写入）。
   - 当前无 LLM/q 元数据时，按设计降级为 deterministic one-hot track 维。
3. 初始化 `theta(id=1)`：
   - `vec` 为 K 维零向量。
   - `version` 从 1 起。
4. final score 到达时：
   - 用 final score 更新 θ。
   - 每条 graded attempt 写 `attempts.theta_version = theta.version`。
5. 对外可读融合状态：
   - `Engine::latent_prediction(concept_id, task_type)`。
   - `Engine::fused_p_known(concept_id, task_type)`。

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

## 禁区

- 不做夜间巩固、残差聚类、Q 重拟合、维度合并或 holdout 回滚。
- 不引入嵌入/HNSW、HMM、hazard、MRT、HTTP/UI。
- 不让外部 AI 直接写 θ、q 或 mastery；LLM 初始化 Q 的在线调用不进入同步路径。
- 不修改冻结参考仓库。

## 交付记录

### 变更清单

- 新增 `mirt` 核心模块：
  - f32 小端向量 BLOB 编解码。
  - MIRT 参数读取。
  - `p_hat = σ(q·θ − b − d_t)`。
  - final score 到达后的 θ 小步更新与逐元素步长帽。
  - BKT-MIRT 融合：`λ = n/(n+n0)`。
- `init_pack` 为概念初始化 q，并确保 `theta(id=1)` 存在。
- final-score 成功路径更新 θ：
  - `Engine::apply_final_score`。
  - `Engine::submit` 中非 degraded grade。
  - `grade_pending` 成功处理 queued grade。
- 每条成功 graded attempt 写入 `attempts.theta_version = theta.version`。
- 对外新增：
  - `Engine::latent_prediction`。
  - `Engine::fused_p_known`。
- 兼容旧 `free_explain` task_type，映射到 DATA_MODEL 已登记的 `free_produce` MIRT 难度。
- Backlog 记录多 pack/多 track 前需补 `latent.dims` 或 pack/track→维度映射。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.75s
exit 0
```

```text
cargo test --workspace
polaris-cli: 9 passed
polaris-core unit: 43 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 5 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-core --test p03a_mirt
5 passed; 0 failed
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 子 agent 审查

审查 agent：Harvey（`019eb544-4e1c-7eb1-9b2c-ee7dbbdeedd0`）。

结论：

- Critical：无。
- Important：无。
- θ 更新只挂在 final 成功路径。
- `attempts.theta_version` 写当前 `theta.version`。
- 融合公式符合 `n/(n+n0)`。
- 未发现 stdout/CLI/MCP/HTTP/HNSW/HMM/MRT 等票外运行时代码变更。

审查提出的覆盖度建议已处理：

- 增加 degraded/provisional 不更新 θ、不写 `theta_version` 的回归测试。

审查提出的未来多 track 风险已记录到 `QUEUE.md` Backlog。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/src/mirt.rs crates/polaris-core/tests/p03a_mirt.rs docs/superpowers/plans/2026-06-11-p03a-mirt-latent.md docs/tickets/TICKET_P03A_MIRT_LATENT.md
```

提交后：

```powershell
git revert <P03A-commit-sha>
```
