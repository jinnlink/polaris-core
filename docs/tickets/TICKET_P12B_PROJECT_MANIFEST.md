# P12B 学习项目声明 v1

## 状态

Done（2026-06-19，通过验收）

## 服务主命题

验证真懂 -> 定位模糊。

本票让 Rust、英语、生物等学习项目可以用 `p-os.toml` 声明自己已接入 P-OS。学生在当前学习项目里说「开工」时，入口壳能先发现当前项目、默认 pack 和启动命令，不要求回到 `polaris-core`。

## 范围

1. 定义 `p-os.toml` 最小协议，明确学习项目声明不是 Domain Pack。
2. 新增 core 侧只读解析、校验和从当前目录向上发现 `p-os.toml` 的函数。
3. 新增 CLI 入口 `polaris project detect --path <dir> [--json]`，供 Aura、labctl、AI 入口壳探测当前学习现场。
4. 提供 Rust、英语、生物三个项目声明样例。
5. 更新路线图和 QUEUE：P12B 转正式票，P12C-P12G 仍为候选。

## 禁区

- 不写 capture queue。
- 不新增或修改数据库 schema。
- 不新增 HTTP/MCP 行为。
- 不修改 `C:\MyProject\Learned` 或 `C:\MyProject\Polaris`。
- 不把项目声明混同为 Domain Pack。
- 不让 `p-os.toml` 直接影响掌握度、调度或评分。

## 预计修改面

- `crates/polaris-core/src/project_manifest.rs`
- `crates/polaris-core/src/lib.rs`
- `crates/polaris-core/tests/p12b_project_manifest.rs`
- `crates/polaris-cli/src/main.rs`
- `docs/PROJECT_MANIFEST_PROTOCOL.md`
- `examples/project-manifests/*.toml`
- `docs/LEARNER_CAPTURE_ROADMAP.md`
- `docs/PRODUCT_ROADMAP.md`
- `docs/tickets/QUEUE.md`
- `docs/tickets/TICKET_P12B_PROJECT_MANIFEST.md`

## 验收

```powershell
cargo test -p polaris-core --test p12b_project_manifest
cargo test -p polaris-cli parses_required_command_set
```

预期：均通过。

```powershell
cargo run -p polaris-cli -- project detect --path examples\project-manifests\rust-mastery-lab
```

预期：输出包含 `project_id: rust-mastery-lab`、`default_pack: rust`、`entry: today`。

```powershell
rg -n "p-os.toml|学习项目声明|Domain Pack|default_pack|project detect" docs\PROJECT_MANIFEST_PROTOCOL.md examples\project-manifests docs\LEARNER_CAPTURE_ROADMAP.md docs\PRODUCT_ROADMAP.md docs\tickets\TICKET_P12B_PROJECT_MANIFEST.md
```

预期：存在匹配，退出码 0。

```powershell
rg -n "capture_queue|recorded_only|mastery_states|attempts.*p-os" crates\polaris-core\src\project_manifest.rs crates\polaris-core\tests\p12b_project_manifest.rs docs\PROJECT_MANIFEST_PROTOCOL.md
```

预期：无匹配，退出码 1。此命令不扫描本票文件，避免匹配到本节中的验收正则本身。

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

若默认 target 遭遇 Windows 文件锁，必须保留失败原文，并用隔离 `CARGO_TARGET_DIR` 同参数复核。

## 回滚方式

删除 `crates/polaris-core/src/project_manifest.rs`、`crates/polaris-core/tests/p12b_project_manifest.rs`、`docs/PROJECT_MANIFEST_PROTOCOL.md`、`examples/project-manifests/` 与本票文件；撤销 `crates/polaris-core/src/lib.rs`、`crates/polaris-cli/src/main.rs`、`docs/LEARNER_CAPTURE_ROADMAP.md`、`docs/PRODUCT_ROADMAP.md`、`docs/tickets/QUEUE.md` 中的 P12B 修改。

## AI 交接记录（2026-06-19）

- 当前状态：P12B 已按用户裁决认领。
- 已完成：初步锁定范围和验收；红灯测试已写，核心实现进行中。
- 未完成：文档样例、路线图更新、完整验收、票尾交付记录。
- 已跑验证：专项红灯测试曾因缺 `project_manifest` 模块失败，符合 TDD 预期。
- 未跑验证及原因：完整验收需实现和文档完成后运行。
- 阻塞点：无。
- 下一步建议：完成项目声明解析、CLI detect、协议文档和样例。

## 交付记录（2026-06-19）

### 变更清单

- 新增 `crates/polaris-core/src/project_manifest.rs`：解析、校验并从当前路径向上发现 `p-os.toml`；仅做只读文件解析，不接数据库。
- 新增 `crates/polaris-core/tests/p12b_project_manifest.rs`：覆盖向上发现、无声明返回 `None`、schema 版本拒绝。
- 更新 `crates/polaris-cli/src/main.rs`：新增 `polaris project detect --path <dir> [--json]`，供 Aura、labctl 或 AI 入口壳探测当前学习项目。
- 新增 `docs/PROJECT_MANIFEST_PROTOCOL.md`：定义学习项目声明协议，明确它不是 Domain Pack。
- 新增 `examples/project-manifests/{rust-mastery-lab,english-learning,biology-foundations}/p-os.toml` 三个样例。
- 更新 `docs/LEARNER_CAPTURE_ROADMAP.md`、`docs/PRODUCT_ROADMAP.md`、`docs/tickets/QUEUE.md`，标记 P12B 已转正式票并完成，P12C-P12G 仍需用户裁决。

### 技术选择

- `p-os.toml` 解析放在 core crate 的独立模块，保持入口壳、CLI 和未来 HTTP/MCP 可复用。
- 未知字段默认忽略，便于后续协议扩展；当前只支持 `schema_version = 1`。
- P12B 不写数据库、不创建 attempt、不安装或切换 pack；`default_pack` 只作为学习项目默认绑定信息。

### 验收输出

> cargo test -p polaris-core --test p12b_project_manifest

```text
running 3 tests
test project_manifest_requires_supported_schema_and_core_fields ... ok
test discovery_returns_none_when_no_manifest_exists ... ok
test discovers_nearest_p_os_manifest_by_walking_upward ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

> cargo test -p polaris-cli parses_required_command_set

```text
running 1 test
test tests::parses_required_command_set ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 74 filtered out
```

> cargo run -p polaris-cli -- project detect --path examples\project-manifests\rust-mastery-lab

```text
project_id: rust-mastery-lab
title: Rust 与软件工程训练
kind: course
default_pack: rust
entry: today
today_command: cargo run -p labctl -- today --date {today}
manifest: examples\project-manifests\rust-mastery-lab\p-os.toml
root: examples\project-manifests\rust-mastery-lab
```

> rg -n "p-os.toml|学习项目声明|Domain Pack|default_pack|project detect" docs\PROJECT_MANIFEST_PROTOCOL.md examples\project-manifests docs\LEARNER_CAPTURE_ROADMAP.md docs\PRODUCT_ROADMAP.md docs\tickets\TICKET_P12B_PROJECT_MANIFEST.md

摘录：

```text
docs\PROJECT_MANIFEST_PROTOCOL.md:1:# 学习项目声明协议 v1
docs\PROJECT_MANIFEST_PROTOCOL.md:3:`p-os.toml` 是学习项目声明。
docs\PROJECT_MANIFEST_PROTOCOL.md:5:它不是 Domain Pack。
docs\PROJECT_MANIFEST_PROTOCOL.md:66:cargo run -p polaris-cli -- project detect --path examples\project-manifests\rust-mastery-lab
examples\project-manifests\rust-mastery-lab\p-os.toml:5:default_pack = "rust"
examples\project-manifests\english-learning\p-os.toml:5:default_pack = "english"
examples\project-manifests\biology-foundations\p-os.toml:5:default_pack = "biology"
```

退出码 0。

> rg -n "capture_queue|recorded_only|mastery_states|attempts.*p-os" crates\polaris-core\src\project_manifest.rs crates\polaris-core\tests\p12b_project_manifest.rs docs\PROJECT_MANIFEST_PROTOCOL.md

输出为空；退出码 1（符合预期）。

> cargo fmt --check

输出为空；退出码 0。

> cargo clippy --workspace --all-targets -- -D warnings

默认 target 失败于 Windows 目标目录写锁，未出现 Rust/Clippy 诊断：

```text
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
```

同参数隔离 target 通过：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-p12b-clippy-target'; cargo clippy --workspace --all-targets -- -D warnings
```

```text
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.55s
```

> cargo test --workspace

输出摘要：

```text
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

> git diff --check

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-cli/src/main.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/LEARNER_CAPTURE_ROADMAP.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/PRODUCT_ROADMAP.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
```

退出码 0；只有 CRLF 提示，无 whitespace error。

### 阻塞与裁决

- 无设计或实现阻塞。
- 用户已裁决继续推进用户入口，因此 P12B 从候选拆分转正式票。
- 工作区存在票外既有脏文件：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/visuals/atlas/`、`docs/visuals/polaris-core-architecture.*`、`target_codex_reviewNndQmJ/` 等；本票未回退这些改动。

### 回滚方式

按本票“回滚方式”删除新增文件并撤销对应修改即可；若已提交，回滚 P12B 提交即可恢复。
