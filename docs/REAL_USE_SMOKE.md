# 真实使用 smoke

这条 smoke 面向“我想确认 Polaris 真的能开始用”的场景。它不改用户默认库，只在 `target\` 下创建临时 SQLite 数据库和 transcript。

## CLI smoke

CLI smoke 验证命令行入口能完成学习闭环。在 `C:\MyProject\polaris-core` 执行：


```powershell
powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1
```

成功时会看到：

```text
capture_id: <uuid>
P14A real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14a-real-use-transcript.txt
```

## MCP smoke

MCP smoke 验证 AI IDE 实际会走的 stdio 通道：脚本会启动真实 `polaris.exe --db ... mcp` 子进程，并用 JSON-RPC `Content-Length` framing 调用工具。

```powershell
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1
```

成功时会看到：

```text
capture_id: <uuid>
attempt_id: <uuid>
P14B MCP real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14b-mcp-real-use-transcript.txt
```

## CLI smoke 验证什么

脚本会按真实学习顺序跑：

1. 构建 `polaris-cli`。
2. 初始化临时数据库 `target\p14a-real-use.sqlite`。
3. 设置 AI 交互偏好。
4. 发现示例课程项目的 `p-os.toml`。
5. 保存一条学习资料，确认 `recorded_only=true`。
6. 自动解析 `capture_id`。
7. 查看学习收件箱。
8. 把资料标记为可练习。
9. 生成 inbox practice 小题。
10. 提交学生回答和 `confidence`，生成 attempt。
11. 读取 learner mirror。

## MCP smoke 验证什么

脚本会按 AI IDE 接入顺序跑：

1. 构建 `polaris-cli`。
2. 初始化临时数据库 `target\p14b-mcp-real-use.sqlite`。
3. 从课程仓库 cwd 启动真实 MCP server。
4. 调用 `initialize` 和 `tools/list`。
5. 调用 `detect_project_manifest` 发现 `p-os.toml`。
6. 调用 `update_ai_interaction_profile` / `get_ai_interaction_profile` 验证 AI 交互偏好可读写。
7. 调用 `capture_evidence` 保存资料，确认 `recorded_only=true`。
8. 调用 `list_learner_inbox` / `act_on_learner_inbox_item` / `draft_inbox_practice` / `submit_inbox_practice` 跑完收件箱练习桥。
9. 调用 `get_learner_mirror` 确认本次作答进入学习者镜像。

## 看 transcript

完整输出在：

```text
target\p14a-real-use-transcript.txt
target\p14b-mcp-real-use-transcript.txt
```

重点看这些信号：

- `project_id`、`default_pack`、`today_command`：说明课程项目声明能被发现。
- `recorded_only: true`：说明保存资料不会直接改变掌握度。
- `status: practice_ready`：说明资料进入可练习状态。
- `prompt:`：说明 Polaris 能把资料转成一道学生可答的小题。
- `attempt_id`、`provisional_score`、`degraded`：说明学生作答后才进入引擎评分路径。
- `confidence_curve`：说明 learner mirror 已能看到本次作答。

## 换成真实课程

默认脚本使用本仓库的示例项目声明。若要检测真实课程项目，CLI smoke 可以只传 `ProjectPath`；MCP smoke 建议同时传独立的临时库和 transcript，避免覆盖默认 smoke 记录：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab
powershell -ExecutionPolicy Bypass -File scripts\mcp_real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab -DbPath target\p14b-learned-mcp-real-use.sqlite -TranscriptPath target\p14b-learned-mcp-real-use-transcript.txt
```

它们仍然只写 `target\` 下的临时库和 transcript，不会修改课程仓库，也不会污染 `C:\MyProject\polaris-data\polaris.sqlite`。
