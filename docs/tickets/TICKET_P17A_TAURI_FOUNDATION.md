# P17A Tauri 产品底座

状态：Queued；依赖 P16B–P16F。

服务主命题：全环节（正式产品承载）。

## 范围

- 在 `apps/desktop` 新建 Tauri 2 + React + TypeScript + Vite 应用，使用 pnpm 锁文件；Rust 桌面 crate 纳入 workspace。
- Tauri Rust 侧直接依赖 `polaris-core`，以单实例 `Engine` 和短锁命令调用 Core；前端不得经 localhost HTTP 绕行或复制业务公式。
- 建立 Rust/TypeScript 共用 DTO 生成/校验、统一错误 envelope、TanStack Query 缓存失效和 Tauri event 刷新。
- 建立 Today/Map/Practice/Inbox/Profile/Goals/Reports/Trust/Settings 路由空壳，以及 Porcelain Intelligence Atlas 设计令牌、系统字体、减弱动态和高对比基线。
- CSP 禁远程脚本，capability 最小权限，无遥测；外链交给系统浏览器。

## 禁区

- 本票不实现业务页面、托盘、更新器或后台调度。
- UI 不直接读 SQLite，不出现第二份 mastery/profile/goal 状态。
- 不把 LLM Key 写入前端 bundle、日志或普通配置文件。

## 验收

- `pnpm install --frozen-lockfile`、lint、typecheck、单元测试、前端 build、Tauri debug build。
- fresh DB/已有 DB 打开 status 命令；DTO 漂移测试；CSP/capability 静态检查。
- SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

移除 `apps/desktop`、workspace member 和桌面专用 CI；Core/数据库不受影响。
