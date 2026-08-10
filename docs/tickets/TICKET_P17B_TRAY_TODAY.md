# P17B 常驻小窗与 Today

状态：已提交（`a458b5b`）；依赖 P17A。

服务主命题：定位模糊 → 针对性补缺。

## 范围

- 实现 Windows 单实例、系统托盘、常驻小窗与展开工作区；关闭窗口默认隐藏到托盘，托盘“退出”才终止进程。
- Today 消费 Status、Mirror top signal、active Pack 和本地 scheduler，始终呈现 2–3 个合法行动；无足够任务时给“补证据/处理收件箱/休息”降级选择。
- 支持 Pack 切换与 shared/isolated 标识；切换后统一失效地图、Today、Goals 和 Inbox 查询。
- 心流 HMM 过门且当前为 flow 时抑制所有非错误通知；未过门只记录，不抑制。
- 小窗只读 Tier 0，打开路径不触发 LLM、网络或夜间任务。

## 禁区

- 不实现完整 Map/Practice/Profile 页面；不做单一强制行动。
- 不把 tray 生命周期写入 Core；不在同步打开路径跑重查询。

## 验收

- 单实例、托盘隐藏/恢复/退出、小窗/工作区切换、Pack 切换、2–3 行动、零任务和 flow 抑制测试。
- 冷启动 p95 与 Tier 0 命令预算记录；无 LLM/断网 smoke。
- 前端/桌面测试、SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

回滚本票桌面壳与托盘事件；P17A 基础应用仍可启动。

## 本轮范围（2026-08-09）

- 只实现 Windows 单实例/托盘/小窗生命周期、Today Tier 0 聚合、Pack 切换与 flow 通知抑制。
- Map/Practice/Profile 等完整工作区仍保持空壳；不记录 tray 生命周期，不在打开路径调用 LLM、网络或后台任务。
- 专项门覆盖壳生命周期、Pack 切换全失效、Today 2–3 行动/零任务降级、flow 门和冷启动/Tier 0 预算。

## 交付记录（2026-08-09）

### 变更清单

- Windows 桌面壳启用 single-instance、系统托盘与通知插件；关闭主窗口只隐藏，托盘支持打开、小窗、工作区和退出，第二实例恢复并聚焦已有窗口。
- 小窗/工作区分别固定为 420×640 常驻置顶和 1180×760 可调整布局；窗口生命周期只存在于 Tauri 壳，不写 Core 事实表。
- 新增 Today Tier 0 聚合：消费 Status、Mirror top signal、active Pack 与本地 scheduler，始终返回 3 个行动；任务不足时依次补证据、处理收件箱和休息。
- 新增 Pack 选择器和 shared/isolated 标识；切换成功统一广播 Map、Today、Goals、Inbox 四域失效事件。
- Core 新增只读通知策略：仅最新 HMM 状态已过 `strategy_enabled` 门且 dominant state 为 flow 时抑制非错误通知，错误通知永不抑制。
- 扩展 Rust→TypeScript 生成合同并加入漂移检查；Today 页面和测试覆盖三行动、top signal、Pack 切换、休息及心流保护提示。

### 实跑验收

- `pnpm --dir apps/desktop contracts:check`、`lint`、`typecheck`、`build` → 全部通过；`pnpm --dir apps/desktop test` → 4 files，7 passed，0 failed。
- `cargo test -p polaris-desktop` → unit 2/2、foundation 8/8；覆盖壳效果、fresh/已有 DB、三行动/零任务、Pack 切换、flow 抑制、DTO/CSP 和预算。
- 20 次独立 fresh DB 冷启动实测 p95 `573.6652ms`；20 次 Today Tier 0 读取 p95 `869.1µs`，分别低于专项门的 2s 与 250ms 预算。
- `POLARIS_TIER0_ONLY=1` 真实进程 smoke → 首实例持续运行、第二实例 10s 内退出、活跃实例数 1；全过程不请求 LLM 或网络。
- `pnpm --dir apps/desktop tauri build --debug --no-bundle` → `Finished dev profile ... in 2m 41s`，生成 `target/debug/polaris-desktop.exe`。
- `cargo fmt --check` 与 `git diff --check` → 通过（无差异错误）。
- `cargo clippy --workspace --all-targets -- -D warnings` → 干净构建通过，`Finished dev profile ... in 7m 31s`。
- `cargo test --workspace` → CLI 120/120、Core 83/83、Desktop unit 2/2 + foundation 8/8，其余集成测试和 doc-tests 全部 0 failed。

### 验收过程记录

- 首轮完整测试因仓库 `target` 历史产物将磁盘写满而中止，错误为 `no space on device`，不是测试断言失败。
- 清理可再生成的 `target` 49.3GiB 后，以 `CARGO_INCREMENTAL=0`、`CARGO_BUILD_JOBS=1` 从干净环境重跑 Clippy、完整测试和桌面构建，结果全绿。

### 技术选择

- Today 只聚合已有 Core 读模型和 scheduler assignment，不在前端复制调度、mastery、画像或目标公式。
- 通知抑制统一经过 Rust command 的 Core policy，前端和调用者无法绕过 flow 门；错误级别明确保留。
- 窗口意图先映射为纯 `ShellEffect` 再执行 Tauri 副作用，使关闭/恢复/退出语义可测试且不污染 Core。

### 回滚方式

- 使用 `git revert <P17B 提交哈希>` 回滚本票；会移除托盘/单实例/通知插件、Today 页面与通知策略，P17A 基础应用和 Core 既有数据库保持可用。
- 本票不新增 schema，不写 tray 生命周期；回滚无需迁移或修复用户数据。
