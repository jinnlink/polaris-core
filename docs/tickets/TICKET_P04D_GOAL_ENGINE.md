# P04D 目标引擎移植（goals / dimensions / milestones）

## 状态

Done

## 服务主命题

验证真懂 -> 定位模糊 -> 针对性补缺

## 背景

旧 Polaris 在 schema v9 / P32A 中把目标从单一标题扩展为可度量、可分解的目标建模层：`goals` 表承担状态、期限、节奏、优先级与父子目标；`goal_dimensions` 定义可量化达成维度；`goal_milestones` 定义路径里程碑。本票只把这层核心建模能力迁入 `polaris-core`，作为后续目标驱动调度、进度分析与 UI 暴露的稳定地基。

参考只读来源：

- `C:\MyProject\Polaris\apps\web\src\lib\db\migrate.ts` 的 schema v9。
- `C:\MyProject\Polaris\docs\tickets\TICKET_P32_GOAL_ENGINE_V0.md` 的 Part A。

## 范围

1. 数据迁移：
   - 在当前一次性迁移模式中新增 `goals`、`goal_dimensions`、`goal_milestones`。
   - 字段、默认值、唯一约束和索引对齐旧库 schema v9 中的 Part A 建模层。
   - `goals` 直接创建完整 v9 形态，不迁移旧库 v4 的简化形态。

2. Core API：
   - 新增目标建模模块，支持创建目标时同时写入 dimensions 和 milestones。
   - 支持读取单个目标的建模快照。
   - 支持更新目标维度当前值。
   - 支持计算确定性的目标进度报告：按维度权重加权平均，维度进度限制在 `0..=1`。
   - 支持按 `dimension_threshold` / `manual` 规则刷新里程碑状态。

3. Engine 封装：
   - 在 `Engine` 上暴露薄封装方法，供后续 CLI/HTTP/MCP/UI 复用。
   - 保持现有学习调度、MRT、评分、模拟行为不变。

4. 测试：
   - 新增 `crates/polaris-core/tests/p04d_goals.rs` 覆盖 schema、创建/读取、进度、里程碑刷新。
   - 先运行新增测试确认红灯，再实现。

## 禁区

- 不实现 Part B 的自动 `refreshDimensionValues`、速度、ETA、风险分析。
- 不新增 HTTP / MCP 工具和资源。
- 不实现 `auto_decompose`、LLM 拆解、`link_tracks` 或 track 生成。
- 不新增领域特定指标计算器，不把英语 `vocab` / `grammar` 逻辑写入 core。
- 不改 `ranked_task_candidates()`、`next_task()`、MRT move 选择或学习调度公式。
- 不修改冻结参考仓库。
- 本票不迁入 `goal_activities`；旧 schema v9 中该表作为活动日志扩展，另票处理。

## 验收

```powershell
cargo test -p polaris-core --test p04d_goals
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

如果默认 `target/debug` 在 Windows 文件锁下导致 clippy 写入失败，可使用隔离 target 目录重跑同一检查，并在交付记录中保留两次输出。

## 当前状态

- 已确认 QUEUE 中没有其他 In Progress 票。
- 已确认 P04D 是 Phase 4 唯一未落票项；P04E 已先行完成。
- 已读取旧库 schema v9 和 P32A Part A：本票只迁移 `goals` / `goal_dimensions` / `goal_milestones` 建模核心。

## 交付记录

### 2026-06-14

#### 本轮范围

- 按单票制认领 P04D，只迁入目标建模核心：`goals`、`goal_dimensions`、`goal_milestones`。
- 参考旧 Polaris schema v9 / P32A Part A；当前库采用一次性迁移，因此直接创建完整 v9 形态。
- 不实现 HTTP/MCP、`auto_decompose`、`link_tracks`、`goal_activities`、自动指标刷新、领域特定指标或调度改造。

#### 变更清单

- 新增 `crates/polaris-core/src/goals.rs`
  - `GoalInput`、`GoalDimensionInput`、`GoalMilestoneInput` 输入结构。
  - `GoalRecord`、`GoalDimensionRecord`、`GoalMilestoneRecord` 快照结构。
  - `GoalProgressReport` 及维度/里程碑进度结构。
  - `create_goal`：事务性写入目标、维度、里程碑；子表约束失败会整体回滚。
  - `goal_snapshot`：读取目标建模快照。
  - `update_goal_dimension_value`：更新维度当前值和目标更新时间。
  - `goal_progress`：按正向目标值计算 `0..=1` 维度进度，并按权重求整体进度。
  - `refresh_goal_milestones`：只处理 `dimension_threshold` 触发器，不自动触发 `manual`。
- 更新 `crates/polaris-core/src/db.rs`
  - 新增 `goals`、`goal_dimensions`、`goal_milestones`。
  - 新增 `idx_goals_updated_at`、`idx_goals_status`、`idx_goals_parent`、`idx_goal_dim_goal`、`idx_milestone_goal`。
  - 迁移自检表/索引清单纳入目标引擎表。
- 更新 `crates/polaris-core/src/engine.rs`
  - 为目标建模 API 增加薄封装方法，不接入调度/MRT/评分路径。
- 更新 `crates/polaris-core/src/error.rs`、`crates/polaris-core/src/lib.rs`
  - 导出 `goals` 模块并增加 `MissingGoal` 错误。
- 新增 `crates/polaris-core/tests/p04d_goals.rs`
  - 覆盖 schema 表/字段/默认值/NOT NULL/唯一约束/索引。
  - 覆盖目标创建与快照读取。
  - 覆盖子表唯一约束失败时事务回滚。
  - 覆盖非正目标值拒绝。
  - 覆盖加权进度、clamp 和 `dimension_threshold` 里程碑刷新。
- 更新 `docs/tickets/QUEUE.md`
  - P04D 从 In Progress 改为已完成。

#### 子 agent 审查

- 审查结论：未发现 Critical / Important；可合并。
- 采纳补强：
  - 补 schema 约束测试。
  - 补事务回滚测试。
  - 将 `target_value` 收紧为必须为正数，并补拒绝测试。

#### 验收输出

```powershell
> cargo test -p polaris-core --test p04d_goals
running 6 tests
test create_goal_rejects_non_positive_dimension_targets ... ok
test create_goal_rolls_back_when_child_rows_violate_constraints ... ok
test refresh_goal_milestones_reaches_dimension_thresholds_only ... ok
test goal_progress_is_weighted_and_clamped ... ok
test migration_creates_goal_engine_tables_and_indexes ... ok
test create_goal_persists_dimensions_and_milestones ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

```powershell
> cargo fmt --check
# exit code: 0
```

默认 target 的 clippy 先遇到 Windows 文件锁：

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-*.rmeta: 拒绝访问。 (os error 5)
# exit code: 1
```

用隔离 target 目录重跑同一检查通过：

```powershell
> cargo clippy --workspace --all-targets --target-dir "$env:TEMP\polaris-p04d-clippy" -j 1 -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.62s
```

```powershell
> cargo test --workspace
test result: ok. 23 passed; 0 failed
test result: ok. 66 passed; 0 failed
...
test result: ok. 6 passed; 0 failed
...
Doc-tests polaris_core: ok
```

```powershell
> git diff --check
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/db.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/error.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
# exit code: 0
```

#### 回滚方式

```powershell
git revert <P04D-commit>
```
