# P03C 几何候选层 v1

状态：已完成并提交

服务主命题：定位模糊 → 针对性补缺

## 背景

P03A/P03B 已有潜因子快通路与残差慢通路。P03C 激活几何层：把概念/图式嵌入存进本地库，用近邻检索快速提出“可能相似”的候选，再交给结构层裁决。几何只提议，不裁决，避免表面相似污染图谱。

本票只做 Tier 0/后台可复用的内核能力：embedding 刷新、瞬时 HNSW 候选检索、组合分数和结构门落 `maps_to` 候选。LLM 解释与验证门仍后置，不进入本票。

## 范围

1. 嵌入刷新：
   - 读取 `POLARIS_EMBED_BASE_URL` / `POLARIS_EMBED_MODEL` / `POLARIS_EMBED_API_KEY`。
   - 调用 OpenAI-compatible `/v1/embeddings`。
   - 返回向量必须单位化后以 f32 小端 BLOB 写入 `concepts.embedding`。
   - embedding 维度写入 `meta('embedding.dim')`；维度变化拒收。
   - 环境变量缺失时几何层整体停用，返回 disabled summary，不联网、不写库。
2. HNSW 候选检索：
   - 使用 `hnsw_rs` 或 `instant-distance` 构建内存索引。
   - 参数默认：`geometry.hnsw_m=16`、`geometry.ef_search=64`。
   - 索引从 SQLite 当前 embedding 重建；本票不做持久化索引文件。
3. 候选打分：
   - `cos_E`：embedding cosine。
   - `cos_Q`：q 向量 cosine，缺失时为 0。
   - `struct(a,b)`：复用 P02A typed 2-hop 结构分数。
   - `coh(a,b)`：复用 `residual_stats` 行相关，公共周不足为 0。
   - `assoc = 0.15·cos_E + 0.35·cos_Q + 0.25·struct + 0.25·coh`。
   - `discover = (0.35·cos_Q + 0.25·struct + 0.25·coh)·(1 − cos_E)`。
4. `maps_to` 候选：
   - 几何近邻只提供候选。
   - 只有 schema↔schema 且结构分数达到 `graph.struct_threshold` 时，才复用现有 `upsert_maps_to_candidate` 写 `maps_to` 边。
   - 写入的 `alignment_json` 仍必须标记 `requires_llm_verification=true`。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03c_geometry
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不把几何候选直接当事实或默认调度依据。
- 不调用 LLM 解释、不生成 verified 图式。
- 不做 q 重拟合、theta 更新、维度合并或夜间巩固扩展。
- 不实现 HTTP/UI、HMM、hazard、MRT。
- 不修改冻结参考仓库。

## 交付记录

### 变更清单

- 新增 `geometry` 内核模块：
  - OpenAI-compatible `/v1/embeddings` provider。
  - env 缺失时几何层 disabled，不联网、不写 embedding、不写 `maps_to`。
  - embedding 单位化后以 f32 小端 BLOB 写入 `concepts.embedding`。
  - `embedding.dim` 写入 `meta`，维度不一致时全量拒收且不留下部分写入。
- 引入 `hnsw_rs`，按 `geometry.hnsw_m=16`、`geometry.ef_search=64` 构建内存 HNSW 索引。
- 新增几何候选 API：
  - `Engine::refresh_missing_embeddings`。
  - `Engine::refresh_missing_embeddings_with_provider`。
  - `Engine::geometry_candidates`。
  - `Engine::upsert_geometry_maps_to_candidates`。
- 实现 P03C 组合分数：
  - `cos_E`、`cos_Q`、typed 2-hop `struct`、residual `coh`。
  - `assoc = 0.15·cos_E + 0.35·cos_Q + 0.25·struct + 0.25·coh`。
  - `discover = (0.35·cos_Q + 0.25·struct + 0.25·coh)·(1 − cos_E)`。
- `maps_to` 写入仍复用 `graph::upsert_maps_to_candidate`：
  - 只处理 schema↔schema。
  - 必须过 `graph.struct_threshold`。
  - `alignment_json.requires_llm_verification=true` 保持不变。
- 新增 P03C 集成测试：
  - env 缺失 disabled 且不写库。
  - embedding 单位化与维度登记。
  - 维度不一致无部分写入。
  - HNSW 候选与组合分数。
  - negative `cos_E` 的 discover 候选保留。
  - `maps_to` 结构门。
  - env 缺失时即使已有 embedding 也不写 `maps_to`。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.22s
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
p03c_geometry: 7 passed
doc-tests: 0 passed
exit 0
```

```text
cargo test -p polaris-core --test p03c_geometry
7 passed; 0 failed
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 子 agent 审查

审查 agent：Mendel（`019eb6b6-7d24-7223-863b-f07fbd11a9fd`）。

首轮范围审查结论：

- P03C 必须使用 `hnsw_rs` 或 `instant-distance`，不能用 exact scan 冒充。
- env 缺失时几何层整体停用。
- `maps_to` 不能直接裁决，必须复用 `graph::upsert_maps_to_candidate`。

代码审查发现并已修复：

- Important：embedding 刷新遇到后续维度不一致时会留下部分写入。
  - 修复：先全量 normalize/维度校验，再通过事务统一写入。
  - 回归：维度不一致时断言 `concepts.embedding` 未写、`embedding.dim` 仍为 `0`。
- Important：`cos_E <= 0` 被硬过滤，破坏 `discover` 的“语义远但结构近”语义。
  - 修复：移除正余弦硬过滤，交给 `assoc/discover` 排序和结构门。
  - 回归：negative `cos_E` 候选仍保留且 `discover > 1.0`。
- Minor：OpenAI embedding response 未校验 index 完整性。
  - 修复：校验数量、index 范围、重复和缺失。

复审结论：

- Critical：无。
- Important：无代码问题。
- Minor：无新的代码问题。
- 代码可以提交；流程项为补齐本交付记录。

### 回滚方式

未提交前：

```powershell
git restore Cargo.lock crates/polaris-core/Cargo.toml crates/polaris-core/src/config.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/src/geometry.rs crates/polaris-core/tests/p03c_geometry.rs docs/superpowers/plans/2026-06-11-p03c-geometry-candidates.md docs/tickets/TICKET_P03C_GEOMETRY_CANDIDATES.md
```

提交后：

```powershell
git revert <P03C-commit-sha>
```
