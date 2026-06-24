# AI IDE 接入模板

这里放的是可提交、可复制的通用模板。想生成本机绝对路径版本，请在仓库根目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\ai_ide_onboarding_kit.ps1
```

生成结果会写到 `target\p14c-ai-ide-kit\`：

- `mcp-config.json`：可复制到 AI IDE MCP 配置里的 `mcpServers.polaris-core`。
- `start-learning-prompt.md`：可直接贴给 AI 的学习开场提示。
- `checklist.md`：首次接入自检清单。

模板文件：

- `mcp-config.template.json`：长期库路径示例，默认课程仓库为 `C:\MyProject\Learned\rust-mastery-lab`。
- `start-learning-prompt.md`：通用中文学习提示，强调课程主导教学、Polaris 管学习状态。
