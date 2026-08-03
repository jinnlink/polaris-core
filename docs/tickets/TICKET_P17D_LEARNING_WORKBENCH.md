# P17D 学习工作台

状态：Queued；依赖 P17B。

服务主命题：验证真懂 → 针对性补缺。

## 范围

- Practice 页面贯通取题、题面、原始回答、反馈前 confidence、乐观回执、后台评分修正和 evidence/provenance。
- Inbox 页面贯通 Capture、列表、accept/defer/ignore/archive、draft practice 和 submit；raw capture 始终显示“尚不算掌握”。
- 每个入口提供 2–3 个行动；失败时保留草稿并给重试/保存资料/返回 Today，而不是丢失回答。
- 评分修正通过 Tauri event 更新 Today/Map/Mirror；不得阻塞用户等待 Tier 1。
- 支持 Tier 0-only、无网络、LLM 配置错误、grade queue 和应用重启后的恢复。

## 禁区

- 不信任前端或外部 AI 分数；不允许 raw capture 直接生成 mastery。
- 不在 UI 生成不可审计概念/边；P12F 前只使用现有 candidate hints。

## 验收

- 正常练习、严格 task receipt、confidence 校验、重复提交、乐观→final 修正、崩溃草稿恢复和全部 Inbox 动作测试。
- 无 LLM/断网/错误 Key/数据库忙/后台失败 smoke；旧 HTTP/MCP 兼容。
- 前端/桌面测试、SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

回滚页面和 Tauri 命令绑定；Core Capture/Practice/MCP 数据保持可用。
