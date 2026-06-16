# P09C polaris doctor --diagnose 全面诊断

状态：已完成

服务主命题环节：全环节（运维可见性 / 可审计性）

## 背景

P06B 已经提供 `polaris doctor`：SQLite integrity check + mastery_states 事件溯源重放自检。它回答“数据库是否一致”。P09C 补一个只读诊断视图，回答“最近 7 天引擎到底跑了什么”。

本票只做运维摘要，不改 DDL、不写数据库、不触发 LLM、不改变 tuning / breeding / mental fit / GU / consolidation / report 的任何行为。

## 范围

1. CLI：
   - 在现有命令上新增 flag：

```powershell
polaris doctor --diagnose [--json]
```

   - `polaris doctor` 现有行为保持不变。
   - `--diagnose` 与既有 doctor integrity/replay 自检可共存：先输出/返回原 `DoctorReport`，再附加诊断摘要。
   - `--json` 输出结构化对象，包含 `doctor` 与 `diagnostics` 两块。

2. Core 只读诊断结构：
   - 在 `ops.rs` 或新 `diagnostics.rs` 中新增只读结构（命名待实现就近决定）：
     - `DoctorDiagnostics`
     - `ActivitySummary`
   - 每类摘要至少包含：
     - `count_7d`
     - `last_at`
     - `last_status`
   - 所有查询使用 read-only `&Connection`，不写库。

3. 摘要类别：
   - `param_tuning_runs`：
     - count：最近 7 天 run 数。
     - last_at：最近 `ran_at`。
     - last_status：最近 `status`。
   - `bred_moves` / breeding evaluations：
     - count：最近 7 天 updated/admitted/retired 记录数。
     - last_at：`COALESCE(updated_at, admitted_at, retired_at, created_at)` 最大值。
     - last_status：最近 `status`。
   - mental fit 拆两条：
     - `mental_fit.hazard`：读取 `hazard_models`，count=最近 7 天拟合数，last_at=`fitted_at`，last_status=`fitted (auc=0.XX)`。
     - `mental_fit.state_gate`：读取 `state_gate_evals`，count=最近 7 天评估数，last_at=`evaluated_at`，last_status=`passed (margin=+0.XX)` 或 `failed_gate (margin=-0.XX)`。
   - GU induction：
     - 从 `gu_rules` 最近 7 天 `updated_at` 统计。
   - `consolidation_runs`：
     - count：最近 7 天 run 数。
     - last_at：最近 `ran_at`。
     - last_status：最近 `status`。
   - breeding 拆三条：
     - `breeding.evaluated_7d`：读取 `bred_moves.updated_at`。
     - `breeding.admitted_7d`：读取 `bred_moves.admitted_at`。
     - `breeding.retired_7d`：读取 `bred_moves.retired_at`。
   - `mirror_reports`：
     - count：最近 7 天报告数。
     - last_at：最近 `generated_at`。
     - last_status：可用 `ok` / `generated` 固定状态，或基于报告 JSON 是否可解析给出 `parse_error`。

4. 文本输出建议：

```text
ok=true
integrity=ok
replay_checked=...

diagnostics_window_days=7
param_tuning_runs	count_7d=...	last_at=...	last_status=...
breeding.evaluated_7d	count_7d=...	last_at=...	last_status=...
breeding.admitted_7d	count_7d=...	last_at=...	last_status=...
breeding.retired_7d	count_7d=...	last_at=...	last_status=...
mental_fit.hazard	count_7d=...	last_at=...	last_status=fitted (auc=0.XX)
mental_fit.state_gate	count_7d=...	last_at=...	last_status=passed | failed_gate (margin=+/-X.XX)
gu_inductions	count_7d=...	last_at=...	last_status=...
consolidation_runs	count_7d=...	last_at=...	last_status=...
mirror_reports	count_7d=...	last_at=...	last_status=...
```

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core ops
cargo test -p polaris-cli doctor
```

专项手工命令：

```powershell
cargo run -p polaris-cli -- doctor --diagnose
cargo run -p polaris-cli -- doctor --diagnose --json
```

专项验收要求：

- `polaris doctor` 不带 `--diagnose` 的文本和 JSON 行为保持。
- `--diagnose` 只读，不插入/更新任何表。
- 空库/缺活动时所有诊断类别输出 count=0，不报错。
- 不新增 DDL，不要求历史迁移。
- JSON 输出字段稳定，适合后续 UI/运维消费。
- `cargo run -p polaris-cli -- doctor --diagnose --json` 输出为 `{"doctor": {...}, "diagnostics": {...}}` 两块独立，不合并字段。

## 禁区

- 不修改 tuning、breeding、mental fit、GU、consolidation、report 的生成逻辑。
- 不触发 LLM，不运行任何后台 job。
- 不改变 doctor 原有 integrity/replay 判定语义。
- 不修改冻结参考仓库。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/` 等预存改动。

## 本轮范围（2026-06-15）

- 当前状态：P07C 已提交（`af1a1a6`）。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`。本票不得回退或混入这些改动。
- 产品/架构审查结果：按反馈修订后进入实现；mental_fit 拆 hazard/state_gate，breeding 拆 evaluated/admitted/retired，doctor/diagnostics JSON 两块独立。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md crates/polaris-core/src/ops.rs crates/polaris-cli/src/main.rs
Remove-Item docs/tickets/TICKET_P09C_DOCTOR_DIAGNOSE.md
```

提交后：

```powershell
git revert <P09C-commit-sha>
```

## 交付记录（2026-06-15）

### 变更清单

- `crates/polaris-core/src/ops.rs`：
  - 新增 `ActivitySummary` / `DoctorDiagnostics`。
  - 新增 `doctor_diagnostics(conn, window_days)` 只读诊断摘要。
  - 诊断项覆盖 param tuning、breeding 三分项、mental fit 两分项、GU、consolidation、mirror reports。
  - hazard last_status 含 `auc=...`，state gate last_status 含 `margin=...`。
- `crates/polaris-cli/src/main.rs`：
  - `polaris doctor` 新增 `--diagnose` flag。
  - `doctor --diagnose --json` 输出 `{ "doctor": ..., "diagnostics": ... }` 两块独立。
  - 文本输出保留原 doctor 行，再追加 diagnostics block。
- 测试：
  - core 单测覆盖最近 7 天各诊断项摘要。
  - CLI 单测覆盖 `doctor --diagnose --json` 解析与 JSON 两块独立。
- 文档：
  - `QUEUE.md` 标记 P09C In Progress。

### TDD 红灯记录

```text
cargo test -p polaris-core ops_doctor_diagnostics_summarizes_recent_activity
error[E0425]: cannot find function `doctor_diagnostics` in module `super`
exit 101
```

```text
cargo test -p polaris-cli doctor_diagnose
error[E0433]: cannot find `DoctorDiagnostics` in `ops`
error[E0026]: variant `Commands::Doctor` does not have a field named `diagnose`
error[E0425]: cannot find function `doctor_diagnose_json` in this scope
exit 101
```

### 验收输出

```text
cargo test -p polaris-core ops_doctor_diagnostics_summarizes_recent_activity
test ops::tests::ops_doctor_diagnostics_summarizes_recent_activity ... ok
exit 0
```

```text
cargo test -p polaris-cli doctor_diagnose
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 35 filtered out
exit 0
```

```text
cargo run -p polaris-cli -- --db target/p09c-doctor.sqlite doctor --diagnose
ok=true
integrity=ok
replay_checked=0
diagnostics_window_days=7
param_tuning_runs	count_7d=0	last_at=-	last_status=-
mental_fit.hazard	count_7d=0	last_at=-	last_status=-
mental_fit.state_gate	count_7d=0	last_at=-	last_status=-
exit 0
```

```text
cargo run -p polaris-cli -- --db target/p09c-doctor.sqlite doctor --diagnose --json
输出包含顶层 "doctor" 与 "diagnostics" 两块独立对象
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.93s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 37 passed
polaris-core unit: 68 passed
all existing integration tests and doc-tests passed
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- `--diagnose` 复用原 DoctorReport，不改变 integrity/replay 语义。
- 诊断摘要全部是只读 SQL，不写库、不触发后台 job、不调用 LLM。
- mental fit 按产品审查拆为 `mental_fit.hazard` 与 `mental_fit.state_gate`。
- breeding 按产品审查拆为 evaluated/admitted/retired 三条，避免隐藏 admitted/retired 价值。

### 待审事项

- 产品/架构 commit 前 diff 审查已通过，结论：ship it。
