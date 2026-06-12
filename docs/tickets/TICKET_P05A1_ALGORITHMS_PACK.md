# P05A1 算法 Domain Pack (Algorithms Pack)

状态：Queued

服务主命题环节：验证真懂 → 定位模糊 → 针对性补缺（第二域验证领域无关性）

## 背景

SPEC §4 声明内核"领域无关"，MASTER_PLAN 将第二 pack 作为 Phase 5 插拔验收。当前只有 `packs/rust/`（24 概念、21 边、11 误解、3 moves），内核的领域无关性尚未被真实验证。

本票创建 `packs/algorithms/` 作为第二个 domain pack，覆盖经典算法与数据结构。与 rust pack 的知识结构截然不同（算法有更深的 prerequisite 链和更多 confusion 边），可验证引擎在不同图谱拓扑下的行为一致性。

本票同时验证 P05A0 课程接入协议的实际可用性——用协议规范创建 pack，以 `pack validate` 验收。

## 范围

1. 创建 `packs/algorithms/` 目录结构：

   ```
   packs/algorithms/
     pack.toml
     concepts.toml
     misconceptions.toml
     rubric.md
     moves.toml
   ```

2. `pack.toml` 元信息：
   ```toml
   name = "algorithms"
   display_name = "Algorithms & Data Structures"
   version = "0.1.0"
   lang = "en"
   ```

3. `concepts.toml` — 概念种子（≥15 个概念 + prerequisite 链）：

   | seed_order | id | name | 关键 prerequisite |
   |---|---|---|---|
   | 1 | complexity_basics | Big-O, Θ, Ω notation | — |
   | 2 | arrays_lists | Arrays and linked lists | — |
   | 3 | stacks_queues | Stacks and queues | arrays_lists |
   | 4 | hash_tables | Hash tables | arrays_lists |
   | 5 | trees_basics | Binary trees | arrays_lists |
   | 6 | bst | Binary search trees | trees_basics |
   | 7 | heaps | Heaps and priority queues | trees_basics |
   | 8 | graphs_repr | Graph representations | arrays_lists |
   | 9 | comparison_sorts | Comparison sorting lower bound | complexity_basics |
   | 10 | merge_sort | Merge sort | comparison_sorts |
   | 11 | quicksort | Quicksort | comparison_sorts |
   | 12 | bfs_dfs | BFS and DFS | graphs_repr |
   | 13 | shortest_path | Shortest path (Dijkstra, BFS-unweighted) | bfs_dfs, heaps |
   | 14 | dynamic_programming | Dynamic programming | complexity_basics |
   | 15 | greedy | Greedy algorithms | complexity_basics |
   | 16 | divide_conquer | Divide and conquer | merge_sort |
   | 17 | backtracking | Backtracking | bfs_dfs |

   - prerequisite 边建模为 `[[edge]]`，形成 DAG。
   - 多条链：comparison_sorts → merge_sort → quicksort / divide_conquer；graphs_repr → bfs_dfs → shortest_path → DP。

4. `misconceptions.toml` — 常见算法误解（≥8 条）：

   | id | concept_id | title | pattern |
   |---|---|---|---|
   | bigo_equals_theta | complexity_basics | O(n) 等同于 Θ(n) | symbol-referent-confusion |
   | recursion_always_slower | divide_conquer | 递归一定比循环慢 | overgeneralization |
   | greedy_always_optimal | greedy | 贪心总能给出最优解 | overgeneralization |
   | dp_is_just_memoization | dynamic_programming | DP 就是加缓存 | procedural-conceptual-gap |
   | bfs_dfs_interchangeable | bfs_dfs | BFS 和 DFS 可互换 | boundary-blindness |
   | sort_always_nlogn | comparison_sorts | 所有排序都是 O(n log n) | boundary-blindness |
   | hash_always_o1 | hash_tables | 哈希查找永远 O(1) | boundary-blindness |
   | graph_needs_adjacency_matrix | graphs_repr | 图必须用邻接矩阵 | granularity-mismatch |

5. `moves.toml` — 使用与 rust pack 相同的 7 种 move schema（若 P03F 已完成）或 3 种（若未完成）：
   - recall/explain/apply 模板用算法域语言：
     - recall: "用自己的话说明 {concept} 的核心思想和时间复杂度。"
     - explain: "解释 {concept} 在什么场景下比替代方案更优，为什么。"
     - apply: "用伪代码实现 {concept} 的核心逻辑，并分析边界情况。"

6. `rubric.md` — 算法域评分标准：
   - 正确性：算法描述/伪代码在标准输入上正确。
   - 复杂度分析：时间和空间复杂度分析准确。
   - 边界意识：提及至少 1 个边界情况或退化场景。

7. 引擎验证测试 `tests/p05a1_algorithms.rs`：
   - `pack validate packs/algorithms` 通过。
   - `init_pack("packs/algorithms")` 成功，概念和边入库。
   - `next_task` 在 algorithms pack 上正确工作（返回的概念属于 algorithms pack）。
   - prerequisite 门控正确：advanced 概念在前置未达标时不被调度。
   - 误解关联正确：failed attempt 带 misconception_id 时 `misconception_active` 返回 true。
   - 引擎对 algorithms pack 和 rust pack 的行为一致性：同样的 submit/grade 流程产出同结构的 mastery_states。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p polaris-cli -- pack validate packs/algorithms
cargo test -p polaris-core --test p05a1_algorithms
```

额外人工检查：

```powershell
git diff --check
```

验收要求：
- `packs/algorithms/` 通过 `pack validate`。
- 引擎加载 algorithms pack 后所有 Tier 0 功能（调度/掌握度更新/校准/诊断）正常工作。
- 不引入任何算法域专用代码到内核 crate。

## 禁区

- 不在内核写算法域逻辑（CEFR 式写死）。
- 不实现 algorithms 专用的 ingest 适配器。
- 不修改 rust pack 的内容。
- 不修改冻结参考仓库。

## 交付记录

待填写。
