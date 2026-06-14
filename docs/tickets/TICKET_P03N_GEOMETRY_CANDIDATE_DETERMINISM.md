# P03N 几何候选池确定性

状态：已完成

服务主命题：全环节（验证稳定性）

## 背景

P05A 验收期间观察到：首次 `cargo test --workspace` 偶发在 `p03c_geometry::geometry_candidates_use_hnsw_and_combined_scores` 缺少 `schema:raii` 候选，进而导致同文件后续测试因 `ENV_LOCK` PoisonError 连锁失败；单跑 `cargo test -p polaris-core --test p03c_geometry` 和重跑 workspace 可通过。

当前 `geometry_candidates` 将 HNSW 搜索数量直接设为最终 `limit`，随后才计算 `cos_Q`、结构分、残差相关和综合分。这会让 embedding 上不是最近、但综合分应胜出的候选在排序前被截掉，也让小夹具对 HNSW 近邻返回细节过敏。

## 本轮范围

1. 将 HNSW 近邻搜索数量与最终返回 `limit` 解耦：
   - HNSW 候选池至少覆盖最终 `limit`。
   - 候选池上限受 `geometry.ef_search` 与实际 embedding 数量约束。
   - 综合打分后再按 `assoc DESC, target ASC` 截断最终 `limit`。
2. 对 HNSW under-return 增加确定性补齐，避免小样本或近似搜索返回不足导致候选缺失。
3. 增加确定性回归测试：embedding 更近的弱候选不能在综合分强候选被评分前挤掉它。
4. 保持 P03C 禁区：
   - 不用 exact scan 冒充几何候选主路径。
   - 不引入持久化索引。
   - 不改变 P03C 评分公式与 `maps_to` 结构门。

## 验收

必须通过：

```powershell
cargo test -p polaris-core --test p03c_geometry
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不改 embedding provider 协议，不联网。
- 不改 `assoc` / `discover` 公式。
- 不改 `graph::upsert_maps_to_candidate` 与结构门。
- 不处理 `.gitignore`、`.cursor/`、`docs/visuals/` 等票外改动。
- 不修改 frozen 参考仓库。

## 当前状态

- 已确认 P03M 已提交。
- 当前工作区仍有票外旧改动：`.gitignore`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`。
- 已定位可疑点：`geometry_candidates` 用最终 `limit` 调 `hnsw_neighbor_indices`，综合评分发生在候选截断之后。

## 交付记录

### 变更清单

- `geometry_candidates` 将 HNSW 搜索池与最终返回 `limit` 解耦：
  - 候选池大小为 `min(items.len()-1, max(limit, geometry.ef_search))`。
  - 对候选池计算 `cos_E`、`cos_Q`、`struct`、`coh`、`assoc`、`discover`。
  - 综合排序后再截断最终 `limit`。
- `hnsw_neighbor_indices` 增加确定性补齐：
  - HNSW 返回不足时，按 `geometry_items` 的稳定 `id ASC` 顺序补足候选池。
  - 主路径仍先构建并查询 `hnsw_rs` 内存索引。
- 新增 `complete_neighbor_indices` 私有 helper 与单元测试：
  - 去重、排除 source、过滤越界索引。
  - HNSW 返回不足时按稳定顺序补齐到目标候选池大小。
- 新增 `geometry_candidates_rank_combined_score_after_hnsw_overfetch` 回归测试：
  - 构造多个 embedding 更近但综合分弱的近邻。
  - `limit=1` 时仍应返回综合分更强的 `schema:raii`。
- `QUEUE.md` 将 P05A 验收观察转为正式票 P03N。

### 根因记录

- 原实现把 HNSW 搜索数量直接设置为最终 `limit`。
- P03C 的最终排序依据是综合分 `assoc`，但综合分计算发生在 HNSW 候选截断之后。
- 因此 embedding 上更近的弱候选可在评分前挤掉 Q/结构/残差更强的候选，导致小样本 HNSW 夹具对近邻返回细节过敏。

### 红灯记录

```text
cargo test -p polaris-core --test p03c_geometry geometry_candidates_rank_combined_score_after_hnsw_overfetch -- --exact
running 1 test
geometry_candidates_rank_combined_score_after_hnsw_overfetch ... FAILED

assertion `left == right` failed
left: "schema:near_00"
right: "schema:raii"
exit 1
```

```text
cargo test -p polaris-core completes_neighbor_indices_without_source_or_duplicates -- --exact
error[E0425]: cannot find function `complete_neighbor_indices` in this scope
exit 1
```

### 验收输出

```text
cargo test -p polaris-core --test p03c_geometry geometry_candidates_rank_combined_score_after_hnsw_overfetch -- --exact
1 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-core completes_neighbor_indices_without_source_or_duplicates
geometry::tests::completes_neighbor_indices_without_source_or_duplicates ... ok
1 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-core --test p03c_geometry
8 passed; 0 failed
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
failed: Windows target 目录文件锁，报错为 target/debug/deps/libpolaris_core-*.rmeta 写入被拒绝（os error 5），并伴随 incremental 目录 GC warning。
exit 1
```

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 38.51s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 23 passed
polaris-core unit: 67 passed
integration suites: all passed, including p03c_geometry 8 passed and p03m_latent_dims 3 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

### 子 agent 审查

Poincare（`019ec508-6f95-7551-9413-ce941096a602`）只读审查结论：

- Critical：无。
- Important：无。
- Minor 1：建议补一个直接覆盖 HNSW under-return 确定性补齐分支的测试。已采纳：抽出 `complete_neighbor_indices` 并新增单元测试。
- Minor 2：工作区存在票外脏文件，提交时只应 stage P03N 文件。已按此提交边界执行。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/geometry.rs crates/polaris-core/tests/p03c_geometry.rs docs/tickets/QUEUE.md
git clean -f docs/tickets/TICKET_P03N_GEOMETRY_CANDIDATE_DETERMINISM.md
```

提交后：

```powershell
git revert <P03N-commit-sha>
```
