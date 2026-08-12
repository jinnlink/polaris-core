# P17D 学习工作台

状态：Completed（用户已于 2026-08-12 确认提交）；依赖 P17B。

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

## 本轮范围（2026-08-10）

- 只实现 Practice 与 Inbox 两个学习工作区、其 Tauri DTO/命令、草稿恢复和评分事件刷新；复用现有 Core Capture、Practice Bridge、submit 与 grade queue 权威能力。
- 前端只采集原始回答、反馈前 confidence 与显式 Inbox 动作，不接受外部分数，不从 raw capture 生成 mastery，不在 UI 创建概念或边。
- 所有失败路径保留草稿，并提供 2–3 个可恢复行动；Tier 1 不可用时继续走 provisional + grade queue，不阻塞下一个学习行动。
- 验收覆盖正常练习、receipt/confidence/幂等、乐观到 final、重启恢复、Inbox 全动作、Tier 0-only/断网/错误配置/数据库忙与旧接口兼容。

## 交付记录（2026-08-10）

### 变更清单

- Core 增加可恢复的 `issue_or_resume_task`、严格 receipt 的 provisional 提交和只读评分状态；桌面提交只做本地乐观落账，Tier 1 评分留在 `grade_queue`，不阻塞用户继续学习。
- Practice 工作台贯通取题、题面、原始回答、反馈前 confidence、乐观回执、后台 final 状态、下一题与三路失败恢复；task event 和未提交草稿在应用重启后恢复。
- Inbox 工作台贯通 raw capture、列表、accept/defer/ignore/archive、draft practice、原始回答和 provisional submit；页面持续明确 `Raw capture ≠ 掌握`，只有学生亲自作答后才进入正式评分闭环。
- Tauri 增加 Practice/Inbox/grade queue DTO 与命令，provisional/final 变更向 Practice、Today、Map、Mirror 发出刷新事件；生成的 TypeScript 契约与 Rust DTO 保持同源。
- 正常练习与 Inbox 都提供 2–3 个主行动；提交失败时回答不清空，可重试、保存为资料或返回 Today。后台评分失败保持 queued，不产生未处理 Promise，也不影响本地回执。
- 保留既有 HTTP/MCP 同步接口语义；同步 Inbox 评分不会在等待外部模型时持有数据库写事务。UI 不接受外部分数、不创建概念或边。
- Practice 与 Inbox 按路由懒加载；生产构建主入口 348.57 kB，Practice 独立 chunk 14.27 kB，Inbox 独立 chunk 12.87 kB。
- 2026-08-11 交付完整性审计补齐 Practice 的回答证据 ID 与评分来源展示：provisional 明示为“本地启发式暂记”，final 明示为“evidence-bound 后台评分”；Inbox Practice 提交后保留持续可见的本地落账回执、临时结果与来源证据，不再无反馈地收起答题区。

### 验收实跑

```text
cargo fmt --check
exit 0

cargo clippy --workspace --all-targets -- -D warnings
Finished dev profile；exit 0

cargo test --workspace
全部测试组通过，0 failed；含 CLI/HTTP/MCP 120 项、Core 单测 83 项、Desktop foundation 12/12 及全部集成测试

cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test foundation -- --nocapture
test result: ok. 12 passed; 0 failed
cold start p95=169.8668ms；Today p95=570.3µs

pnpm --dir apps/desktop test
Test Files  7 passed (7)
Tests       18 passed (18)

pnpm --dir apps/desktop typecheck
exit 0

pnpm --dir apps/desktop lint
exit 0，0 warnings

pnpm --dir apps/desktop build
✓ built in 893ms；主入口 348.57 kB，Practice 14.27 kB，Inbox 12.87 kB

pnpm --dir apps/desktop contracts:check
generate-contracts --check exit 0

git diff --check
exit 0（仅 Windows LF/CRLF 提示）
```

专项覆盖：严格 task receipt、confidence 1–5、重复提交拒绝、provisional→grade queue、无 LLM/Tier 0 降级、raw capture 不生成 attempt/mastery、Inbox 全动作、旧 HTTP/MCP、草稿恢复、数据库繁忙零部分写入且原题可恢复、前端失败保留回答与三路恢复。

浏览器实机：Practice 取题→填写→confidence→本地回执与后台排队、Inbox 列表→转题→答题面板均通过；900×720 窄窗口 `scrollWidth=885 <= innerWidth=900`，无横向溢出，操作按钮不再断字。

2026-08-11 补强后复验：`cargo fmt --check`、workspace Clippy、workspace 全量测试、Desktop foundation 12/12、Vitest 7 files / 18 tests、typecheck、lint、contracts check 与生产构建全部退出 0；构建主入口 348.62 kB，Practice 12.49 kB，Inbox 13.63 kB，548ms 完成。组件测试新增断言 evidence/provenance 和 Inbox 成功回执。尝试对补强后的 localhost 页面做浏览器实机复查时被浏览器 URL 安全策略拒绝，因此不宣称本轮新增 UI 已完成浏览器视觉复验；2026-08-10 的原流程与窄窗口实机结果仍有效。

### 用户确认

- 自动验收与实机 QA 已完成；用户已于 2026-08-12 明确授权提交与推送。

### 回滚方式

- 移除 Practice/Inbox 页面、样式、开发预览 fixture 与前端命令封装，路由恢复为占位页。
- 移除 Desktop Practice/Inbox/grade queue 命令、DTO 与状态映射，并重新生成 TypeScript 契约。
- 移除 Core 的 issued-task 恢复、provisional task/inbox 提交和评分状态读取；既有同步 Capture/Practice/HTTP/MCP API 与数据库数据保持不变，不需要迁移或删表。
