# AI IDE 快速接入 Polaris

这页只解决一个问题：你已经有一个课程仓库，怎么让 AI IDE 接上 Polaris，然后开始学。

核心关系：

- 课程仓库负责教学内容和今天从哪里开始。
- AI IDE 负责读课程、和你对话、调用工具。
- Polaris 负责本地学习状态、证据、调度、收件箱和学习者镜像。

你不需要给每门课程做一个 MCP。课程仓库只要有 `p-os.toml`，同一个 Polaris MCP 就能服务它。

## 1. 生成接入包

在 `C:\MyProject\polaris-core` 运行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1
```

它会：

- 构建 `target\debug\polaris.exe`。
- 用 `target\p14c-ai-ide-kit.sqlite` 临时库初始化 Rust pack。
- 检查课程项目能发现 `p-os.toml`。
- 生成 MCP 配置、开场提示和检查清单。

如果要用真实课程仓库验证：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14c-learned-ai-ide.sqlite -OutDir target\p14c-learned-ai-ide-kit
```

脚本默认只写 `target\...`，不会写你的长期数据库。

## 2. 复制 MCP 配置

打开脚本输出的 `mcp-config.json`，把 `mcpServers.polaris-core` 配置块复制到 AI IDE 的 MCP 配置里。

脚本生成的试跑配置形状类似：

```json
{
  "mcpServers": {
    "polaris-core": {
      "command": "C:\\MyProject\\polaris-core\\target\\debug\\polaris.exe",
      "args": [
        "--db",
        "C:\\MyProject\\polaris-core\\target\\p14c-learned-ai-ide.sqlite",
        "mcp"
      ],
      "cwd": "C:\\MyProject\\Learned\\rust-mastery-lab"
    }
  }
}
```

如果你的 AI IDE 不支持 `cwd`，仍然可以保留 `command` 和 `args`，然后要求 AI 调用 `detect_project_manifest` 时显式传课程路径。

## 3. 打开课程仓库

在 AI IDE 中打开课程仓库，例如：

```text
C:\MyProject\Learned\rust-mastery-lab
```

课程仓库根目录需要有 `p-os.toml`。它只声明项目、默认 pack、今天入口和可收集证据路径，不替代课程本身。

## 4. 粘贴开场提示

把脚本输出的 `start-learning-prompt.md` 粘给 AI。

这份提示会要求 AI：

- 先调用 `detect_project_manifest`。
- 再调用 `get_ai_interaction_profile`，按你的性格、话量、解释深度、主动程度和介入频率说话。
- 贴资料时用 `capture_evidence` 保存。
- 练 inbox 资料时先 `act_on_learner_inbox_item(action=accept)`，再 `draft_inbox_practice`。
- 学生回答后用 `submit_inbox_practice`，并记录 `confidence`。
- 不把外部 AI 的评分当掌握度权威。

## 5. 做第一次自检

让 AI 按这个顺序调用工具：

```text
1. detect_project_manifest
2. get_ai_interaction_profile
3. capture_evidence
4. list_learner_inbox
5. act_on_learner_inbox_item(action=accept)
6. draft_inbox_practice
7. submit_inbox_practice
8. get_learner_mirror
```

第一次跑通后，你就可以按课程正常学习。AI 讲不清时继续问；你贴的资料、错误、笔记可以先进入 Polaris 收件箱，再被转成真正的小练习。

## 常见问题

**每个课程都要做一个 MCP 吗？**

不需要。同一个 Polaris MCP 通过 `detect_project_manifest` 识别当前课程。

**AI 说我懂了，Polaris 就会改掌握度吗？**

不会。`capture_evidence` 只是记录资料。只有学生作答、解释或迁移应用后，才通过 `submit_inbox_practice` 或 `submit_evidence` 进入引擎评分路径。

**脚本生成的是临时库，真实学习怎么办？**

先用临时库确认接入没问题。正式使用时，把 MCP 配置里 `--db` 后面的路径换成你的长期库，例如 `C:\MyProject\polaris-data\polaris.sqlite`，并先用 `polaris.exe --db <path> init --pack packs\rust` 初始化。

**AI 话太多、太少或太主动怎么办？**

让它调用 `update_ai_interaction_profile`，或者用命令行 `ai-profile set` 调整。这个设置只影响 AI 的交流方式，不影响掌握度。

## 模板位置

- `examples\ai-ide\mcp-config.template.json`
- `examples\ai-ide\start-learning-prompt.md`
- `docs\AI_IDE_USAGE.md`
- `docs\REAL_USE_SMOKE.md`
