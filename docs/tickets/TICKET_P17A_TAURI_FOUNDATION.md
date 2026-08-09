# P17A Tauri 产品底座

状态：已实现并通过验收，等待提交；依赖 P16B–P16F。

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

## 本轮范围（2026-08-09）

- 只建立 `apps/desktop` 工程、Core 直连命令与 DTO 校验、路由空壳、设计/无障碍/安全基线和必要 CI。
- 不实现业务页面、托盘、更新器、后台调度，不经 localhost HTTP，不复制 Core 公式或数据真相。
- 验收包含 pnpm 锁定安装、lint/typecheck/test/build、Tauri debug build、fresh/旧库 status、DTO 漂移及 CSP/capability 静态检查，以及 SPEC §6 Rust 基线。

## 交付记录（2026-08-09）

### 变更清单

- 新建 `apps/desktop`：Tauri 2.11 + React 19 + TypeScript 6 + Vite 8，pnpm 锁文件和 Windows 桌面 CI；Rust 桌面 crate 纳入 workspace。
- 桌面进程在应用数据目录持有唯一 `Mutex<Engine>`，`status` command 在工作线程短锁直调 Core，不启动 localhost HTTP。
- Core `StatusSnapshot` 通过可选 `desktop-bindings`/`ts-rs` 生成 TypeScript 合同；漂移测试、统一 `CommandError` envelope、TanStack Query 和 `polaris://data-changed` 失效通道已建立。
- Today/Map/Practice/Inbox/Profile/Goals/Reports/Trust/Settings 九个路由空壳、Porcelain Intelligence Atlas 瓷白/玉色令牌、系统字体、跳转链接、减弱动态和 forced-colors 基线已落地。
- CSP 禁远程脚本，capability 仅授权事件监听和 HTTPS 系统外链；无 SQLite/FS/shell/process 前端权限，无遥测。

### 实跑验收

- `pnpm install --frozen-lockfile` → `Already up to date`，通过。
- `pnpm lint` → 通过，0 warnings；`pnpm typecheck` → 通过。
- `pnpm test` → 3 files，5 passed，0 failed。
- `pnpm build` → Vite 86 modules，产物 `dist/index.html` + CSS/JS，通过。
- `pnpm tauri build --debug --no-bundle` → `Finished dev profile ... in 20.76s`，生成 `target/debug/polaris-desktop.exe`。
- `cargo test -p polaris-desktop` → Rust unit 1/1、foundation 4/4；覆盖 fresh/已有 DB status、DTO 漂移、CSP/capability/无遥测。
- `cargo fmt --all -- --check` 与 `git diff --check` → 通过（无输出）。
- `cargo clippy --workspace --all-targets -- -D warnings` → 通过，`Finished dev profile ... in 22.31s`。
- `cargo test --workspace` → CLI 120/120、Core 81/81、Desktop unit 1/1 + foundation 4/4，其余集成测试和 doc-tests 全部 0 failed。

### 技术选择

- DTO 以 Core Rust struct 为权威源，TypeScript 文件只是可验证生成物，避免维护第二份状态或业务公式。
- 路由使用 hash history，保持 Tauri 静态资源刷新可用；外链统一经 opener 交给系统浏览器。

### 回滚方式

- 使用 `git revert <P17A 提交哈希>`，或移除 `apps/desktop`、`.github/workflows/desktop.yml`、workspace member 及 Core `desktop-bindings` 可选 derive。
- P17A 未修改 schema 或业务数据；移除桌面程序不影响现有 Core/CLI/数据库。
