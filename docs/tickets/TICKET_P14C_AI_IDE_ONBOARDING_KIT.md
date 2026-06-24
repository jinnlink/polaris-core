# TICKET P14C：AI IDE 接入实战包 v1

状态：Completed

服务主命题：验证真懂 → 定位模糊 → 针对性补缺；同时服务“用户能在 AI IDE 里实际把 Polaris 用起来”。

## 背景

P14A 已经验证 CLI 真实学习闭环，P14B 已经验证 AI IDE 走的 MCP stdio 通道。下一步不是继续扩大引擎能力，也不是做桌面 UI，而是把“怎么把 AI IDE 接到 Polaris 并开始学”落成一套普通用户能照做的实战包。

本票只做接入材料和本地自检，不改变 MCP 工具契约，不新增学习数学，不修改冻结课程仓库。

## 范围

- 新增一个 PowerShell 脚本，生成 AI IDE 接入包：
  - 构建 `polaris-cli`。
  - 使用 `target\p14c-ai-ide-kit.sqlite` 临时库初始化 Rust pack。
  - 验证课程项目路径能发现 `p-os.toml`。
  - 输出具体 `mcpServers.polaris-core` JSON 配置。
  - 输出可直接贴给 AI IDE 的学习开场提示。
  - 输出一页检查清单。
  - 默认全部写到 `target\p14c-ai-ide-kit\`，不写用户默认数据库。
- 新增可提交的模板文件：
  - 通用 MCP 配置模板。
  - AI IDE 学习开场提示模板。
- 新增或更新用户文档：
  - 一页式 AI IDE 快速接入指南。
  - README / AI IDE 使用指南入口。
- 在票尾粘贴脚本红灯、脚本实跑输出、生成文件关键内容摘要和基线验收输出。

## 禁区

- 不修改 MCP tool 列表、schema、HTTP API 或内核学习逻辑。
- 不新增桌面 UI、daemon 或长期运行服务。
- 不把外部 AI 的判断写入掌握度；仍遵守 engine-owned scoring。
- 不写用户默认数据库；脚本默认只写 `target\` 下临时库和输出文件。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不承诺某个 AI IDE 的私有配置文件路径永远不变；只提供可复制的 MCP server 配置块和通用操作步骤。

## 验收

必须真实运行并粘贴输出：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1
powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14c-learned-ai-ide.sqlite -OutDir target\p14c-learned-ai-ide-kit
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14c-learned-mcp-smoke.sqlite -TranscriptPath target\p14c-learned-mcp-smoke-transcript.txt
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

脚本输出必须包含：

- `P14C AI IDE onboarding kit generated.`
- `config:` 路径。
- `prompt:` 路径。
- `checklist:` 路径。
- `project_id: rust-mastery-lab`
- `command:` 指向 `target\debug\polaris.exe`。

生成的 config 必须包含：

- `mcpServers.polaris-core.command`
- `args` 中的 `--db` 与 `mcp`
- `cwd` 指向课程项目路径

生成的 prompt 必须要求 AI：

- 先调用 `detect_project_manifest`
- 再调用 `get_ai_interaction_profile`
- 保存资料时使用 `capture_evidence`
- 对 inbox 资料先 `act_on_learner_inbox_item(action=accept)`，再 `draft_inbox_practice`
- 学生回答后用 `submit_inbox_practice` 且记录 `confidence`
- 不把外部 AI 评分当掌握度权威

## 回滚方式

- 删除 `scripts\ai_ide_onboarding_kit.ps1`。
- 删除 `examples\ai-ide\` 下本票新增模板。
- 删除 `docs\AI_IDE_QUICKSTART.md`。
- 恢复 README、`docs\AI_IDE_USAGE.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14c-*`。

## 本轮范围（2026-06-24）

- 用户认可将下一票定为“AI IDE 接入实战包 v1”。
- 目标是把 P14A/P14B 的验证结果转成用户可照做的接入材料，而不是扩展产品架构。

## 交付记录（2026-06-24）

### 变更清单

- 新增 `scripts\ai_ide_onboarding_kit.ps1`：
  - 构建 `polaris-cli`。
  - 默认使用 `target\p14c-ai-ide-kit.sqlite` 临时库初始化 Rust pack。
  - 设置一份默认 AI interaction profile，方便 AI IDE 首次读取偏好。
  - 用 `polaris project detect --path <ProjectPath>` 验证课程仓库能发现 `p-os.toml`。
  - 生成 `mcp-config.json`、`start-learning-prompt.md`、`checklist.md`。
  - 强制 `DbPath` 与 `OutDir` 位于本仓库 `target\` 下，避免污染用户长期库。
  - 限制 `DbPath` 必须为 `.sqlite`，并拒绝 target 内已有 reparse point，避免 junction/symlink 把临时写入带到仓库外。
- 新增 `docs\AI_IDE_QUICKSTART.md`：一页式接入流程，覆盖生成接入包、复制 MCP 配置、打开课程仓库、粘贴开场提示和第一次自检。
- 新增 `examples\ai-ide\mcp-config.template.json`、`examples\ai-ide\start-learning-prompt.md` 与 `examples\ai-ide\README.md`，提供可提交的通用模板。
- 更新 `README.md`，把 AI IDE quickstart 和接入包脚本放入学生/AI IDE 使用入口，并补充普通题 `submit_evidence` 的必填字段。
- 更新 `docs\AI_IDE_USAGE.md`，加入接入包脚本、输出文件和模板目录说明；将 `p-os.toml` 复制示例改为用户自己的课程仓库，并标明 `C:\MyProject\Learned\rust-mastery-lab` 在本项目中仅作只读验证参考。
- 更新 `docs\tickets\QUEUE.md`，登记 P14C。
- 完成子代理审查补修：无 Blocking；已处理冻结仓库误写风险、普通题 `submit_evidence` 字段提示不足、target reparse point 防护不足和 quickstart 临时库示例歧义。

### 红灯输出

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1
The argument 'scripts\ai_ide_onboarding_kit.ps1' to the -File parameter does not exist.
```

### 验收输出

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1
P14C AI IDE onboarding kit generated.
project_id: rust-mastery-lab
command: C:\MyProject\polaris-core\target\debug\polaris.exe
db: C:\MyProject\polaris-core\target\p14c-ai-ide-kit.sqlite
cwd: C:\MyProject\polaris-core\examples\project-manifests\rust-mastery-lab
config: C:\MyProject\polaris-core\target\p14c-ai-ide-kit\mcp-config.json
prompt: C:\MyProject\polaris-core\target\p14c-ai-ide-kit\start-learning-prompt.md
checklist: C:\MyProject\polaris-core\target\p14c-ai-ide-kit\checklist.md
```

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14c-learned-ai-ide.sqlite -OutDir target\p14c-learned-ai-ide-kit
P14C AI IDE onboarding kit generated.
project_id: rust-mastery-lab
command: C:\MyProject\polaris-core\target\debug\polaris.exe
db: C:\MyProject\polaris-core\target\p14c-learned-ai-ide.sqlite
cwd: C:\MyProject\Learned\rust-mastery-lab
config: C:\MyProject\polaris-core\target\p14c-learned-ai-ide-kit\mcp-config.json
prompt: C:\MyProject\polaris-core\target\p14c-learned-ai-ide-kit\start-learning-prompt.md
checklist: C:\MyProject\polaris-core\target\p14c-learned-ai-ide-kit\checklist.md
```

生成的真实课程 config 关键内容：

```json
{
    "mcpServers":  {
                       "polaris-core":  {
                                            "command":  "C:\\MyProject\\polaris-core\\target\\debug\\polaris.exe",
                                            "args":  [
                                                         "--db",
                                                         "C:\\MyProject\\polaris-core\\target\\p14c-learned-ai-ide.sqlite",
                                                         "mcp"
                                                     ],
                                            "cwd":  "C:\\MyProject\\Learned\\rust-mastery-lab"
                                        }
                   }
}
```

生成的 prompt 关键要求：

```text
1. Call Polaris MCP detect_project_manifest to confirm the current course project.
2. Call get_ai_interaction_profile and follow its guidance for persona, verbosity, explanation depth, proactivity, and intervention frequency.
- When I paste material, notes, error logs, code snippets, or chat excerpts, save them with capture_evidence. This is raw capture only; it does not mean I mastered it.
- If I want to practice an inbox item, first call act_on_learner_inbox_item(action=accept), then call draft_inbox_practice.
- After I answer an inbox practice item, ask for or record my confidence, then call submit_inbox_practice with my answer and confidence.
- For ordinary course exercises or non-inbox questions you create, first call get_next_task or use the course's explicit concept, collect my confidence, then call submit_evidence with session, concept_id or concept, response, and confidence.
- Do not treat your own score, judgement, or encouragement as mastery authority. Mastery can only be updated by the Polaris engine from evidence.
```

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14c-learned-mcp-smoke.sqlite -TranscriptPath target\p14c-learned-mcp-smoke-transcript.txt
capture_id: 465830e8-57c3-4698-8d6e-bb75f78f23b7
attempt_id: ede0dabf-da22-4215-9203-cc1434bd841a
P14B MCP real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14c-learned-mcp-smoke-transcript.txt
```

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo clippy --workspace --all-targets -- -D warnings

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.76s
```

```powershell
> cargo test --workspace

running 93 tests
...
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s

running 80 tests
...
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
...
running 5 tests
test update_ai_interaction_profile_rejects_invalid_values_without_mutation ... ok
test update_ai_interaction_profile_persists_student_preferences ... ok
test default_ai_interaction_profile_is_balanced_and_read_only ... ok
test update_ai_interaction_profile_trims_blank_custom_notes ... ok
test update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

说明：`cargo test --workspace` 完整输出较长，本票保留关键分组与尾部结果；命令已按验收实跑，退出码 0。

### 回滚方式

- 删除 `scripts\ai_ide_onboarding_kit.ps1`。
- 删除 `docs\AI_IDE_QUICKSTART.md`。
- 删除 `examples\ai-ide\README.md`、`examples\ai-ide\mcp-config.template.json`、`examples\ai-ide\start-learning-prompt.md`。
- 恢复 `README.md`、`docs\AI_IDE_USAGE.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14c-*.sqlite*`、`target\p14c-*-kit\` 与 `target\p14c-*-transcript.txt`。
