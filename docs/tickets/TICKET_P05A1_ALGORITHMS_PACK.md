# P05A1 算法 Domain Pack (Algorithms Pack)

状态：Completed（已提交；用户确认继续推进；默认 target clippy 受 Windows 文件锁阻塞，隔离 target 同参数通过）

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

### 开工记录（2026-06-13）

- 当前范围：创建 `packs/algorithms/` 的 5 个声明式 pack 文件，并新增 `crates/polaris-core/tests/p05a1_algorithms.rs` 覆盖 pack validate、init_pack、next_task、prerequisite 门控、misconception 关联与 algorithms/rust submit 流程一致性。
- 禁区：不在 core 写算法领域专用逻辑；不实现 algorithms 专用 ingest；不修改 `packs/rust/`；不修改冻结参考库 `C:\MyProject\Polaris` 与 `C:\MyProject\Learned`。
- 验收命令：
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo run -p polaris-cli -- pack validate packs/algorithms`
  - `cargo test -p polaris-core --test p05a1_algorithms`
  - `git diff --check`
- 预计修改面：`docs/tickets/QUEUE.md`、本票、`packs/algorithms/*`、`crates/polaris-core/tests/p05a1_algorithms.rs`。

### 交付记录（2026-06-13）

#### 变更清单

- 新增 `packs/algorithms/`：`pack.toml`、`concepts.toml`、`misconceptions.toml`、`moves.toml`、`rubric.md`。
- `concepts.toml` 声明 17 个算法/数据结构概念、16 条 prerequisite 边、2 条 confusion 边；`misconceptions.toml` 声明 8 条常见误解；`moves.toml` 使用 7-move schema。
- 新增 `crates/polaris-core/tests/p05a1_algorithms.rs`，覆盖 validator、pack 初始化、next_task、prerequisite 门控、active misconception 调度优先级、algorithms/rust submit+grade+mastery 形状一致性。
- 更新 `docs/tickets/QUEUE.md` 与本票状态/记录。

#### TDD 红灯

`cargo test -p polaris-core --test p05a1_algorithms`

```text
running 5 tests
test algorithms_pack_validates_expected_shape ... FAILED
test algorithms_and_rust_packs_share_submit_grade_mastery_shape ... FAILED
test failed_attempt_with_misconception_raises_repair_priority ... FAILED
test prerequisite_gate_keeps_advanced_concepts_out_until_ready ... FAILED
test algorithms_pack_initializes_and_schedules_domain_concepts ... FAILED

called `Result::unwrap()` on an `Err` value: MissingFile("pack.toml")
called `Result::unwrap()` on an `Err` value: Pack(MissingFile("pack.toml"))

test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

#### 验收输出

`cargo fmt --check`

```text
第一次：失败，指出 crates\polaris-core\tests\p05a1_algorithms.rs:175 需要换行。
执行 cargo fmt 后复跑：exit 0，无输出。
```

`cargo clippy --workspace --all-targets -- -D warnings`

```text
两次复跑均失败于默认 target 文件锁：
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
```

排查：

```text
Get-Process | Where-Object { $_.ProcessName -match 'cargo|rustc|clippy|rls|rust-analyzer' } | Select-Object ProcessName,Id,Path
输出为空。

Test-Path .git\index.lock
False
```

同参数隔离 target 复核：

```text
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-target-p05a1-clippy'; cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 58.58s
```

`cargo test --workspace`

```text
P04E: test result: ok. 3 passed; 0 failed; finished in 5.65s
P05A1: test result: ok. 5 passed; 0 failed; finished in 0.03s
Doc-tests polaris_core: test result: ok. 0 passed; 0 failed
Finished `test` profile [unoptimized + debuginfo] target(s) in 4.01s
```

`cargo run -p polaris-cli -- pack validate packs/algorithms`

```text
pack ok: concepts=17 prerequisites=16 misconceptions=8
Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.39s
```

`cargo test -p polaris-core --test p05a1_algorithms`

```text
running 5 tests
test algorithms_pack_validates_expected_shape ... ok
test algorithms_pack_initializes_and_schedules_domain_concepts ... ok
test failed_attempt_with_misconception_raises_repair_priority ... ok
test prerequisite_gate_keeps_advanced_concepts_out_until_ready ... ok
test algorithms_and_rust_packs_share_submit_grade_mastery_shape ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

`git diff --check`

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/TICKET_P05A1_ALGORITHMS_PACK.md', LF will be replaced by CRLF the next time Git touches it
```

#### 阻塞点与建议

- 阻塞点：票面原始 `cargo clippy --workspace --all-targets -- -D warnings` 在默认 `target/debug` 下被 Windows `os error 5` 文件写入拒绝挡住；同一问题在 P04E 验收时也出现过。
- 建议：接受隔离 `CARGO_TARGET_DIR` 的同参数 clippy 作为本机 target 锁的替代证据；若必须原命令 exit 0，再单独处理默认 `target/debug` 锁后复跑。
- 是否改变设计/验收/数据模型：不改变设计和数据模型；只涉及本机验收执行环境。
- 用户裁决：用户回复“继续推进”，按接受隔离 target clippy 证据处理。

#### 回滚方式

- 删除 `packs/algorithms/`。
- 删除 `crates/polaris-core/tests/p05a1_algorithms.rs`。
- 将 `docs/tickets/QUEUE.md` 与本票状态/交付记录恢复到 P05A1 开工前。
