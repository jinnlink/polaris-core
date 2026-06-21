# P12C Capture Queue v1

状态：已实现并通过验收（2026-06-19）

服务环节：验证真懂 → 定位模糊。

## 背景

P12B 已让学习项目通过 `p-os.toml` 声明接入 P-OS。下一步需要给外部知识一个安全入口：学生或课程壳可以把“刚学到的内容”先记录进本地事实源，但它只能成为待处理资料，不能直接变成 attempt、掌握度或调度依据。

## 范围

1. 新增 `capture_queue` 最小数据结构，状态 v1 只写入 `pending`。
2. 新增 core API：`Engine::capture_learning_evidence(CaptureInput)`。
3. 新增 CLI：`polaris capture --text ... --source ...`，返回 `recorded_only` 和学生可读提示。
4. 新增 HTTP：`POST /capture`，返回 `capture_id`、`evidence_id`、`recorded_only`、`message`。
5. 更新 `DATA_MODEL.md` 与 `API_CONTRACT.md`。

## 禁区

- 不做概念自动新增。
- 不做 GUI。
- 不修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。
- 不允许 raw evidence、外部 AI 判断、`external_score`、`final_score` 或类似字段直接写入 `attempts`、`mastery_states`、`grade_queue`。
- 不做 inbox 列表、accept/defer/ignore/archive；这些留给 P12D。

## 数据模型

新增表：

```sql
capture_queue(
  id TEXT PRIMARY KEY,
  evidence_id TEXT NOT NULL,
  status TEXT NOT NULL,
  learner_kind TEXT NOT NULL,
  candidate_concept_ids_json TEXT NOT NULL DEFAULT '[]',
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(evidence_id) REFERENCES evidence_items(id)
)
```

状态枚举：`pending | mapped | practice_ready | practiced | ignored | archived`。P12C 只创建 `pending`。

资料类型枚举：`reference | own_answer | error_log | code_change | chat_excerpt | unknown`。

## 验收命令

```powershell
cargo test -p polaris-core --test p12c_capture_queue
cargo test -p polaris-cli capture
cargo test -p polaris-cli http_capture
cargo run -p polaris-cli -- --db target\p12c-capture.db capture --text "我刚看了 Rust 所有权的一段解释" --source paste
rg -n "capture_queue|recorded_only|POST /capture|polaris capture|raw evidence" crates docs
rg -n "external_score.*mastery|final_score.*capture|capture.*mastery_states|capture.*attempts" crates\polaris-core\src crates\polaris-cli\src
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

第二个 `rg` 预期可为退出码 1；它用于确认没有把 capture 与外部评分/掌握度直连。

## 回滚方式

回滚本票提交即可恢复。若手工回滚，删除 `crates/polaris-core/src/capture_queue.rs` 与 `crates/polaris-core/tests/p12c_capture_queue.rs`，撤销 `db.rs/lib.rs/engine.rs`、CLI、HTTP、`DATA_MODEL.md`、`API_CONTRACT.md`、`QUEUE.md` 和本票文档中的 P12C 修改；对已迁移数据库需在确认没有 capture 数据后删除 `capture_queue` 并将 `PRAGMA user_version` 回退到上一版。

## AI 交付记录（2026-06-19 19:21 +08:00）

- 当前状态：已完成 P12C，实现 raw evidence capture queue v1，等待用户确认后再 commit。
- 已完成：
  - 新增 `crates/polaris-core/src/capture_queue.rs` 与 `Engine::capture_learning_evidence(CaptureInput)`。
  - SQLite schema 升为 v2，新增 `capture_queue` 表、状态索引与迁移账本记录。
  - 新增 `polaris capture` 与 HTTP `POST /capture`，返回 `recorded_only=true` 与学生可读提示。
  - 新增 core/CLI/HTTP 测试，覆盖 capture 不生成 `attempts`、不写 `mastery_states`、不入 `grade_queue`，并忽略外部评分字段。
  - 更新 `DATA_MODEL.md`、`API_CONTRACT.md`、`LEARNER_CAPTURE_ROADMAP.md`、`PRODUCT_ROADMAP.md` 与 `QUEUE.md`。
- 技术选择：
  - P12C 只写 `evidence_items` + `capture_queue(status='pending')`；`candidate_concept_ids` 仅作为候选元数据，不触发概念新增或掌握度 fold。
  - `CURRENT_SCHEMA_VERSION` 从 1 提升到 2；migration count 从 1 变为 2。
  - `POST /capture` 对 `external_score`、`final_score` 等未知/外部评分字段不读取、不信任。
- 阻塞点：无。

### 验收输出

```powershell
> cargo test -p polaris-core --test p12c_capture_queue
running 2 tests
test migration_creates_capture_queue_and_records_schema_version ... ok
test captured_evidence_is_recorded_only_and_does_not_create_attempt_or_mastery ... ok
test result: ok. 2 passed; 0 failed

> cargo test -p polaris-cli capture
running 3 tests
test tests::capture_record_text_reports_recorded_only_message ... ok
test http::tests::http_capture_records_pending_item_without_attempt_or_mastery ... ok
test tests::capture_command_records_pending_item_without_attempt_or_mastery ... ok
test result: ok. 3 passed; 0 failed

> cargo test -p polaris-cli http_capture
running 1 test
test http::tests::http_capture_records_pending_item_without_attempt_or_mastery ... ok
test result: ok. 1 passed; 0 failed

> cargo test -p polaris-cli parses_required_command_set
running 1 test
test tests::parses_required_command_set ... ok
test result: ok. 1 passed; 0 failed

> cargo run -p polaris-cli -- --db target\p12c-capture.db capture --text "我刚看了 Rust 所有权的一段解释" --source paste
capture_id: a1c92faf-3c54-4c1a-ab59-c52c21f5e9c8
evidence_id: a7e98bd2-c5c9-43c5-9226-62389528cff1
status: pending
learner_kind: reference
recorded_only: true
message: 已保存为学习资料，不会直接算作掌握。

> rg -n "capture_queue|recorded_only|POST /capture|polaris capture|raw evidence" crates docs
docs\API_CONTRACT.md:86:### `POST /capture`
docs\DATA_MODEL.md:88:capture_queue(id TEXT PRIMARY KEY,
crates\polaris-core\src\capture_queue.rs:90:            Self::RecordedOnly => "recorded_only",
crates\polaris-cli\src\http.rs:468:            "POST /capture",
...

> rg -n "external_score.*mastery|final_score.*capture|capture.*mastery_states|capture.*attempts" crates\polaris-core\src crates\polaris-cli\src
退出码 1；无命中。

> cargo fmt --check
退出码 0；无输出。

> cargo clippy --workspace --all-targets -- -D warnings
error: failed to write ...\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
说明：默认 target 目录遇到 Windows 文件访问锁。

> $env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-p12c-clippy-target'; cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 52.37s

> cargo test --workspace
test result: ok. 78 passed; 0 failed
test result: ok. 80 passed; 0 failed
...
test result: ok. 2 passed; 0 failed
Doc-tests polaris_core

> git diff --check
退出码 0；仅 CRLF replacement warnings，无 whitespace error。
```
