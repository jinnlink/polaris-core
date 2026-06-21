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
然后按课程仓库自己的 today 入口带我学习。
我贴资料、笔记、错误日志或代码片段时，请用 capture_evidence 保存。
我回答问题后，再用 submit_evidence 提交作答证据。
需要看我现在的学习状态时，用 get_learner_mirror。
下一步练什么，请以 get_next_task 为本地调度参考，但课程讲解以当前仓库为主。
```

## AI 应该调用哪些工具

| 场景 | MCP tool | 说明 |
|---|---|---|
| 刚打开课程仓库 | `detect_project_manifest` | 发现 `p-os.toml`，知道课程名、默认 pack 和今天入口 |
| 学生贴资料、笔记、错误日志、代码片段 | `capture_evidence` | 只保存为 raw capture，不改变掌握度 |
| 学生完成回答或解释 | `submit_evidence` | 进入 engine-owned scoring，才会产生 attempt 和掌握度更新 |
| 学生说累了、卡住、想暂停 | `record_learner_feedback` | 记录学习状态，不直接改掌握度 |
| 想知道当前状态 | `get_learner_mirror` | 读取学习者镜像 |
| 想安排下一步练习 | `get_next_task` | 读取本地调度建议 |
| 想看系统信任面 | `get_trust_panel` | 查看验证门、实验和治理状态 |

最重要的边界：`capture_evidence` 不是“我学会了”。它只是把资料放进本地库。只有学生作答、解释、迁移应用后，才应该用 `submit_evidence`。

## 推荐学习流程

1. AI 调用 `detect_project_manifest`。
2. AI 根据 `entry.today_command` 或课程仓库约定打开今天的学习内容。
3. 学生正常学习、提问、贴代码或错误。
4. AI 用 `capture_evidence` 保存资料和现场证据。
5. AI 引导学生回答一个问题，而不是直接给结论。
6. 学生回答后，AI 用 `submit_evidence` 提交回答。
7. AI 用 `get_learner_mirror` 或 `get_next_task` 决定下一步。

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

读取学习者镜像：

```powershell
C:\MyProject\polaris-core\target\debug\polaris.exe --db C:\MyProject\polaris-data\polaris.sqlite learner-mirror --json
```

## 相关文档

- [学习项目声明协议](PROJECT_MANIFEST_PROTOCOL.md)
- [API 稳定性合约](API_CONTRACT.md)
- [票队列](tickets/QUEUE.md)
