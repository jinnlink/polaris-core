# TICKET P13B - README 与 AI IDE 使用指南

状态：**已实现、通过验收并提交**

## 背景

P13A 已把项目发现、raw capture 和学习者镜像暴露给 Polaris MCP。现在 README 仍主要停留在 P01 命令行闭环，用户不知道在真实课程仓库里怎样让 AI IDE、课程项目和 Polaris 配合起来。

本票补齐面向使用者的入口文档，避免用户把 Polaris 误理解成需要手动操作的内部工程。

## 服务主命题

- 验证真懂：说明 `submit_evidence` 只在学生作答后使用，raw capture 不等于掌握。
- 定位模糊：说明 `get_learner_mirror` 和 `get_next_task` 在学习过程中的作用。
- 针对性补缺：说明 AI IDE 如何先读课程项目，再调用 Polaris tools 辅助学习。

## 范围

1. 更新 `README.md`：
   - 加入“学生 / AI IDE 怎么用”的最短路径。
   - 给出 MCP 配置示例。
   - 明确不需要每门课程各做一个 MCP。
   - 更新文档地图。
2. 新增 `docs/AI_IDE_USAGE.md`：
   - 说明课程仓库应放 `p-os.toml`。
   - 说明如何初始化 Polaris 数据库和 pack。
   - 说明如何配置 MCP server。
   - 说明 AI IDE 应调用哪些 tools，以及学生应该怎样提问。
3. 轻量更新 `docs/PROJECT_MANIFEST_PROTOCOL.md`，补充使用指南入口。

## 禁区

- 不改代码。
- 不改 P13A MCP 工具语义。
- 不修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。
- 不把内部概念包装成学生必须理解的流程。

## 验收命令

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 回滚方式

回滚本票对 `README.md`、`docs/AI_IDE_USAGE.md`、`docs/PROJECT_MANIFEST_PROTOCOL.md`、`docs/tickets/QUEUE.md` 和本票文件的改动即可。

## AI 交付记录（2026-06-21）

- 当前状态：已实现、通过验收并提交。
- 已完成：
  - `README.md` 新增学生 / AI IDE 最短路径、5 分钟本地试跑、MCP 配置示例和 AI 开场提示词。
  - 新增 `docs/AI_IDE_USAGE.md`，说明课程仓库、`p-os.toml`、Polaris 数据库、MCP server、tools 调用边界和常见误区。
  - `docs/PROJECT_MANIFEST_PROTOCOL.md` 增加 AI IDE 使用指南入口。
  - `docs/tickets/QUEUE.md` 已更新为无 In Progress。
- 禁区遵守：
  - 未改代码。
  - 未改 P13A MCP 工具语义。
  - 未修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。

### 验收输出

```powershell
cargo fmt --check
```

```text
Exit code: 0
```

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.34s
```

```powershell
cargo test --workspace
```

输出摘录：

```text
test result: ok. 81 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

### 回滚方式

回滚本票修改文件：`README.md`、`docs/AI_IDE_USAGE.md`、`docs/PROJECT_MANIFEST_PROTOCOL.md`、`docs/tickets/QUEUE.md`、`docs/tickets/TICKET_P13B_README_AI_IDE_USAGE.md`。
