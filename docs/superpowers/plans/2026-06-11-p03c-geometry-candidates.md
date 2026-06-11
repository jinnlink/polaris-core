# P03C 几何候选层 v1 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 激活几何层的 embedding 刷新、HNSW 近邻候选、组合分数和结构门 `maps_to` 候选写入。

**架构：** 新增 `geometry` 模块，负责 embedding provider、向量规范化、瞬时 HNSW 检索和候选打分。`graph` 模块继续负责 typed 2-hop 结构分数与 `maps_to` 写入，几何层只调它，不绕过结构门。

**技术栈：** Rust、SQLite/rusqlite、reqwest blocking、hnsw_rs、serde_json、现有 f32 小端向量 BLOB。

---

## 文件结构

- 创建：`crates/polaris-core/src/geometry.rs`
  - embedding provider trait 与 OpenAI-compatible provider。
  - embedding 刷新 summary、单位化、维度校验。
  - HNSW 近邻检索与 `GeometryCandidate`。
  - assoc/discover 打分与 residual cohesion。
- 修改：`crates/polaris-core/src/lib.rs`
  - 导出 `geometry` 模块。
- 修改：`crates/polaris-core/src/engine.rs`
  - 暴露 `refresh_missing_embeddings`、`refresh_missing_embeddings_with_provider`、`geometry_candidates`、`upsert_geometry_maps_to_candidates`。
- 修改：`crates/polaris-core/src/config.rs`
  - 登记 `geometry.hnsw_m` 与 `geometry.ef_search`。
- 修改：`crates/polaris-core/Cargo.toml`
  - 添加 HNSW 依赖。
- 创建：`crates/polaris-core/tests/p03c_geometry.rs`
  - P03C 红绿测试。
- 修改：`docs/tickets/QUEUE.md`
  - 标记 P03C In Progress。
- 创建：`docs/tickets/TICKET_P03C_GEOMETRY_CANDIDATES.md`
  - 当前票范围与验收。

### 任务 1：红测锁定 P03C 行为

**文件：**
- 创建：`crates/polaris-core/tests/p03c_geometry.rs`

- [ ] **步骤 1：编写失败的测试**

测试必须覆盖：

```rust
#[test]
fn embedding_refresh_normalizes_and_records_dimension()
```

期望：静态 provider 返回非单位向量，刷新后 `concepts.embedding` 为单位向量，`meta('embedding.dim')` 为维度。

```rust
#[test]
fn geometry_candidates_use_hnsw_and_combined_scores()
```

期望：给定 embedding/q/residual/结构，候选包含近邻 schema，且 `assoc`/`discover` 按 DATA_MODEL §6 公式计算。

```rust
#[test]
fn geometry_maps_to_candidates_respect_structure_gate()
```

期望：embedding 相近但结构分数不足时不写 `maps_to`；补齐 typed 结构后才写候选，`alignment_json.requires_llm_verification=true`。

- [ ] **步骤 2：运行测试验证失败**

运行：

```powershell
cargo test -p polaris-core --test p03c_geometry
```

预期：FAIL，原因是 `polaris_core::geometry` 或 Engine 几何入口尚不存在。

### 任务 2：实现 embedding 刷新

**文件：**
- 创建：`crates/polaris-core/src/geometry.rs`
- 修改：`crates/polaris-core/src/lib.rs`
- 修改：`crates/polaris-core/src/engine.rs`

- [ ] **步骤 1：新增 provider 和 summary**

实现：

```rust
pub trait EmbeddingProvider {
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f64>>>;
}

pub struct EmbeddingRefreshSummary {
    pub disabled: bool,
    pub refreshed: usize,
    pub skipped: usize,
    pub dimension: Option<usize>,
}
```

- [ ] **步骤 2：实现单位化与维度写入**

行为：
- 只刷新 `embedding IS NULL` 的 concepts。
- provider 返回数量必须等于输入数量。
- 向量必须非空、有限、非零范数。
- 写入前单位化。
- `embedding.dim` 不存在则插入；存在且不同则报 `InvalidParameter`。

- [ ] **步骤 3：验证测试通过**

运行：

```powershell
cargo test -p polaris-core --test p03c_geometry embedding_refresh_normalizes_and_records_dimension
```

预期：PASS。

### 任务 3：实现 HNSW 候选与组合分数

**文件：**
- 修改：`crates/polaris-core/Cargo.toml`
- 修改：`crates/polaris-core/src/config.rs`
- 修改：`crates/polaris-core/src/geometry.rs`

- [ ] **步骤 1：加入 HNSW 依赖与参数登记**

实现：
- `hnsw_rs` 依赖。
- `geometry.hnsw_m=16`。
- `geometry.ef_search=64`。

- [ ] **步骤 2：实现候选检索**

行为：
- 读取所有有 embedding 的 concepts。
- 用 HNSW 搜索 source 的近邻。
- 跳过自身。
- 只返回正 `cos_E` 候选。

- [ ] **步骤 3：实现组合分数**

行为：
- `cos_Q` 缺失或维度不一致时为 0。
- `struct` 调用现有 `structural_mapping_score`，节点缺失时为 0。
- `coh` 使用 residual_stats 公共周 Pearson；公共周 `<4` 或零方差为 0。
- `assoc`/`discover` 按票据公式。

- [ ] **步骤 4：验证候选测试通过**

运行：

```powershell
cargo test -p polaris-core --test p03c_geometry geometry_candidates_use_hnsw_and_combined_scores
```

预期：PASS。

### 任务 4：结构门写入 maps_to 候选

**文件：**
- 修改：`crates/polaris-core/src/geometry.rs`
- 修改：`crates/polaris-core/src/engine.rs`

- [ ] **步骤 1：实现 upsert 几何 maps_to 候选**

行为：
- 几何层先给候选列表。
- 只对 schema↔schema 调用 `upsert_maps_to_candidate`。
- 未过结构阈值不写边。
- 返回已写入的候选映射。

- [ ] **步骤 2：验证结构门测试通过**

运行：

```powershell
cargo test -p polaris-core --test p03c_geometry geometry_maps_to_candidates_respect_structure_gate
```

预期：PASS。

### 任务 5：整体验证与审查

**文件：**
- 修改：`docs/tickets/TICKET_P03C_GEOMETRY_CANDIDATES.md`

- [ ] **步骤 1：运行完整验收**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03c_geometry
git diff --check
```

- [ ] **步骤 2：请求子 agent 审查**

审查重点：
- 几何只提议，不裁决。
- 环境变量缺失时不联网、不写库。
- `maps_to` 写入仍经过结构门。
- 无 LLM/q/theta/UI/HMM/hazard/MRT 越界。

- [ ] **步骤 3：补交付记录并提交**

提交信息：

```text
feat(P03C): 激活几何候选层
```
