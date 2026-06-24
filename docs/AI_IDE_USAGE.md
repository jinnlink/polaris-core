# AI IDE 使用 Polaris 学习指南

这份指南面向真正使用的人：你在课程仓库里学习，AI IDE 负责读课程、提问、记录，Polaris 负责本地学习状态。

核心判断：**不用给每门课程做一个 MCP。** 课程仓库提供内容和入口，AI IDE 连接同一个 Polaris MCP，先发现当前课程项目，再调用 Polaris tools。

## 你需要准备什么

- 一个课程仓库，例如 `C:\MyProject\Learned\rust-mastery-lab`。
- 一个 Polaris Core 仓库，例如 `C:\MyProject\polaris-core`。
- 一个支持 MCP 的 AI IDE。
- 课程仓库根目录有 `p-os.toml`。

如果课程仓库还没有 `p-os.toml`，可以先复制样例：

```powershell
Copy-Item C:\MyProject\polaris-core\examples\project-manifests\rust-mastery-lab\p-os.toml C:\MyProject\Learned\rust-mastery-lab\p-os.toml
```

`p-os.toml` 只说明“这是哪个学习项目、默认 pack 是什么、今天怎么开工、哪些路径可作为学习证据”。它不是课程内容，也不是 Domain Pack。

## 第一次初始化

在 `C:\MyProject\polaris-core` 运行：

```powershell
cargo build -p polaris-cli
New-Item -ItemType Directory -Force C:\MyProject\polaris-data
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite init --pack packs\rust
.\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite ai-profile set --persona balanced_mentor --verbosity normal --explanation-depth key_steps --proactivity stuck_only --intervention-frequency normal --correction-style guided
```

确认项目声明能被发现：

```powershell
.\target\debug\polaris.exe project detect --path C:\MyProject\Learned\rust-mastery-lab
```

看到类似输出即可：

```text
project_id: rust-mastery-lab
default_pack: rust
entry: today
today_command: cargo run -p labctl -- today --date {today}
```

## 配置 AI IDE 的 MCP

推荐先用已构建的 `polaris.exe`。这样 MCP server 可以把工作目录设成课程仓库。

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

不同 AI IDE 的配置文件位置不一样，但启动命令相同：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite mcp
```

如果客户端不支持 `cwd`，让 AI 调用 `detect_project_manifest` 时显式传路径：

```json
{
  "path": "C:\\MyProject\\Learned\\rust-mastery-lab"
}
```

## 学习时怎么说

打开课程仓库后，可以先对 AI 说：

```text
请先调用 Polaris 的 detect_project_manifest，确认这个课程项目。
然后调用 get_ai_interaction_profile，按 guidance 调整你的性格、话量、解释深度、主动程度和介入频率。
然后按课程仓库自己的 today 入口带我学习。
我贴资料、笔记、错误日志或代码片段时，请用 capture_evidence 保存。
请定期用 list_learner_inbox 查看我保存过但还没处理的资料；如果我想练其中一条，用 act_on_learner_inbox_item 的 accept 标记为可转小题。
对已经 practice_ready 的资料，先用 draft_inbox_practice 生成一道小题，让我回答；我回答后，用 submit_inbox_practice 提交回答和我的 confidence。
普通课程题或你自己临时出的非 inbox 题，再用 submit_evidence 提交作答证据。
需要看我现在的学习状态时，用 get_learner_mirror。
下一步练什么，请以 get_next_task 为本地调度参考，但课程讲解以当前仓库为主。
```

## AI 应该调用哪些工具

| 场景 | MCP tool | 说明 |
|---|---|---|
| 刚打开课程仓库 | `detect_project_manifest` | 发现 `p-os.toml`，知道课程名、默认 pack 和今天入口 |
| 开始对话或用户改了 AI 风格 | `get_ai_interaction_profile` | 读取性格、话量、解释深度、主动程度、介入频率和 guidance |
| 用户要求“你少说点 / 多解释 / 主动一点 / 别老打断” | `update_ai_interaction_profile` | 更新本地交互偏好，只影响 AI 说话方式，不影响掌握度 |
| 学生贴资料、笔记、错误日志、代码片段 | `capture_evidence` | 只保存为 raw capture，不改变掌握度 |
| 想看保存过但还没处理的资料 | `list_learner_inbox` | 返回学生可读状态和 2 到 3 个可选动作 |
| 想把某条资料留到后续练习 | `act_on_learner_inbox_item` | `accept` 只标记为 `practice_ready`，不生成 attempt |
| 想把某条 `practice_ready` 资料变成一道小题 | `draft_inbox_practice` | 生成学生可答的 prompt，不生成 attempt，不暴露内部概率 |
| 学生回答 inbox 小题 | `submit_inbox_practice` | 提交回答和 `confidence`，进入 engine-owned scoring，并把条目标为 `practiced` |
| 学生完成普通课程题或非 inbox 解释 | `submit_evidence` | 进入 engine-owned scoring，才会产生 attempt 和掌握度更新 |
| 学生说累了、卡住、想暂停 | `record_learner_feedback` | 记录学习状态，不直接改掌握度 |
| 想知道当前状态 | `get_learner_mirror` | 读取学习者镜像 |
| 想安排下一步练习 | `get_next_task` | 读取本地调度建议 |
| 想看系统信任面 | `get_trust_panel` | 查看验证门、实验和治理状态 |

最重要的边界：`capture_evidence` 不是“我学会了”。它只是把资料放进本地库。`draft_inbox_practice` 也只是出一道小题。只有学生作答、解释、迁移应用后，才应该用 `submit_inbox_practice` 或 `submit_evidence`。

## AI 性格和介入频率怎么设

命令行设置：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite ai-profile set --persona socratic_tutor --verbosity detailed --explanation-depth examples_first --proactivity stuck_only --intervention-frequency normal --correction-style guided
```

常用值：

| 字段 | 可选值 | 含义 |
|---|---|---|
| `persona` | `balanced_mentor` / `socratic_tutor` / `strict_coach` / `friendly_companion` / `direct_operator` | AI 的性格或角色 |
| `verbosity` | `brief` / `normal` / `detailed` | 话多话少 |
| `explanation_depth` | `answer_only` / `key_steps` / `deep` / `examples_first` | 解释深度 |
| `proactivity` | `on_request` / `stuck_only` / `proactive` | 主动程度 |
| `intervention_frequency` | `low` / `normal` / `high` | 介入频率 |
| `correction_style` | `direct` / `guided` / `supportive` | 纠错风格 |

查看当前设置：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite ai-profile show --json
```

AI IDE 也可以通过 MCP 的 `update_ai_interaction_profile` 修改这些值。它不应该擅自修改；只有你明确说“少说点”“多解释一点”“主动提醒我”这类偏好时才改。

## 推荐学习流程

1. AI 调用 `detect_project_manifest`。
2. AI 调用 `get_ai_interaction_profile`，按 `guidance` 调整说话和介入方式。
3. AI 根据 `entry.today_command` 或课程仓库约定打开今天的学习内容。
4. 学生正常学习、提问、贴代码或错误。
5. AI 用 `capture_evidence` 保存资料和现场证据。
6. AI 用 `list_learner_inbox` 看是否有值得处理的资料，并只给 2 到 3 个选择。
7. 如果学生选择把某条资料变成练习，AI 先用 `act_on_learner_inbox_item(action=accept)` 标记，不要直接算掌握。
8. AI 用 `draft_inbox_practice` 生成一个具体问题，而不是直接给结论。
9. 学生回答后，AI 询问或记录学生 `confidence`，再用 `submit_inbox_practice` 提交回答。
10. 普通课程题仍可用 `submit_evidence`，但不要把 AI 自己的评分字段当掌握度权威。
11. AI 用 `get_learner_mirror` 或 `get_next_task` 决定下一步。

## 常见误区

- **误区：每门课程都要做一个 MCP。** 不需要。课程仓库只要声明 `p-os.toml`，AI IDE 连接同一个 Polaris MCP。
- **误区：AI 说你懂了就算掌握。** 不算。外部 AI 判断只能作为证据，掌握度由 Polaris 引擎根据 evidence-bound scoring 更新。
- **误区：保存资料会改变掌握度。** 不会。`capture_evidence` 返回 `recorded_only=true`，表示只记录。
- **误区：Polaris 要负责所有教学内容。** 不对。课程怎么教通常由课程仓库决定；Polaris 负责学习状态、调度、证据和镜像。

## 自检命令

查看项目声明：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe project detect --path C:\MyProject\Learned\rust-mastery-lab
```

记录一条资料：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite capture --text "今天学到一条所有权规则。" --source paste
```

查看学习收件箱：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite inbox list
```

把一条收件箱资料变成练习：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite inbox act --capture <capture_id> --action accept
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite inbox practice --capture <capture_id>
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite inbox submit --capture <capture_id> --response "我的解释..." --confidence 4
```

读取学习者镜像：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite learner-mirror --json
```

## 相关文档

- [学习项目声明协议](PROJECT_MANIFEST_PROTOCOL.md)
- [API 稳定性合约](API_CONTRACT.md)
- [票队列](tickets/QUEUE.md)
