# P03G 交错调度 (Interleaved Scheduling)

状态：Completed

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

### 2026-06-12 开工记录

- 当前状态：P03F 已提交；`docs/tickets/QUEUE.md` 中遗留的 P03F 待确认状态已在本票认领时修正为 P03G In Progress。
- 工作区检查：开工前 `git status --short` 无输出。
- 本票范围：新增 `Engine::get_interleaved_batch(batch_size)` 与 MCP 工具 `get_interleaved_batch`；默认 3 题 mini-batch；默认 1 新/弱 + 2 复习，复习 slot 使用 2-hop 超图邻域指纹做簇分散；HMM 仅在 `strategy_enabled=true` 后改变组成；输出包含 move、task_type、template、phase、p_known、expected_success。
- 禁区：不修改单概念 `next_task` 的既有行为；不实现 MRT/Thompson；不让未过门 HMM 影响 batch；不修改冻结参考仓库。
- 验收命令：`cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace`；`cargo test -p polaris-core --test p03g_interleaved`；`git diff --check`。

### 2026-06-12 交付记录

变更清单：

- `crates/polaris-core/src/engine.rs`：新增 `TaskAssignment` 与 `Engine::get_interleaved_batch(batch_size)`；复用现有 U(c) 候选排序；实现默认 1 新/弱 + 2 复习、fatigue/bored 过门后 0 新 + 易复习、flow 过门后 2 新/弱 + 1 复习；复习 slot 使用包含中心节点的 2-hop 超图邻域指纹，要求交集比例 ≤ 50%；`expected_success` 使用现有 MIRT/BKT 融合预测，MIRT 不可用时降级到 BKT/候选 p_known。
- `crates/polaris-cli/src/mcp.rs`：新增 MCP 工具 `get_interleaved_batch`，支持可选 `batch_size`，返回数组字段 `concept_id/concept_name/move/task_type/template/phase/p_known/expected_success`。
- `crates/polaris-core/tests/p03g_interleaved.rs`：新增 P03G 专测，覆盖默认交错、簇分散、85% 均值、gated fatigue、easy review 不足时不补弱/新、最新 ungated HMM 覆盖旧 gated、gated flow、少于 3 概念降级为 `next_task` 等价，以及 pack validate。
- `docs/tickets/QUEUE.md`：P03G 标记为已实现并通过验收。
- 子 agent 审查后修复：fatigue 策略不再用任意候选补位；HMM 策略改为只读取最新一条 mental_state，并仅在该条 `strategy_enabled=true` 时启用；候选详情映射由线性查找改为 `BTreeMap`，避免影响单题路径性能；新增 expected_success 均值调整逻辑，按角色约束尝试替换候选使 batch 均值靠近 `[0.75, 0.90]`。

验收输出：

```powershell
> cargo fmt --check
# exit 0, no output
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.54s
```

```powershell
> cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core --test p03g_interleaved
running 9 tests
test p03g_pack_fixture_still_validates ... ok
test flow_batch_allows_two_weak_concepts ... ok
test fewer_than_three_concepts_degrades_to_existing_next_task ... ok
test ungated_mental_state_does_not_change_default_batch ... ok
test gated_fatigue_does_not_backfill_with_weak_when_easy_reviews_are_insufficient ... ok
test default_batch_interleaves_one_weak_and_two_diverse_reviews ... ok
test latest_ungated_mental_state_overrides_older_gated_state ... ok
test gated_fatigue_batch_uses_only_easy_reviews ... ok
test batch_replaces_out_of_band_review_to_keep_expected_success_in_target ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```powershell
> cargo run -p polaris-cli -- pack validate packs\rust
pack ok: concepts=24 prerequisites=21 misconceptions=11
```

```powershell
> git diff --check
warning: in the working copy of 'crates/polaris-cli/src/mcp.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/TICKET_P03G_INTERLEAVED_SCHEDULING.md', LF will be replaced by CRLF the next time Git touches it
# exit 0
```

备注：`cargo clippy --workspace --all-targets -- -D warnings` 在普通沙箱内曾因默认 `target/debug/deps/*.rmeta` 写入被 Windows 拒绝而失败；清理异常构建产物后仍复现。按同一票面命令在提升权限下重跑通过，未发现 clippy 代码告警。

回滚方式：

```powershell
git restore crates/polaris-core/src/engine.rs crates/polaris-cli/src/mcp.rs docs/tickets/QUEUE.md docs/tickets/TICKET_P03G_INTERLEAVED_SCHEDULING.md
Remove-Item crates/polaris-core/tests/p03g_interleaved.rs
```
