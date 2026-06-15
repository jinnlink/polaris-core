# P07A 相图产品化语义层

状态：已完成

服务主命题环节：验证真懂 -> 用户读懂

## 背景

P03E 已经让引擎能稳定判定知识相图，但当前对外输出仍是开发者枚举值：

- `undetermined`
- `phantom`
- `fluctuation`
- `settling`
- `solidification`
- `transfer`
- `generation`
- `regression`

这些值适合审计与测试，不适合直接展示给学习者。P07A 只补一层纯展示语义：给每个相增加学习者可读的产品名与一句话解读，并让 status 输出同时保留稳定枚举值与可读字段。

## 范围

1. `Phase` 语义映射：
   - 在 `crates/polaris-core/src/phase.rs` 的 `impl Phase` 中新增 `label()` 与 `summary()`。
   - 覆盖 `Phase::ALL` 的 8 个相。
   - 产品名口语化，不超过 5 个字；解读描述现象，不贴学习者标签。
   - 保留 `as_str()`、`parse()`、`progress_rank()`、`schedule_bonus()` 现有语义。

2. Status 输出：
   - `ConceptStatus` 只增字段，不改字段：
     - `phase_label`
     - `phase_summary`
   - 原 `phase` 字段继续输出稳定枚举字符串，确保 JSON API 向后兼容。

3. 测试：
   - 为 status snapshot 增加 JSON 字段断言，证明新字段随概念输出。
   - 为 `Phase::ALL` 增加语义覆盖断言，确保 8 个相都有 label/summary，且枚举字符串不变。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core --test p03e_phase
cargo test -p polaris-core --test p04a_desktop_status
```

专项验收要求：

- `determine_phase` 判据不发生任何代码修改。
- 8 个 `Phase` 枚举成员仍保持原有 `as_str()` 输出。
- `status_snapshot` 的 `ConceptStatus.phase` 保留原枚举字符串。
- `status_snapshot` 的每个概念新增 `phase_label` 与 `phase_summary`。
- 不新增推荐动作字段；P07D/P07C 再处理行动闭环与报告顶部提示。

## 禁区

- 不修改相图判据、优先级顺序、调度、MRT、breeding、报告断言或任何公式。
- 不修改数据库 DDL、参数登记处或 DATA_MODEL。
- 不新增 UI/Tauri 面板。
- 不新增 CLI/MCP/HTTP 接口；本票只扩展已有 status 结构。
- 不修改冻结参考仓库。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/` 等预存改动。

## 本轮范围（2026-06-15）

- 当前状态：P08B 已提交（`9070423`），产品路线图已提交（`9585bf0`）。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`。本票不得回退或混入这些改动。
- 产品/架构要求：`phase.rs` 增加纯语义映射，`status.rs` 新增 `phase_label` / `phase_summary`；不改 `determine_phase` 判据，不改原 `phase` 枚举输出。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md crates/polaris-core/src/phase.rs crates/polaris-core/src/status.rs crates/polaris-core/tests/p03e_phase.rs crates/polaris-core/tests/p04a_desktop_status.rs crates/polaris-cli/src/main.rs
Remove-Item docs/tickets/TICKET_P07A_PHASE_PRODUCT_SEMANTICS.md
```

提交后：

```powershell
git revert <P07A-commit-sha>
```

## 交付记录（2026-06-15）

### 变更清单

- `crates/polaris-core/src/phase.rs`：
  - 新增 `Phase::label()` 与 `Phase::summary()`。
  - 覆盖 8 个相，不改变 `as_str()`、`parse()`、`progress_rank()`、`schedule_bonus()` 或 `determine_phase` 判据。
- `crates/polaris-core/src/status.rs`：
  - `ConceptStatus` 新增 `phase_label` 与 `phase_summary`。
  - 原 `phase` 字段继续输出稳定枚举字符串。
- `crates/polaris-cli/src/main.rs`：
  - 更新 status JSON 测试夹具，覆盖新增字段。
  - 文本输出测试保持旧格式不变。
- 测试：
  - `p03e_phase.rs` 增加 8 相语义覆盖与稳定枚举断言。
  - `p04a_desktop_status.rs` 增加 status JSON 新字段断言。
- 文档：
  - `QUEUE.md` 标记 P07A In Progress。

### 产品名与解读

| phase | phase_label | phase_summary |
|---|---|---|
| `undetermined` | 还看不清 | 才试了几次，证据还不够，系统会先补探针任务。 |
| `phantom` | 看起来懂 | 自信高但实际表现不稳，需要用更硬的题确认。 |
| `fluctuation` | 刚上路 | 表现起伏明显，结果还不结实。 |
| `settling` | 刚扎根 | 原场景中渐稳，新场景还卡。 |
| `solidification` | 稳了但僵 | 熟练但迁移受限，需要用变式题松动。 |
| `transfer` | 能迁移 | 能在新情境使用。 |
| `generation` | 能创造 | 能独立产出，且迁移表现更快更稳。 |
| `regression` | 退步了 | 之前会但近期又脱档，需要回到证据补缺。 |

### TDD 红灯记录

```text
cargo test -p polaris-core --test p04a_desktop_status
test status_snapshot_exposes_stable_phase_counts_for_desktop_mirror ... FAILED
left: Null
right: "看起来懂"
exit 101
```

```text
cargo test -p polaris-core --test p03e_phase phase_semantics_cover_all_stable_phase_values
error[E0599]: no method named `label` found for reference `&Phase`
error[E0599]: no method named `summary` found for reference `&Phase`
exit 101
```

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.14s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 31 passed
polaris-core unit: 63 passed
engine_submit_pipeline: 5 passed
engine_task_selection: 3 passed
p03e_phase: 18 passed
p04a_desktop_status: 1 passed
p08b_privacy: 4 passed
all existing integration tests and doc-tests passed
exit 0
```

```text
cargo test -p polaris-core --test p03e_phase
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p04a_desktop_status
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- 语义映射放在 `Phase` 同位实现，避免在 status 层复制相图知识。
- Status JSON 只增 `phase_label` / `phase_summary`，不改 `phase`，保持向后兼容。
- 不新增推荐动作字段，避免越界到 P07C/P07D。

### 待审事项

- 产品/架构审查结果：ship it。
