# polaris-core

个人学习引擎内核（Rust）。主命题：**验证真懂 → 定位模糊 → 针对性补缺**。

这是 Polaris（`C:\MyProject\Polaris`）与 rust-mastery-lab（`C:\MyProject\Learned`）两次实现的内核提取与重构。两个旧仓库已冻结为只读参考。

## 当前状态

- Phase 0（设计与交接包）：**完成** —— 设计冻结于 `docs/MASTER_PLAN.md`，宪法在 `SPEC.md`。
- Phase 1（walking skeleton）：**P01 已实现并完成子 agent 审查补修** —— 票在 `docs/tickets/TICKET_P01_WALKING_SKELETON.md`。
- Phase 12/13（学习入口）：**项目声明、Capture Queue、Learner Inbox、Inbox Practice Bridge、AI IDE MCP 入口、AI 交互偏好已实现** —— 课程仓库通过 `p-os.toml` 声明自己，AI IDE 连接同一个 Polaris MCP 即可辅助学习。

## 学生 / AI IDE 怎么用

你不需要为每门课程各写一个 MCP。推荐用法是：

1. 在课程仓库放一个 `p-os.toml`，声明这是学习项目。
2. 在 AI IDE（Codex、Claude Code、Cursor 等支持 MCP 的工具）里打开课程仓库。
3. AI IDE 连接 `polaris-core` 的 MCP server。
4. AI 先调用 `detect_project_manifest` 理解当前课程，再用 Polaris tools 记录证据、安排练习、读取学习镜像。

最短可用流程见 [AI IDE 快速接入](docs/AI_IDE_QUICKSTART.md)；完整说明见 [AI IDE 使用指南](docs/AI_IDE_USAGE.md)。

如果你想打开 `C:\MyProject\Learned` 后让 AI 自动发现里面的课程项目，推荐先生成 Learned 根目录接入包：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\learned_auto_connect.ps1
```

生成内容会在 `target\p14d-learned-auto-connect\`；MCP 配置的 `cwd` 指向 `C:\MyProject\Learned`，AI 开场后先调用 `discover_learning_projects`，再接入具体课程。

如果你想为单个课程仓库生成本机可复制的 MCP 配置、学习开场提示和检查清单：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14c-learned-ai-ide.sqlite -OutDir target\p14c-learned-ai-ide-kit
```

脚本默认只写 `target\` 下临时库和输出文件，不碰用户长期数据库或 `C:\MyProject\Learned`。

想先确认本机闭环能跑通，可以运行 [真实使用 smoke](docs/REAL_USE_SMOKE.md)：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1
```

## 5 分钟本地试跑

以下命令在 `C:\MyProject\polaris-core` 运行：

```powershell
cargo build -p polaris-cli
New-Item -ItemType Directory -Force C:\MyProject\polaris-data
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite init --pack packs\rust
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite ai-profile set --persona socratic_tutor --verbosity detailed --explanation-depth examples_first --proactivity stuck_only --intervention-frequency normal --correction-style guided
.\target\debug\polaris.exe project detect --path examples\project-manifests\rust-mastery-lab
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite capture --text "我刚看了一段所有权解释：一个值同一时刻只有一个 owner。" --source paste --candidate-concept ownership
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite inbox list
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite learner-mirror --json
```

你应该看到：

- `project detect` 输出 `project_id`、`default_pack`、`today_command`。
- `ai-profile set/show` 让你设置 AI 的性格、话量、解释深度、主动程度和介入频率；这些偏好只影响外部 AI 说话方式，不改变掌握度。
- `capture` 输出 `recorded_only: true`，表示资料已保存，但不会直接算作掌握。
- `inbox list` 输出学习收件箱，给出“转成一道小题 / 稍后再看 / 忽略”等学生动作。
- 想练其中一条时，先运行 `inbox act --capture <capture_id> --action accept`，再运行 `inbox practice --capture <capture_id>` 生成小题。
- 学生回答后，运行 `inbox submit --capture <capture_id> --response "..." --confidence 4`；这时才会生成 attempt 并进入引擎自有评分路径。
- `learner-mirror --json` 输出学习者镜像字段。

## AI IDE MCP 配置示例

先构建二进制：

```powershell
cargo build -p polaris-cli
```

然后在 AI IDE 的 MCP 配置中加入类似配置。字段名会因客户端不同而略有差异，但核心是启动 `polaris.exe --db <数据库> mcp`。

```json
{
  "mcpServers": {
    "polaris-core": {
      "command": "C:\\MyProject\\polaris-core\\target\\debug\\polaris.exe",
      "args": [
        "--db",
        "C:\\MyProject\\polaris-data\\polaris.sqlite",
        "mcp"
      ],
      "cwd": "C:\\MyProject\\Learned\\rust-mastery-lab"
    }
  }
}
```

如果你的 AI IDE 不支持给 MCP server 设置 `cwd`，也可以让 AI 调用：

```json
{
  "path": "C:\\MyProject\\Learned\\rust-mastery-lab"
}
```

对应 tool：`detect_project_manifest`。

## AI 应该怎么配合学生

给 AI 的开场提示可以直接复制：

```text
你现在是我的学习助手。请先调用 Polaris MCP 的 detect_project_manifest，确认当前课程项目。
然后调用 get_ai_interaction_profile，按其中 guidance 调整你的性格、话量、解释深度和介入频率。
学习过程中，不要直接判断我“掌握了”。如果我只是贴资料、笔记、错误日志或代码片段，请用 capture_evidence 保存为学习资料。
请定期用 list_learner_inbox 查看我保存过但还没处理的资料；如果我想练其中一条，用 act_on_learner_inbox_item 的 accept 标记为可转小题。
对已经 practice_ready 的资料，先用 draft_inbox_practice 生成一道小题，让我回答；我回答后，用 submit_inbox_practice 提交回答和我的 confidence。
只有普通课程题或你自己出的非 inbox 题，才用 submit_evidence 提交作答证据；先用 get_next_task 拿到 concept_id，或使用课程明确给出的概念，并提交 session、concept_id/concept、response、confidence。
需要了解我当前状态时，用 get_learner_mirror；需要安排下一步练习时，用 get_next_task。
课程怎么教以当前仓库为主，Polaris 负责记录证据、调度和学习者镜像。
```

## 命令行闭环自测

```powershell
cargo run -p polaris-cli -- pack validate packs/rust
cargo run -p polaris-cli -- --db target\p01-quickstart.db init --pack packs/rust
cargo run -p polaris-cli -- --db target\p01-quickstart.db next --session quickstart
cargo run -p polaris-cli -- --db target\p01-quickstart.db submit --concept ownership --response "Ownership controls which binding can drop a value." --confidence 4 --session quickstart
cargo run -p polaris-cli -- --db target\p01-quickstart.db status
cargo run -p polaris-cli -- --db target\p01-quickstart.db grade-pending
```

实跑要点：

- `pack validate` 输出：`pack ok: concepts=24 prerequisites=21 misconceptions=11`
- `next` 首题：`concept: ownership`，`task_type: recall`
- `submit` 输出：`provisional_score=0.700 degraded=true`
- `grade-pending` 输出：`processed=0 pending=1`

## 给人类

读 `SPEC.md` 前两节即可知道这是什么；想看全部设计读 `docs/MASTER_PLAN.md`。
开工方式：把下面的启动提示词发给你的执行 AI（Codex/GPT 等）。更完整的新窗口续跑规则见 `docs/AI_RUNBOOK.md`。

## 执行 AI 启动提示词（复制即用）

```text
请读取 AGENTS.md 和 docs/AI_RUNBOOK.md，并严格遵守其中的阅读顺序、工作纪律和新窗口续跑协议。
然后读 docs/tickets/QUEUE.md。如果已有 In Progress 票，只续做那张票；如果没有，认领下一张票并按单票制实现。
设计已冻结：不要再设计、不要引入新概念、不要做票范围外的事。
有歧义就在票内记录阻塞点和你的建议方案，停下来请我裁决。
完成的定义 = SPEC §6 验收基线全绿且实跑输出贴在票尾。
```

## 文档地图

| 文件 | 作用 |
|---|---|
| `SPEC.md` | 工程宪法：主命题、三支柱、铁律、边界、验收基线（冲突时它赢） |
| `AGENTS.md` | 执行 AI 的入口：阅读顺序、纪律、移植参考映射 |
| `docs/AI_RUNBOOK.md` | 新窗口续跑手册：启动提示词、接手清单、交接模板、防错规则 |
| `docs/ENHANCEMENT_ROADMAP.md` | P03E+ 增强优先级；姊妹文档见 `C:\MyProject\Learned\rust-mastery-lab\docs\ENHANCEMENT_ROADMAP.md` |
| `docs/MASTER_PLAN.md` | 完整设计蓝图（含抽象引擎、心智动力学引擎、五个原创框架、教学法纲要 v3、分阶段与验证门） |
| `docs/DATA_MODEL.md` | 表结构 DDL 与全部公式（实现的直接依据） |
| `docs/AI_IDE_QUICKSTART.md` | AI IDE 快速接入 Polaris：生成配置、复制 MCP server、粘贴开场提示、做第一次自检 |
| `docs/AI_IDE_USAGE.md` | AI IDE 接入 Polaris MCP 的使用指南：课程仓库、`p-os.toml`、MCP 配置和学习流程 |
| `docs/REAL_USE_SMOKE.md` | 一键真实使用 smoke：init、AI profile、项目声明、capture、inbox、practice、submit、learner mirror |
| `docs/PROJECT_MANIFEST_PROTOCOL.md` | `p-os.toml` 学习项目声明协议 |
| `docs/API_CONTRACT.md` | HTTP 与 MCP 对外稳定契约 |
| `docs/tickets/QUEUE.md` | 票队列（单票制，P01→P05 对应 Phase 1→5） |
