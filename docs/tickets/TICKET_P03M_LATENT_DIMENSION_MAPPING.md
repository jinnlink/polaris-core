# P03M 多 pack latent 维度映射

状态：已完成并通过验收

服务主命题：定位模糊 → 针对性补缺

## 背景

P03A 已落地 MIRT 潜因子层：`init_pack` 在没有 LLM/q 元数据时为概念写入 deterministic one-hot q。当前降级路径固定使用 `q[0]=1.0`，在单 Rust pack 下可用，但在多 pack/多 track 场景会让不同领域的概念共享同一潜因子，削弱跨域定位能力。

本票只把 P03A 审查后续转为最小可验收补丁：没有 track 字段时，以 pack id 作为降级维度标签，写入 `meta('latent.dims')`，并据此生成 pack 专属 one-hot q。

## 本轮范围

1. 为 Q 降级初始化新增稳定维度映射：
   - `meta('latent.dims')` 保存 JSON 字符串数组。
   - 缺失时从空数组开始。
   - pack 降级标签为 `pack:<pack_id>`。
   - 已存在标签复用原索引。
   - 新 pack 追加到首个可用维度。
2. `Engine::init_pack` 使用 pack 专属 q 初始化该 pack 新概念。
3. 保留已有概念 q：
   - 重复 `init_pack` 不覆盖已有 q。
   - 旧库里已有 q 的概念不因维度映射改变而漂移。
4. 当 `latent.k` 已无可用维度且遇到新 pack 时，返回明确错误，不静默复用错误维度。
5. 增加回归测试覆盖 Rust + Algorithms 双 pack 的 q 分离和 `latent.dims` 持久化。

## 验收

必须通过：

```powershell
cargo test -p polaris-core --test p03m_latent_dims
cargo test -p polaris-core --test p03a_mirt
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不新增 pack `track` 字段，不改课程接入协议。
- 不接入 LLM 生成 q，不做维度合并、重拟合或 holdout 评估。
- 不改 MIRT 公式、theta 更新公式、BKT-MIRT 融合公式。
- 不改 frozen 参考仓库 `C:\MyProject\Polaris` 与 `C:\MyProject\Learned`。
- 不顺手处理 P05A HNSW 测试偶发问题。

## 当前状态

- 已确认 QUEUE 无其他 In Progress 票。
- 已确认现有降级路径在 `mirt::initial_track_q` 中固定写 `q[0]=1.0`。
- 已确认当前 pack TOML 没有 track 字段，本票采用 pack id 作为降级映射粒度。

## 交付记录

### 变更清单

- 新增 `mirt::initial_pack_q_blob`：
  - 读取/初始化 `meta('latent.dims')`。
  - 使用 `pack:<pack_id>` 作为当前无 track 字段时的降级维度标签。
  - 已存在标签复用原索引，新标签追加到首个可用维度。
  - `latent.k` 满额或既有标签索引越界时返回 `InvalidParameter`。
- `Engine::init_pack` 在事务内按 pack 生成初始 q，并继续通过 SQL `COALESCE(existing q, initial_q)` 保留已有概念 q。
- 新增 `crates/polaris-core/tests/p03m_latent_dims.rs`：
  - Rust + Algorithms 双 pack 的 q 落在不同 one-hot 维度。
  - 重复 `init_pack` 不改变 `latent.dims` 或已有 q。
  - 既有概念 q 被手工写入后，重复初始化不覆盖。
  - `latent.k=1` 时第二个 pack 初始化失败，不静默复用维度。
  - 满额失败后 `latent.dims` 与 concepts 写入保持无副作用。
- `QUEUE.md` 将 P03A 审查后续转为正式票 P03M。

### 技术选择说明

- 当前 pack 协议没有 track 字段，本票不改协议；因此降级映射粒度选择 pack id。
- `latent.dims` 与 concepts 写入放在同一个 `init_pack` 事务内，避免 pack 初始化失败后留下半截映射。
- 不改变 P03A 的 MIRT/θ 数学，只改变无 q 元数据时的初始 q 选择。

### 红灯记录

```text
cargo test -p polaris-core --test p03m_latent_dims
running 3 tests
fallback_q_preserves_existing_concept_q_when_reinitializing_pack ... ok
fallback_q_rejects_new_pack_when_latent_dimensions_are_full ... FAILED
fallback_q_uses_distinct_pack_latent_dimensions ... FAILED

failures:
- latent.dims 不存在：QueryReturnedNoRows
- latent.k=1 时第二个 pack 静默复用维度
exit 1
```

### 验收输出

```text
cargo test -p polaris-core --test p03m_latent_dims
3 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-core --test p03a_mirt
5 passed; 0 failed
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
failed: Windows target 目录文件锁，报错为 target/debug/deps/*.rmeta 拒绝访问（os error 5），并伴随 incremental 目录 GC warning。
exit 1
```

```text
cargo clippy --target-dir "$env:TEMP\polaris-p03m-clippy" --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.51s
exit 0
```

补充子 agent 审查后最终复跑：

```text
cargo clippy --target-dir "$env:TEMP\polaris-p03m-clippy-final" --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.84s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 23 passed
polaris-core unit: 66 passed
integration/doc suites: all passed, including p03m_latent_dims 3 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

### 子 agent 审查

- 审查 agent：Epicurus（`019ec4fd-02dd-74d3-b35f-eee00860a9b9`）。
- 结论：未发现 Critical / Important。
- Minor 1：提醒提交时排除 `.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md` 等票外改动。本票提交将只 stage P03M 相关文件。
- Minor 2：建议满额失败路径补事务无副作用断言。已在 `p03m_latent_dims.rs` 补充：失败后 `latent.dims == ["pack:rust"]` 且 algorithms 概念未写入。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/engine.rs crates/polaris-core/src/mirt.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/tests/p03m_latent_dims.rs docs/tickets/TICKET_P03M_LATENT_DIMENSION_MAPPING.md
```

提交后：

```powershell
git revert <P03M-commit-sha>
```
