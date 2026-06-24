# TICKET P14D：Learned 根目录自动接入 v1

状态：已实现、通过验收

服务主命题：验证真懂 → 定位模糊 → 针对性补缺；同时服务“用户打开学习根目录就能无感接入 Polaris”。

## 背景

P14C 已经能为单个课程仓库生成 AI IDE 接入包，但用户希望直接接入 `C:\MyProject\Learned`，让 AI IDE 从学习根目录自动发现课程，而不是每次手动指定某个课程路径。

当前 `detect_project_manifest` 只会从当前目录向上查找 `p-os.toml`。如果 AI IDE 的 cwd 是 `C:\MyProject\Learned`，它不会自动发现 `rust-mastery-lab\p-os.toml`。本票补一个只读的向下扫描入口，并生成默认以 `C:\MyProject\Learned` 为 cwd 的接入包。

## 范围

- core 新增只读学习项目扫描函数：
  - 从给定 root 向下查找 `p-os.toml`。
  - 默认限制扫描深度，避免扫进嵌套工程副本。
  - 跳过 `_worktrees`、`.git`、`target` 等非课程目录。
  - 按稳定顺序返回项目列表。
- CLI 新增 `polaris project scan --root <dir> [--max-depth N] [--json]`。
- MCP 新增 `discover_learning_projects` tool，供 AI IDE 在学习根目录启动时自动发现课程项目。
- 新增 `scripts\learned_auto_connect.ps1`：
  - 默认 root 为 `C:\MyProject\Learned`。
  - 构建 `polaris-cli`。
  - 初始化 target 下临时库。
  - 调用 `project scan` 发现课程项目。
  - 生成 `mcp-config.json`、`start-from-learned-prompt.md`、`projects.json`、`checklist.md`。
  - 默认输出到 `target\p14d-learned-auto-connect\`，不写 `C:\MyProject\Learned`。
- 更新 README / AI IDE 使用指南 / API 合约 / QUEUE。

## 禁区

- 不修改 `C:\MyProject\Learned`、`C:\MyProject\Polaris`。
- 不新增长期后台服务、daemon、桌面 UI。
- 不改变掌握度、调度、评分数学或数据库 schema。
- 不让外部 AI 评分直接改掌握度。
- 不把所有子目录都当课程；必须以 `p-os.toml` 为准。

## 验收

必须真实运行并粘贴输出：

```powershell
cargo test -p polaris-core --test p12b_project_manifest scans_learning_projects_under_root
cargo test -p polaris-cli mcp_discovers_learning_projects_under_root
cargo run -p polaris-cli -- project scan --root C:\MyProject\Learned --max-depth 3
cargo run -p polaris-cli -- project scan --root C:\MyProject\Learned --max-depth 3 --json
powershell -ExecutionPolicy Bypass -File scripts\learned_auto_connect.ps1
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14d-learned-mcp-smoke.sqlite -TranscriptPath target\p14d-learned-mcp-smoke-transcript.txt
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

脚本输出必须包含：

- `P14D Learned auto-connect kit generated.`
- `learned_root: C:\MyProject\Learned`
- `projects_found:` 且至少为 1
- `default_project_id: rust-mastery-lab`
- `config:` 路径
- `prompt:` 路径
- `projects:` 路径
- `checklist:` 路径

生成的 config 必须满足：

- `mcpServers.polaris-core.command` 指向 `target\debug\polaris.exe`
- `args` 中包含 `--db` 与 `mcp`
- `cwd` 指向 `C:\MyProject\Learned`

生成的 prompt 必须要求 AI：

- 先调用 `discover_learning_projects`
- 对发现的默认项目调用 `detect_project_manifest(path=project_root)`
- 再调用 `get_ai_interaction_profile`
- 保存资料时使用 `capture_evidence`
- inbox 练习走 `act_on_learner_inbox_item(action=accept)` → `draft_inbox_practice` → `submit_inbox_practice`
- 不把外部 AI 评分当掌握度权威

## 回滚方式

- 删除 `scripts\learned_auto_connect.ps1`。
- 删除 core/CLI/MCP 的项目扫描函数、命令和 tool。
- 恢复 `docs\API_CONTRACT.md`、README、`docs\AI_IDE_USAGE.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14d-*`。

## 本轮范围（2026-06-24）

- 用户明确要求接入 `C:\MyProject\Learned`，并希望“开始就自动接入”的无感体验。
- 本票只做只读自动发现和接入材料，不修改 Learned 根目录。

## 交付记录（2026-06-24）

状态：已实现、通过验收。

### 变更清单

- core 新增 `discover_learning_projects(root, max_depth)`，从学习根目录向下只读扫描 `p-os.toml`：
  - 跳过 `_worktrees`、`.git`、`target`、`node_modules`、`.cursor` 等非课程目录。
  - 遇到一个项目 manifest 后停止下钻该项目内部，避免扫进课程里的 engine/fixture。
  - 跳过 symlink 目录，返回按 `project_root` 稳定排序的项目列表。
- CLI 新增 `polaris project scan --root <dir> [--max-depth N] [--json]`。
- MCP 新增 `discover_learning_projects` tool，并写入 API 合约稳定工具名和语义。
- 新增 `scripts\learned_auto_connect.ps1`：
  - 默认 root 为 `C:\MyProject\Learned`。
  - 输出 `target\p14d-learned-auto-connect\mcp-config.json`、`start-from-learned-prompt.md`、`projects.json`、`checklist.md`。
  - 只写 `target\` 下临时库和接入材料，不修改 `C:\MyProject\Learned`。
- 更新 README、`docs\AI_IDE_QUICKSTART.md`、`docs\AI_IDE_USAGE.md`、`docs\API_CONTRACT.md`、`docs\tickets\QUEUE.md`。
- 按用户最新要求，停止并清理了未完成的 Claude/codeagent 审查进程；后续不再使用 Claude。

### 红灯输出

```powershell
> cargo test -p polaris-core --test p12b_project_manifest scans_learning_projects_under_root
error[E0432]: unresolved import `polaris_core::project_manifest::discover_learning_projects`
```

```powershell
> cargo test -p polaris-cli mcp_discovers_learning_projects_under_root
FAILED
assertion `left != right` failed: tool returned error: "unknown tool: discover_learning_projects"
warning: unused import: `discover_learning_projects`
```

### 验收输出

```powershell
> cargo test -p polaris-core --test p12b_project_manifest scans_learning_projects_under_root

running 1 test
test scans_learning_projects_under_root ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.02s
```

```powershell
> cargo test -p polaris-cli mcp_discovers_learning_projects_under_root

running 1 test
test mcp::tests::mcp_discovers_learning_projects_under_root ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 93 filtered out; finished in 0.03s
```

```powershell
> cargo run -p polaris-cli -- project scan --root C:\MyProject\Learned --max-depth 3
root: C:\MyProject\Learned
projects_found: 1
- project_id: rust-mastery-lab
  title: Rust 与软件工程训练
  kind: course
  root: C:\MyProject\Learned\rust-mastery-lab
  manifest: C:\MyProject\Learned\rust-mastery-lab\p-os.toml
  today_command: cargo run -p labctl -- today --date {today}
```

```powershell
> cargo run -p polaris-cli -- project scan --root C:\MyProject\Learned --max-depth 3 --json
{
  "projects": [
    {
      "manifest": {
        "default_entry": "bridge_session",
        "default_pack": "rust",
        "entry": {
          "capture_label": "记录我刚学到的",
          "start_label": "继续今天",
          "stuck_label": "我卡住了",
          "today_command": "cargo run -p labctl -- today --date {today}"
        },
        "kind": "course",
        "project_id": "rust-mastery-lab",
        "schema_version": 1,
        "title": "Rust 与软件工程训练"
      },
      "manifest_path": "C:\\MyProject\\Learned\\rust-mastery-lab\\p-os.toml",
      "project_root": "C:\\MyProject\\Learned\\rust-mastery-lab"
    }
  ],
  "root": "C:\\MyProject\\Learned"
}
```

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\learned_auto_connect.ps1
P14D Learned auto-connect kit generated.
learned_root: C:\MyProject\Learned
projects_found: 1
default_project_id: rust-mastery-lab
default_project_root: C:\MyProject\Learned\rust-mastery-lab
command: C:\MyProject\polaris-core\target\debug\polaris.exe
db: C:\MyProject\polaris-core\target\p14d-learned-auto.sqlite
cwd: C:\MyProject\Learned
config: C:\MyProject\polaris-core\target\p14d-learned-auto-connect\mcp-config.json
prompt: C:\MyProject\polaris-core\target\p14d-learned-auto-connect\start-from-learned-prompt.md
projects: C:\MyProject\polaris-core\target\p14d-learned-auto-connect\projects.json
checklist: C:\MyProject\polaris-core\target\p14d-learned-auto-connect\checklist.md
```

生成的 config 关键内容：

```json
{
  "mcpServers": {
    "polaris-core": {
      "command": "C:\\MyProject\\polaris-core\\target\\debug\\polaris.exe",
      "args": [
        "--db",
        "C:\\MyProject\\polaris-core\\target\\p14d-learned-auto.sqlite",
        "mcp"
      ],
      "cwd": "C:\\MyProject\\Learned"
    }
  }
}
```

生成的 prompt 关键要求：

```text
1. Call Polaris MCP discover_learning_projects with root="C:\MyProject\Learned" and max_depth=3.
3. Call detect_project_manifest with path=<selected project_root> and confirm the selected course project.
4. Call get_ai_interaction_profile and follow its guidance for persona, verbosity, explanation depth, proactivity, intervention frequency, and correction style.
- When I paste material, notes, error logs, code snippets, or chat excerpts, save them with capture_evidence.
- If I want to practice an inbox item, first call act_on_learner_inbox_item(action=accept), then call draft_inbox_practice.
- After I answer an inbox practice item, ask for or record my confidence, then call submit_inbox_practice with my answer and confidence.
- Do not treat your own score, judgement, or encouragement as mastery authority.
```

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14d-learned-mcp-smoke.sqlite -TranscriptPath target\p14d-learned-mcp-smoke-transcript.txt
capture_id: a7cbb4e8-0367-49ce-8d26-709fb7900f9b
attempt_id: d6eb6ffc-7903-46e4-9df8-ef66cb35935e
P14B MCP real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14d-learned-mcp-smoke-transcript.txt
```

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo clippy --workspace --all-targets -- -D warnings

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.07s
```

```powershell
> cargo test --workspace

running 94 tests
...
test result: ok. 94 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s

running 80 tests
...
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
...
running 5 tests
test default_ai_interaction_profile_is_balanced_and_read_only ... ok
test update_ai_interaction_profile_trims_blank_custom_notes ... ok
test update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation ... ok
test update_ai_interaction_profile_rejects_invalid_values_without_mutation ... ok
test update_ai_interaction_profile_persists_student_preferences ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

说明：`cargo test --workspace` 完整输出较长，本票保留关键分组与尾部结果；命令已按验收实跑，退出码 0。

### 回滚方式

- 删除 `scripts\learned_auto_connect.ps1`。
- 删除 core/CLI/MCP 的 `discover_learning_projects`、`project scan` 和 MCP tool 定义。
- 恢复 README、`docs\AI_IDE_QUICKSTART.md`、`docs\AI_IDE_USAGE.md`、`docs\API_CONTRACT.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14d-*`。
