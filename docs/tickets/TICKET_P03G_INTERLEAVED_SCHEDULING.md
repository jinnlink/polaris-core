# P03G 交错调度 (Interleaved Scheduling)

状态：Queued

服务主命题环节：定位模糊 → 针对性补缺

## 背景

当前 `get_next_task` 每次返回单个概念。认知科学交错效应文献（Rohrer & Taylor 2007，43%+ 保持率提升；Brunmair 2019 meta d≈0.42）表明，在一个 mini-batch 内混合不同概念类型的练习能显著提升辨析学习和长期保持。

本票实现 mini-batch 调度：一次返回 3 个概念组成的练习批次，1 个新/弱 + 2 个复习，复习概念来自不同超图簇。同时整合 HMM 状态层（P03D）和 85% 规则（Wilson 2019），使 batch 组成随学习者即时状态自适应。

科学锚点：交错效应（Rohrer & Taylor 2007）、85% 规则（Wilson 2019）（见 `docs/COGNITIVE_SCIENCE_ANCHORS.md`）。

## 范围

1. 新增 mini-batch 调度逻辑：
   - `Engine::get_interleaved_batch(batch_size: usize) -> Vec<TaskAssignment>`，默认 `batch_size=3`。
   - 组成策略：
     - slot 0：U(c) 最高的新/弱概念（`p_known < 0.6` 或无 attempt）。
     - slot 1-2：U(c) 最高的复习概念（`p_known ≥ 0.6`），需来自不同超图簇。
   - 簇划分：取概念的 2-hop 超图邻域指纹；slot 1 和 slot 2 的邻域交集不得超过 50%。若无法满足（概念太少），降级为任意不同概念。

2. 新增 MCP 工具 `get_interleaved_batch`：
   - 输入：`batch_size`（可选，默认 3）。
   - 输出：`[{ concept_id, concept_name, move, task_type, template, phase, p_known, expected_success }]`。
   - 每个 slot 附带预测成功概率 `expected_success = σ(q·θ − b − d_t)` 或 BKT p_known（无 MIRT 时降级）。

3. HMM 状态感知：
   - 当 HMM dominant_state 为 fatigue 或 disengagement（无聊）且 `strategy_enabled=true`（已过门）：
     - batch 切换为 0 新 + 3 易复习（`p_known ≥ 0.8`），降低认知负荷。
   - 当 HMM dominant_state 为 flow 且 `strategy_enabled=true`：
     - batch 允许 2 新/弱 + 1 复习，提高挑战。
   - HMM 未过门时：一律使用默认 1+2 策略。

4. 85% 规则集成：
   - batch 整体预测成功率目标 ≈ 0.85：`mean(expected_success across batch) ∈ [0.75, 0.90]`。
   - 若 slot 0 的 expected_success < 0.50：替换为次优但更易的概念，或给 slot 0 降 move 深度（如从 apply 降到 explain）。

5. 决定性平手排序保持：batch 内排序按 U(c) 降序 → seed_order → id 字典序。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03g_interleaved
```

额外人工检查：

```powershell
git diff --check
```

验收要求：
- batch 包含 3 个不同概念。
- slot 1 和 slot 2 来自不同超图簇（邻域交集 ≤ 50%）。
- batch 整体预测成功率在 [0.75, 0.90] 区间。
- HMM 疲劳态下 batch 不含新概念（仅过门时）。
- 概念数 < 3 时降级为单概念调度（等价现有 `next_task`）。
- MCP 工具返回格式正确且 `pack validate` 通过。

## 禁区

- 不修改单概念 `next_task` 的既有行为——`get_interleaved_batch` 是新增工具，非替换。
- 不实现 MRT 或 Thompson 采样。
- 不让 HMM 未过门的状态层影响 batch 组成。
- 不修改冻结参考仓库。

## 交付记录

待填写。
