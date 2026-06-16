# P11A 数据库 schema 版本化迁移

状态：已通过验收并提交

服务主命题环节：全环节（Local-persistent / 可演进底座）

## 背景

`PRODUCT_ROADMAP.md` §7 已把「数据库 schema 版本化迁移」列为 schema 演进时再立的 P11A。当前 `crates/polaris-core/src/db.rs::migrate()` 仍是一次性幂等建表：新库可用，部分旧库靠 `ensure_column` 补列，但没有正式 schema 版本、迁移账本或 doctor 可见性。

P07-P10 主轴已经完成，schema 里已有 goals、pack theta、trust 依赖表、热路径索引等多轮演进。继续加功能前，先把迁移系统变成可审计、可幂等、可诊断的底座。

## 范围

1. Core migration 版本化：
   - 在 `db.rs` 增加 `CURRENT_SCHEMA_VERSION`。
   - 新增 `schema_migrations(version, name, applied_at)` 迁移账本。
   - `migrate()` 必须把当前完整 schema 作为 baseline migration 记录下来，并设置 SQLite `PRAGMA user_version`。
   - 重复执行 `migrate()` 不得重复插入迁移记录，不得覆盖用户已有 `meta` 值。

2. 旧库基线兼容：
   - 对没有 `schema_migrations` 的既有库，先沿用当前幂等 DDL / `ensure_column` 补齐当前 schema，再写入 baseline 记录。
   - 保留既有数据和用户手动参数值。
   - 只处理本仓库已支持的旧 schema 兼容面；不承诺迁移任意外部未知 SQLite。

3. 可诊断性：
   - `doctor_report` 增加 schema version 与 migration count。
   - CLI `polaris doctor` 文本和 JSON 都能看到当前 schema 版本。
   - 只读入口仍然只读：`doctor` / `diagnose` / `trust show` 不创建库、不自动迁移。

4. 文档：
   - `docs/DATA_MODEL.md` 增加 schema 版本账本说明。
   - `QUEUE.md` 标记本票 In Progress。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core db::tests
cargo test -p polaris-core ops::tests
cargo test -p polaris-cli doctor
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- 新库迁移后 `PRAGMA user_version == CURRENT_SCHEMA_VERSION`。
- 新库迁移后 `schema_migrations` 至少有 baseline 记录。
- 对已有 `meta('bkt.p_init')='0.33'` 的旧库运行 `migrate()` 后，该值仍为 `0.33`。
- `migrate()` 连跑两次后 migration count 不变。
- `polaris doctor --json` 暴露 schema version；文本输出包含 `schema_version=...`。

## 禁区

- 不实现跨设备/多用户同步。
- 不做 MCP/HTTP API 稳定性合约或 deprecation 政策。
- 不改 pack 协议、不改调度、评分、报告、育种或 trust panel 行为。
- 不重写所有历史迁移为多文件迁移系统；本票只建立当前 baseline 与未来可追加的顺序 runner。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 本轮范围（2026-06-17）

- 当前状态：P10A 已提交（`6afa7eb`），QUEUE 无 In Progress，本票按 `PRODUCT_ROADMAP.md` §7 的 P11A backlog 转正式票并认领。
- 已有非本票改动：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`、`target_codex_reviewNndQmJ/`。本票不得回退或混入。
- 预计修改面：`crates/polaris-core/src/db.rs`、`crates/polaris-core/src/ops.rs`、`crates/polaris-cli/src/main.rs`、`docs/DATA_MODEL.md`、`docs/tickets/QUEUE.md` 和本票文件。

## 交付记录（2026-06-17）

### 变更清单

- `crates/polaris-core/src/db.rs`
  - 新增 `CURRENT_SCHEMA_VERSION = 1`、固定 baseline version、`schema_migrations` 迁移账本。
  - `migrate()` 对空库和既有未版本化库幂等补齐当前 schema，写入 baseline 账本并设置 `PRAGMA user_version`。
  - `open_database()` 在设置 WAL 和迁移前先拒绝高于当前二进制支持版本的数据库，避免旧程序误写新库。
  - 新增 `schema_version()`、`schema_migration_count()` 供诊断层读取。
- `crates/polaris-core/src/error.rs`
  - 新增 `UnsupportedSchemaVersion { found, current }` 错误。
- `crates/polaris-core/src/ops.rs`
  - `DoctorReport` 增加 `schema_version` 与 `migration_count`。
- `crates/polaris-cli/src/main.rs`
  - `polaris doctor` 文本输出增加 `schema_version=` 与 `migration_count=`；JSON 自然暴露同名字段。
  - `backup` 源库打开时同样拒绝高于当前二进制支持版本的 schema，且不启用 WAL。
  - 补充 init / backup / doctor 相关测试，确认 schema version、迁移账本、备份保留版本号和高版本拒绝。
- `docs/DATA_MODEL.md`
  - 增加 schema 版本、迁移账本、旧库兼容和高版本拒绝规则。
- `docs/tickets/QUEUE.md`
  - P11A 从 backlog 转正式票，并在验收后标记完成。

### TDD 红灯

先写/扩展测试后运行，编译按预期失败，关键输出：

```text
error[E0432]: unresolved import `polaris_core::db::CURRENT_SCHEMA_VERSION`
error[E0560]: struct `DoctorReport` has no field named `schema_version`
error[E0560]: struct `DoctorReport` has no field named `migration_count`
```

随后实现 schema version、migration ledger 与 doctor 字段后，窄测试转绿。

### 审查与裁决

- 研究子agent确认现状没有正式 schema version / migration ledger，建议用 `PRAGMA user_version` 做权威版本源。
- 审查子agent发现阻塞点：`open_database()` 原实现会在拒绝未来版本库前先设置 WAL。
- 裁决与修复：把 `ensure_supported_schema_version()` 移到 `open_database()` 设置 WAL 之前，并新增文件库测试 `open_database_rejects_newer_file_database_before_wal_write`，断言高版本库报错后仍保持 `journal_mode=delete`，且不产生 WAL/SHM 文件。
- 最终审查子agent未发现必须修复项；建议补强 `backup` 高版本拒绝和 `doctor --json` 的 `migration_count` 断言。已落实：`open_existing_database()` 读取 `user_version` 并拒绝未来版本，新增 `backup_rejects_newer_schema_source_without_enabling_wal`，并补充 JSON 断言。
- 审查建议的“每个未来迁移事务化 runner”未在本票扩展：当前 P11A 只建立 baseline 与账本，未来 v2+ 追加迁移时再按版本单元补事务边界，避免提前引入无实际迁移序列的复杂度。

### 验收输出

```text
> cargo fmt --check
exit 0
```

```text
> cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.12s
```

说明：沙箱内首次 clippy 因 Windows `target` rmeta 写入权限失败：

```text
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris-57ed2e745a270c60.rmeta: 拒绝访问。 (os error 5)
```

已按权限规则提升后重跑，通过如上。

```text
> cargo test --workspace
test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

```text
> cargo test -p polaris-core db::tests
running 10 tests
test db::tests::open_database_rejects_newer_file_database_before_wal_write ... ok
test db::tests::open_database_migrates_existing_file_database_to_current_schema_version ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 64 filtered out
```

```text
> cargo test -p polaris-core ops::tests
running 2 tests
test ops::tests::ops_doctor_diagnostics_summarizes_recent_activity ... ok
test ops::tests::ops_doctor_detects_replay_mismatches_without_repairing_state ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out
```

```text
> cargo test -p polaris-cli doctor
running 3 tests
test tests::doctor_diagnose_json_keeps_doctor_and_diagnostics_separate ... ok
test tests::doctor_diagnose_json_flags_parse ... ok
test tests::backup_and_doctor_helpers_create_backup_and_report ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 59 filtered out
```

```text
> cargo test -p polaris-cli backup
running 4 tests
test tests::backup_rejects_missing_source_without_creating_it ... ok
test tests::backup_rejects_newer_schema_source_without_enabling_wal ... ok
test tests::backup_rejects_existing_output_without_overwriting ... ok
test tests::backup_and_doctor_helpers_create_backup_and_report ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 58 filtered out
```

```text
> git diff --check
exit 0
```

`git diff --check` 仅输出 Windows 换行提示，无空白错误。

真实 CLI 检查：

```text
> cargo run -p polaris-cli -- --db target/p11a-schema-final-20260617-01.sqlite init --pack packs/rust
initialized
```

```json
> cargo run -p polaris-cli -- --db target/p11a-schema-final-20260617-01.sqlite doctor --json
{
  "ok": true,
  "schema_version": 1,
  "migration_count": 1,
  "integrity_ok": true,
  "integrity_messages": [
    "ok"
  ],
  "replay_checked": 0,
  "replay_mismatches": []
}
```

```text
> cargo run -p polaris-cli -- --db target/p11a-schema-final-20260617-01.sqlite doctor
ok=true
schema_version=1
migration_count=1
integrity=ok
integrity_message=ok
replay_checked=0
replay_mismatches=0
```

### 回滚方式

- 代码回滚：`git revert <P11A 提交>`。
- 数据库回滚：本票只对打开/迁移过的库新增 `schema_migrations` 表并设置 `PRAGMA user_version=1`，保留既有业务行与用户 `meta` 值；若必须回退到 P11A 前二进制，可先使用 P11A 前备份，或在确认没有后续 schema 迁移后删除 `schema_migrations` 并执行 `PRAGMA user_version=0`。
