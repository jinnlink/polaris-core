# P03F Moves Bloom 扩展 (Moves Bloom Expansion)

状态：Completed

服务主命题环节：验证真懂 → 针对性补缺

## 背景

当前 `moves.toml` 只有 3 种 move（recall/explain/apply），对应 Bloom 分类学前三层。MASTER_PLAN 教学法纲要 v3 明确要求更细粒度的深度评判：recall→explain→apply→transfer，且教法签名 F1 的有效性依赖于 move 库的丰富度。SPEC §1 定义"掌握"需在"足够深度上被证明"——3 种 move 不足以覆盖 Bloom 修订版的完整分类。

本票将 move 从 3 种扩展到 7 种，对齐 Anderson & Krathwohl 2001 修订版 Bloom 分类学 + 迁移维度。每种 move 有明确的认知参与层级、评分 rubric 要求和调度选取规则。

科学锚点：Bloom 1956 + Anderson & Krathwohl 2001（修订版 Bloom 分类学）；Chi & Wylie 2014 ICAP 框架（见 `docs/COGNITIVE_SCIENCE_ANCHORS.md`）。

## 范围

1. 扩展 move schema——`moves.toml` 从 3 种到 7 种：

   | move_id | Bloom 层级 | task_type | MIRT d_t | 认知要求 |
   |---------|-----------|-----------|----------|---------|
   | recall | Remember | recall | −0.30 | 用自己的话复述核心约束 |
   | explain | Understand | free_explain | 0.00 | 解释为什么某种写法被限制 |
   | apply | Apply | apply | +0.30 | 给出最小可运行例子并说明边界 |
   | analyze | Analyze | analyze | +0.40 | 比较两种设计的 trade-off，识别模式 |
   | evaluate | Evaluate | evaluate | +0.45 | 审查给定代码找 bug/设计缺陷 |
   | create | Create | create | +0.50 | 从零设计 API/数据结构/架构方案 |
   | transfer | Transfer | transfer | +0.50 | 在不同项目/领域上下文中应用概念 |

2. pack TOML schema 变更：
   - `moves.toml` 新增 4 个 `[[move]]` 条目（analyze/evaluate/create/transfer），每条含 `id`、`task_type`、`template`（中文模板）。
   - `pack.toml` 版本号更新以标记不兼容变更。
   - `polaris pack validate` 更新：新 move id 合法但旧 pack 仍可通过（向后兼容）。

3. 调度器集成——按掌握度深度选 move：
   - `next_task` 选定概念后，根据 `mastery_states.max_depth` 选取下一 move：
     - max_depth = NULL → recall
     - max_depth = recall → explain
     - max_depth = explain → apply
     - max_depth = apply → analyze 或 evaluate（轮替）
     - max_depth = analyze/evaluate → create
     - max_depth = create → transfer
   - 当 `p_known < 0.5` 时回退到 recall 不论 max_depth（防止强推深度）。

4. 评分 rubric 扩展：
   - `rubric.md` 为每种 move type 定义评分标准：
     - analyze：要求对比 ≥2 个方案，识别至少 1 个 trade-off。
     - evaluate：要求定位 ≥1 个 bug/缺陷并解释原因。
     - create：要求产出可编译的设计方案并说明决策理由。
     - transfer：要求在不同于原 pack 的上下文中正确应用。
   - grader prompt 模板按 move type 切换 rubric 段落。

5. MCP `teaching_instruction` 工具适配：
   - `move` 字段从 3 值枚举扩展到 7 值。
   - `target_depth` 字段显式携带 Bloom 层级。

6. MIRT 难度参数注册：
   - 新增 `mirt.d.analyze`、`mirt.d.evaluate`、`mirt.d.create` 到 `meta` 表（B 类参数）。
   - `mirt.d.transfer` 复用现有 `+0.50`。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03f_moves
cargo run -p polaris-cli -- pack validate packs/rust
```

额外人工检查：

```powershell
git diff --check
```

验收要求：
- `packs/rust/moves.toml` 含 7 条 move，`pack validate` 通过。
- 新 move 的 `task_type` 在 MIRT 有对应 d_t 值。
- `next_task` 在 max_depth=apply 的概念上返回 analyze/evaluate 而非再次 apply。
- 旧 3-move pack 仍可通过 validate（向后兼容）。

## 禁区

- 不实现 MRT 签名估计或 Thompson 采样——仅扩展 move 库。
- 不修改 BKT/FSRS/校准公式。
- 不在内核写域特定的 rubric 逻辑——rubric 内容留在 pack 的 `rubric.md`。
- 不修改冻结参考仓库。

## 交付记录

## AI 交接记录（2026-06-12 开工）

- 当前状态：已按 QUEUE 与 ENHANCEMENT_ROADMAP 认领 P03F；P03D/P03E 均已在 QUEUE 标为完成，下一票为 P03F。
- 本轮范围：仅实现 P03F 票内的 moves 扩展、pack validate 兼容、next_task move 选择、rubric/grader 提示适配、teaching_instruction 字段扩展、MIRT d_t 参数登记与对应测试。
- 禁区：不实现 MRT 签名估计/Thompson 采样，不改 BKT/FSRS/校准公式，不把 Rust 领域 rubric 逻辑写入内核，不修改冻结参考仓库。
- 验收命令：`cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace`；`cargo test -p polaris-core --test p03f_moves`；`cargo run -p polaris-cli -- pack validate packs/rust`；`git diff --check`。
- 已知工作区状态：开工前已有未提交/未跟踪文档与后续票文件；本轮不回退这些既有改动。

## 交付记录（2026-06-12）

### 变更清单

- `packs/rust/moves.toml` 扩展为 7 个 Bloom move：recall/explain/apply/analyze/evaluate/create/transfer；`pack.toml` 版本升至 `0.2.0`；`rubric.md` 增加 7 个 move 分段评分标准。
- `crates/polaris-core/src/moves.rs` 新增领域无关 move 选择与模板渲染；`pack.rs` 读取并安装 move 模板到 `meta('pack.<id>.moves')`，旧 3-move pack 仍可 validate。
- `engine.next_task` 在选定概念后按 `p_known/max_depth` 选择下一 move；`p_known < 0.5` 回退 recall；`max_depth=apply` 在 analyze/evaluate 间轮替。
- `mastery.rs`、`phase.rs` 扩展 depth 识别；`submit` 的 optimistic depth 按 task_type 写入，不再统一写 recall。
- `config.rs` 新增 `mirt.d.analyze/evaluate/create`，并显式登记 `mirt.d.free_explain=0.00`；`mirt.rs` 与 `consolidation.rs` 使用该难度。
- `grader.rs` 接受新 depth，并在 prompt 中声明当前 `task_type`，要求按 pack rubric 对应段落评分。
- `teaching.rs`/MCP 输出新增 `target_depth`，普通概念 teaching_instruction 跟随同一 Bloom move 选择。
- 新增 `crates/polaris-core/tests/p03f_moves.rs` 覆盖 7 moves、旧 pack 兼容、MIRT 难度、next_task 阶梯、teaching target_depth、grader 新 depth。

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.74s
exit 0
```

说明：普通权限首次运行 clippy 时 Windows 拒绝写入 `target/debug/deps/*.rmeta`（os error 5）；按权限规则提升权限重跑同一命令后通过，失败原因为文件系统写入权限，不是 lint 错误。

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
p03d_mental_state: 10 passed
p03e_phase: 15 passed
p03f_moves: 7 passed
doc-tests: 0 passed
exit 0
```

说明：一次全量测试中 `p03c_geometry` 出现非稳定失败；单独重跑 `cargo test -p polaris-core --test p03c_geometry` 通过，随后精确重跑 `cargo test --workspace` 通过。最终验收以上方通过输出为准。

```text
cargo test -p polaris-core --test p03f_moves
running 7 tests
test grader_accepts_new_bloom_depths ... ok
test rust_pack_declares_seven_bloom_moves_and_validates ... ok
test legacy_three_move_pack_still_validates ... ok
test teaching_instruction_exposes_target_depth_for_bloom_moves ... ok
test next_task_advances_from_apply_to_analyze_evaluate_and_falls_back_when_weak ... ok
test next_task_follows_full_bloom_depth_ladder ... ok
test new_move_task_types_have_registered_mirt_difficulties ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo run -p polaris-cli -- pack validate packs/rust
pack ok: concepts=24 prerequisites=21 misconceptions=11
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 技术选择

- move 选择保持领域无关：内核只知道 Bloom move 阶梯、task_type、target_depth；具体中文任务模板和 rubric 均来自 pack。
- analyze/evaluate 在 `max_depth` 中同阶；调度在 `max_depth=apply` 时按已有 analyze/evaluate 尝试次数轮替，任一达到后下一步进入 create。
- explain 的 MIRT 难度按 P03F 表显式落为 `mirt.d.free_explain=0.00`；旧 `free_produce` 保留为更高难度自由产出/迁移类任务。
- `teaching_instruction` 的诊断补缺 move（repair_prerequisite/discriminate）保留 P02C 语义，同时增加 `target_depth`，避免破坏 Tier 2 护栏。
- `submit` 在乐观落账时按 task_type 写入 target depth；这是 P01 乐观更新铁律下的观测占位，final grader 回来后仍以 evidence-bound depth 覆盖。

### 子 agent 复查处理

- 已按用户建议使用子 agent 做只读复查，未改文件。
- 子 agent 报告的 QUEUE 状态不一致为读取延迟；主线复核 `QUEUE.md` 当前状态为 `P03F 已完成，等待用户确认 commit`。
- 采纳补测建议：
  - 补 `next_task_follows_full_bloom_depth_ladder` 覆盖 `NULL→recall`、`recall→explain`、`explain→apply`、`analyze/evaluate→create`、`create→transfer`。
  - MCP `get_next_task` JSON 测试补断言 `teaching_instruction.move` 与 `target_depth`。
  - 旧 3-move pack 兼容测试改为不带 `task_type` 的旧形态，确认 validator 会按 move id 补默认 task_type。
- 复查确认仍需注意：工作区存在 P03F 外的既有未提交文档/后续票文件；提交 P03F 时应只暂存本票相关文件。

### 回滚方式

未提交前：

```powershell
git restore crates/polaris-cli/src/mcp.rs crates/polaris-core/src/config.rs crates/polaris-core/src/consolidation.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/grader.rs crates/polaris-core/src/lib.rs crates/polaris-core/src/mastery.rs crates/polaris-core/src/mirt.rs crates/polaris-core/src/pack.rs crates/polaris-core/src/phase.rs crates/polaris-core/src/teaching.rs packs/rust/moves.toml packs/rust/pack.toml packs/rust/rubric.md docs/tickets/QUEUE.md docs/tickets/TICKET_P03F_MOVES_BLOOM_EXPANSION.md
git clean -f crates/polaris-core/src/moves.rs crates/polaris-core/tests/p03f_moves.rs
```

提交后：

```powershell
git revert <P03F-commit-sha>
```
