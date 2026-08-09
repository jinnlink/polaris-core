# Polaris Desktop

P17A 建立的 Tauri 2 产品底座。当前只有路由空壳和本地 Core 状态连接；Today、Map、Practice 等业务页面由后续 P17 票实现。

## 本地开发

```powershell
cd apps/desktop
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm tauri dev
```

Rust/Tauri 专项验证：

```powershell
cargo test -p polaris-desktop
cargo clippy -p polaris-desktop --all-targets -- -D warnings
pnpm tauri build --debug --no-bundle
```

桌面进程在系统应用数据目录持有一个 `Engine`，前端通过 Tauri command 直接调用 Core，不启动 localhost API。Rust DTO 是 TypeScript 合同的权威源；`foundation` 测试会阻止 `src/contracts/core.ts` 漂移。

安全基线：CSP 禁止远程脚本，capability 只允许 Core event 监听和 HTTPS 系统外链；前端不读 SQLite、不保存 LLM Key，也不加载遥测。
