# P07B 学习者状态镜子 v1

状态：已实现、通过验收并提交（a7e37f1）

服务主命题环节：验证真懂 -> 定位模糊

## 背景

P04A 已交付 Tier 0 状态镜子契约，P04B 已把状态镜子暴露到本地 HTTP，P07A 已把相图 enum 翻译为学习者可读语义，P07C 已让镜像报告给出 `top_signal` 与 `suggested_action`。但学习者仍缺一个打开即可理解的实时面板：我自信与实际表现是否一致、当前知识相分布是什么、最近有哪些证据约束的提醒。

本票补学习者侧状态镜子 v1。它只聚合本地只读数据，不新增 DDL，不触发 LLM，不自动生成报告，不改变掌握度、相图、调度或报告 admission 规则。

## 范围

1. Core 只读聚合契约：
   - 新增 `LearnerMirrorSnapshot`，至少包含：
     - `generated_at`
     - `confidence_curve`
     - `phase_distribution`
     - `recent_assertions`
   - `confidence_curve` 从最近 attempts 派生：
     - `self_confidence` 归一化为 `[0,1]`。
     - `actual_score = final_score ?? provisional_score`。
     - `is_final` 标明是否来自 final_score。
     - 按 `created_at` 升序输出，默认最多 30 点。
   - `phase_distribution` 复用 `status_snapshot().phase_counts` 与 P07A `Phase::label()` / `summary()`，覆盖全部相。
   - `recent_assertions` 只读取 `latest_mirror_report()`，不得调用 `run_mirror_report()`；每条带 `id`、`kind`、`claim`、`confidence`、`suggested_action`。

2. Engine / CLI / HTTP 出口：
   - `Engine::learner_mirror_snapshot() -> LearnerMirrorSnapshot`。
   - 新增 `polaris learner-mirror --json`，输出稳定 JSON；本票不要求文本 UI。
   - HTTP 新增只读 `GET /learner-mirror`，复用同一结构。
   - 不新增公网暴露，不新增 `Access-Control-Allow-Origin: *`。

3. 学习者静态面板：
   - 新增 `docs/visuals/learner-mirror/` 静态站，复用现有 `docs/visuals/atlas/` 的静态站模式，但不要修改 atlas 既有文件。
   - 站点加载 `data/sample.json` 渲染：
     - 自信 vs 实际曲线。
     - 相分布条。
     - 近期断言与行动提示。
   - sample data 必须是人工脱敏夹具，不得来自真实用户 DB。
   - 提供本地打开即可查看的 HTML；不引入 Node/Vite/Tauri 依赖。

4. 校验脚本：
   - 新增轻量 Python 校验脚本，检查 sample JSON 必要字段与静态文件存在。
   - 不依赖网络，不写真实数据。

## 预计修改面

- `crates/polaris-core/src/learner_mirror.rs`
- `crates/polaris-core/src/lib.rs`
- `crates/polaris-core/src/engine.rs`
- `crates/polaris-core/tests/p07b_learner_mirror.rs`
- `crates/polaris-cli/src/main.rs`
- `crates/polaris-cli/src/http.rs`
- `docs/visuals/learner-mirror/index.html`
- `docs/visuals/learner-mirror/styles.css`
- `docs/visuals/learner-mirror/app.js`
- `docs/visuals/learner-mirror/data/sample.json`
- `docs/visuals/learner-mirror/scripts/validate_learner_mirror.py`
- `docs/visuals/learner-mirror/README.md`
- `docs/tickets/QUEUE.md`
- 本票

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core --test p07b_learner_mirror
cargo test -p polaris-cli learner_mirror
cargo test -p polaris-cli http_learner_mirror
python docs\visuals\learner-mirror\scripts\validate_learner_mirror.py
```

专项验收要求：

- `LearnerMirrorSnapshot` 同输入同输出，且只读 SQLite。
- `confidence_curve` 不把 provisional 冒充 final；`is_final=false` 时仍可显示但语义明确。
- `recent_assertions` 读取已有 latest report；空库或无报告时返回空数组，不写库。
- `GET /learner-mirror` 返回 JSON 且不设置 `Access-Control-Allow-Origin: *`。
- 静态面板能在无构建步骤下打开，并通过校验脚本。
- 不新增 DDL，不触发 LLM，不改变 `StatusSnapshot`、相图判据、报告 admission、调度/MRT/breeding 行为。

## 禁区

- 不做 P07E 反馈通道。
- 不做 P08A 多 pack 切换。
- 不做 P10A trust panel / 五框架门状态。
- 不扩展 `StatusSnapshot` 承载时间序列，避免污染 P04A 稳定契约。
- 不自动运行 `run_mirror_report()`。
- 不把 UI 逻辑写进 `polaris-core`。
- 不引入 Tauri、Vite、Node 或网络依赖。
- 不修改冻结参考仓库。
- 不混入 `.gitignore`、`.cursor/`、`docs/polaris-core-comic-system-brief.md`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、既有 `docs/visuals/atlas/` 等预存改动。

## 开工前复述

## AI 交接记录（2026-06-16 开工）

- 当前状态：P07D 已实际提交为 `78efe0d`，但 QUEUE/票尾仍有“待 commit”文字；本轮已将 QUEUE 状态修正，并认领 P07B 为唯一 In Progress。
- 本轮范围：新增 learner mirror 只读聚合 JSON、Engine/CLI/HTTP 出口、无构建静态学习者面板、校验脚本与专项测试。
- 禁区确认：不新增 DDL；不触发 LLM；不自动生成报告；不改 `StatusSnapshot`、相图判据、报告 admission、调度/MRT/breeding；不做 P07E/P08A/P10A；不修改冻结参考仓库；不混入预存脏文件。
- 已知预存改动：`.gitignore`、`.cursor/`、`docs/polaris-core-comic-system-brief.md`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、既有 `docs/visuals/atlas/` 未跟踪文件。P07B 若新增 `docs/visuals/learner-mirror/`，提交时只 stage learner-mirror 路径。
- 验收命令：`cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace`；`cargo test -p polaris-core --test p07b_learner_mirror`；`cargo test -p polaris-cli learner_mirror`；`cargo test -p polaris-cli http_learner_mirror`；`python docs\visuals\learner-mirror\scripts\validate_learner_mirror.py`。

## AI 交付记录（2026-06-16）

- 当前状态：已实现、通过验收并提交（a7e37f1）。
- 变更清单：
  - 新增 `polaris_core::learner_mirror` 只读聚合契约：`LearnerMirrorSnapshot`、自信/实际曲线、相分布、latest report 断言摘要。
  - `Engine::learner_mirror_snapshot()` 暴露同一快照；`polaris learner-mirror --json` 使用只读数据库打开；HTTP 新增 `GET /learner-mirror`。
  - 新增 `docs/visuals/learner-mirror/` 无构建静态面板、synthetic sample、校验脚本和 README。
  - 修正 P07D 队列状态漂移：P07D 已提交为 `78efe0d`；QUEUE 认领并完成 P07B。
- 子 agent 协作：
  - 建设：Hooke 负责 core/CLI/HTTP 与 Rust 测试；Raman 负责静态 learner mirror 面板。
  - 审查：Erdos 做代码/接口审查；Mendel 做票面范围与禁区审查。
- 审查处理：
  - Erdos 发现 `generated_at` 使用 wall clock 会破坏“同输入同输出”；已改为从 attempts/mastery/concepts/latest mirror report 时间戳派生的数据水位线，空库固定 `1970-01-01T00:00:00Z`，并加确定性回归测试。
  - Erdos 发现 `suggested_action=None` 序列化时字段被省略；已改为保留字段并序列化为 `null`，并加 JSON 形状测试。
  - Mendel 确认 P07B 代码范围没有新增 DDL、没有触发 `run_mirror_report()`、没有改 `StatusSnapshot`/相图/调度/报告 admission；但提醒提交时必须只 stage P07B 白名单路径，不能混入预存脏改动。
- TDD/红灯记录：
  - 初始 core 专项红灯：缺 `Engine::learner_mirror_snapshot()`。
  - 初始 CLI/HTTP 专项红灯：缺 `learner_mirror` 模块导出、`Commands::LearnerMirror`、`learner_mirror_json()`。
  - 审查后红灯：`generated_at` 非确定性、`suggested_action` 空值字段形状不稳；均已补测试并修复。
  - `cargo clippy --workspace --all-targets -- -D warnings` 在沙箱内多次因 `target\debug\deps\libpolaris_core-*.rmeta` 写入 `拒绝访问 (os error 5)` 失败；同一原始命令在沙箱外通过，判定为 target 权限/锁问题，不是 clippy 诊断。
- 票外改动说明：
  - 工作区已有 `.gitignore`、`.cursor/`、`docs/polaris-core-comic-system-brief.md`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、既有 `docs/visuals/atlas/`、`docs/visuals/polaris-core-architecture.*` 等预存改动；本票不修改、不依赖、不提交这些路径。

### 验收输出

```powershell
cargo fmt --check
```

输出：无；退出码 0。

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

沙箱内失败摘录：

```text
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: could not compile `polaris-core` (lib) due to 1 previous error; 1 warning emitted
```

沙箱外原始命令通过：

```text
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.30s
```

```powershell
cargo test --workspace
```

真实输出摘要：

```text
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s
test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.49s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.10s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 00s
```

```powershell
cargo test -p polaris-core --test p07b_learner_mirror
```

```text
running 2 tests
test learner_mirror_snapshot_is_empty_without_attempts_or_report ... ok
test learner_mirror_snapshot_derives_curve_phases_and_latest_assertions ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

```powershell
cargo test -p polaris-cli learner_mirror
```

```text
running 4 tests
test tests::learner_mirror_json_serializes_static_panel_fields ... ok
test tests::learner_mirror_json_flag_parses ... ok
test http::tests::http_learner_mirror_returns_static_panel_snapshot ... ok
test http::tests::http_learner_mirror_stream_does_not_add_wildcard_cors ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 0.02s
```

```powershell
cargo test -p polaris-cli http_learner_mirror
```

```text
running 2 tests
test http::tests::http_learner_mirror_returns_static_panel_snapshot ... ok
test http::tests::http_learner_mirror_stream_does_not_add_wildcard_cors ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out; finished in 0.02s
```

```powershell
python docs\visuals\learner-mirror\scripts\validate_learner_mirror.py
```

```text
learner mirror validation passed
```

补充静态检查：

```powershell
node --check docs\visuals\learner-mirror\app.js
git diff --check
```

`node --check` 无输出，退出码 0；`git diff --check` 退出码 0，仅有 CRLF 提示。

### 提交前复核输出（2026-06-17）

```powershell
cargo fmt --check
```

输出：无；退出码 0。

```powershell
python docs\visuals\learner-mirror\scripts\validate_learner_mirror.py
```

```text
learner mirror validation passed
```

```powershell
node --check docs\visuals\learner-mirror\app.js
git diff --check
```

`node --check` 无输出，退出码 0；`git diff --check` 退出码 0，仅有 CRLF 提示。

```powershell
cargo test -p polaris-core --test p07b_learner_mirror
```

```text
running 2 tests
test learner_mirror_snapshot_is_empty_without_attempts_or_report ... ok
test learner_mirror_snapshot_derives_curve_phases_and_latest_assertions ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

```powershell
cargo test -p polaris-cli learner_mirror
```

```text
running 4 tests
test tests::learner_mirror_json_serializes_static_panel_fields ... ok
test tests::learner_mirror_json_flag_parses ... ok
test http::tests::http_learner_mirror_returns_static_panel_snapshot ... ok
test http::tests::http_learner_mirror_stream_does_not_add_wildcard_cors ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out; finished in 0.01s
```

```powershell
cargo test -p polaris-cli http_learner_mirror
```

```text
running 2 tests
test http::tests::http_learner_mirror_returns_static_panel_snapshot ... ok
test http::tests::http_learner_mirror_stream_does_not_add_wildcard_cors ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out; finished in 0.01s
```

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.53s
```

```powershell
cargo test --workspace
```

真实输出摘要：

```text
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
...
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
```

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md docs/tickets/TICKET_P07D_ACTION_LOOP.md crates/polaris-core/src/lib.rs crates/polaris-core/src/engine.rs crates/polaris-cli/src/main.rs crates/polaris-cli/src/http.rs
Remove-Item crates/polaris-core/src/learner_mirror.rs
Remove-Item crates/polaris-core/tests/p07b_learner_mirror.rs
Remove-Item -Recurse docs/visuals/learner-mirror
Remove-Item docs/tickets/TICKET_P07B_LEARNER_MIRROR.md
```

提交后：

```powershell
git revert <P07B-commit-sha>
```
