# P06B 数据主权运维

状态：已完成

服务主命题：全环节（Local-persistent 铁律）

## 背景

SPEC 要求 Polaris 单库 SQLite、本地持久、数据主权在用户本机。当前内核已有 WAL、迁移和热路径索引审计，但还缺用户可直接调用的运维入口：安全备份、SQLite 完整性检查，以及事件溯源视角的 `mastery_states` 自检。

本票来自 `docs/ENHANCEMENT_ROADMAP.md` 强化轴线「数据主权运维」候选。目标是补一个小而可审计的运维面，不改变学习算法。

## 本轮范围

1. 新增 `polaris backup --output <path>`：
   - 使用 SQLite `VACUUM INTO` 从当前 `--db` 生成一致性备份。
   - 不覆盖已存在备份文件，失败时返回错误。
2. 新增 `polaris doctor [--json]`：
   - 执行 `PRAGMA integrity_check`。
   - 对有 scored attempt 的概念重跑 `fold_all`，对比 `mastery_states` 的核心折叠字段。
   - 文本输出给人读；`--json` 输出结构化结果。
3. 新增只读核心运维模块，供 CLI 与后续 MCP/HTTP 复用。

## 验收

必须通过：

```powershell
cargo test -p polaris-core ops
cargo test -p polaris-cli backup_and_doctor
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不改 BKT/FSRS/MIRT/相图/HMM/G_u/报告公式。
- 不重写 `mastery_states`，doctor 只读检查，不自动修复。
- 不新增后台服务，不做定时任务。
- 不删除、覆盖用户已有备份文件。
- 不处理 `.gitignore`、`.cursor/`、`docs/visuals/` 等票外改动。
- 不修改 frozen 参考仓库。

## 当前状态

- P06A 已提交：`0ff28ec feat(P06A): 补全 MCP 强化工具面`。
- 当前工作区仍有票外旧改动：`.gitignore`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`。
- 已确认 `Engine::replay_concept_after` 内有事件溯源重放路径；P06B 应抽取/复用其只读折叠逻辑，而不是另写公式。

## 交付记录

### 变更清单

- 新增 `polaris_core::ops`：
  - `doctor_report` 执行 `PRAGMA integrity_check`。
  - 对所有有 scored attempt 的概念重跑 `fold_all`。
  - 对比 `mastery_states` 的 `p_known`、`fsrs_json`、`calib_gap`、`brier_ewma`、`last_depth`、`max_depth`、`attempt_count`、`lapses`。
  - 发现不一致只报告 `ReplayMismatch`，不自动修复。
- 新增 CLI：
  - `polaris backup --output <path>`：使用 `VACUUM INTO` 生成 SQLite 一致性备份；目标已存在时拒绝覆盖。
  - 备份源库使用 `OpenFlags::SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_URI` 打开，不带 `SQLITE_OPEN_CREATE`。
  - `polaris doctor [--json]`：输出完整性与重放自检报告；发现问题时返回错误。
- 更新命令解析覆盖 `backup` / `doctor`。

### 红灯记录

```text
cargo test -p polaris-core ops
error[E0425]: cannot find function `doctor_report` in module `super`
exit 1
```

```text
cargo test -p polaris-cli backup_and_doctor
error[E0425]: cannot find function `doctor_report` in module `polaris_core::ops`
error[E0425]: cannot find function `backup_database` in this scope
error[E0425]: cannot find function `doctor_report_text` in this scope
exit 1
```

### 验收输出

```text
cargo test -p polaris-core ops
ops::tests::ops_doctor_detects_replay_mismatches_without_repairing_state ... ok
1 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-cli backup_and_doctor
tests::backup_and_doctor_helpers_create_backup_and_report ... ok
1 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-cli backup
backup_and_doctor_helpers_create_backup_and_report ... ok
backup_rejects_missing_source_without_creating_it ... ok
backup_rejects_existing_output_without_overwriting ... ok
3 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-cli parses_required_command_set
1 passed; 0 failed
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
failed: Windows target 目录文件锁，报错为 target/debug/deps/libpolaris_core-*.rmeta 写入被拒绝（os error 5），并伴随 incremental 目录 GC warning。
exit 1
```

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.63s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 27 passed
polaris-core unit: 68 passed
integration suites: all passed, including p03c_geometry 8 passed, p03m_latent_dims 3 passed, p05b_breeding 5 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

### 技术选择

- `doctor` 复用 `fold_all` 和 `MasteryParams::from_conn`，不复制 BKT/FSRS 公式。
- 自检只比较事件折叠能确定的核心字段；`phase`、`next_due_at` 等依赖额外引擎上下文的派生字段暂不纳入本票。
- `backup` 不复用 `open_database`，避免备份缺失库时顺手创建新库。

### 子 agent 审查

Singer（`019ec52b-8083-7a61-bda5-9f9a22fbd453`）只读审查结论：

- Critical：无。
- Important 1：`open_existing_database` 不应使用默认带 CREATE 的 `Connection::open`。已修复为显式 `OpenFlags::SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_URI`，不带 `SQLITE_OPEN_CREATE`。
- Minor 1：建议补 backup 源库缺失、输出已存在不覆盖测试。已采纳并新增 2 条安全测试。
- 确认项：doctor 复用 `fold_all`/`MasteryParams::from_conn`，未发现 doctor 写 `mastery_states` 或 `behavior_events`，`doctor/backup` 已在默认创建/迁移路径前处理。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/lib.rs crates/polaris-core/src/ops.rs crates/polaris-cli/src/main.rs docs/tickets/QUEUE.md
git clean -f docs/tickets/TICKET_P06B_DATA_SOVEREIGNTY_OPS.md
```

提交后：

```powershell
git revert <P06B-commit-sha>
```
