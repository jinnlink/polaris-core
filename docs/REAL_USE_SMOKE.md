# 真实使用 smoke

这条 smoke 面向“我想确认 Polaris 真的能开始用”的场景。它不改用户默认库，只在 `target\` 下创建临时 SQLite 数据库和 transcript。

## 运行

在 `C:\MyProject\polaris-core` 执行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1
```

成功时会看到：

```text
capture_id: <uuid>
P14A real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14a-real-use-transcript.txt
```

## 它验证什么

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

## 看 transcript

完整输出在：

```text
target\p14a-real-use-transcript.txt
```

重点看这些信号：

- `project_id`、`default_pack`、`today_command`：说明课程项目声明能被发现。
- `recorded_only: true`：说明保存资料不会直接改变掌握度。
- `status: practice_ready`：说明资料进入可练习状态。
- `prompt:`：说明 Polaris 能把资料转成一道学生可答的小题。
- `attempt_id`、`provisional_score`、`degraded`：说明学生作答后才进入引擎评分路径。
- `confidence_curve`：说明 learner mirror 已能看到本次作答。

## 换成真实课程

默认脚本使用本仓库的示例项目声明。若要检测真实课程项目，只传 `ProjectPath`：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1 -ProjectPath C:\MyProject\Learned\rust-mastery-lab
```

它仍然只写 `target\p14a-real-use.sqlite`，不会修改课程仓库，也不会污染 `C:\MyProject\polaris-data\polaris.sqlite`。
