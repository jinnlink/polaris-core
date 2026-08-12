# P17F 桌面生命周期

状态：已实现、通过验收并提交（`887341f`）；依赖 P17E。

## 本轮范围（2026-08-12）

- 先审计现有 Tauri 启动、Engine 生命周期、托盘退出、grade queue、备份/doctor 与 Settings 契约，再以专项红测锁定恢复和降级语义。
- 桌面配置只保存非敏感路径与偏好；API Key 必须进入 Windows Credential Manager，所有日志与诊断导出在写盘前脱敏。
- 后台任务采用单 worker 串行调度，退出时给正在执行的安全任务明确等待/取消路径，绝不让重活占用 UI 线程或并发争用治理参数。
- 预计修改面集中在 `apps/desktop/src-tauri` 的生命周期模块、命令与测试；仅在用户确实需要控制/反馈时补 Settings 页面与前端合同。

服务主命题：全环节（本地可靠性）。

## 范围

- 数据库解析顺序锁定为：已保存路径 → `POLARIS_CORE_DB` → `%LOCALAPPDATA%\Polaris\polaris.sqlite`；首次展示实际路径并允许改选，不静默搬迁。
- 后台 worker 串行执行 grade queue、报告、巩固、拟合和备份，重活不占 Tauri UI 线程；应用事件只通知结果和失效范围。
- 开机启动默认关闭、由用户主动开启；托盘退出等待/取消安全任务并关闭 Engine。
- LLM API Key 使用 Windows Credential Manager，非敏感配置写 AppData；日志脱敏、轮转并可导出诊断包。
- 启动时处理崩溃标记、WAL/完整性检查、数据库锁定、损坏、版本过新和上次未完成后台任务。

## 禁区

- 不把密钥写 SQLite、前端 localStorage、日志或 crash dump。
- 不静默启用开机启动、遥测或远程同步。
- 不并发运行可能争用同一治理参数的夜间任务。

## 验收

- 三层 DB 解析、路径切换、凭据写读删、后台串行/取消、开机开关、崩溃恢复、锁定/损坏/新版 DB 和日志脱敏测试。
- 真实已有数据库先备份再升级 smoke；退出无残留锁。
- 桌面测试、SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

关闭后台/启动项并回滚本票；保留用户数据库，删除桌面配置与凭据需用户明确选择。

## 交付记录（2026-08-12）

### 变更清单

- 数据库启动链路按“保存路径 → `POLARIS_CORE_DB` → LocalAppData”解析；首次展示真实路径，支持无搬迁改选，并分类处理缺失、锁定、损坏、版本过新与升级前备份。损坏的桌面配置会隔离后使用默认值，不再阻断恢复入口。
- 增加单线程串行 worker，统一承载评分、报告、巩固、心智动力学拟合、参数调优、FSRS 拟合与备份；Practice、Inbox、Reports 只负责排队，应用仅在队列非空时轮询结果事件。未完成任务写入 AppData，崩溃恢复后续跑；恢复态数据库就绪前保持待办不误执行。
- Windows Credential Manager 负责三类 API Key 的写、读、删，并在真实 Desktop bootstrap 时注入 Core 调用环境；完整清除会同步删除本地凭据。开机启动默认关闭，主动开关使用 HKCU Run 可写句柄；托盘退出排空安全任务并释放 Engine，系统退出在任务边界取消排队项。
- 增加崩溃标记、轮转脱敏日志与 JSON 诊断包导出；诊断包只含平台、数据库状态、非敏感配置、待办任务与脱敏日志，不含数据库内容和密钥。
- Settings 展示真实路径/来源/schema、恢复与升级提示、启动项、凭据、后台维护和诊断导出；任一路由检测到数据库未就绪都会转入可改选路径的全局恢复工作区。

### 验收实跑

```text
cargo fmt --all -- --check
exit 0

cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

CARGO_BUILD_JOBS=1 cargo test --workspace
CLI unit: 120 passed; Core unit: 83 passed
Desktop lifecycle: 16 passed; Desktop foundation: 20 passed
其余跨阶段集成测试与 doc-tests 全部通过；0 failed
exit 0

pnpm --dir apps/desktop typecheck
exit 0

pnpm --dir apps/desktop lint
exit 0

pnpm --dir apps/desktop test -- --run
Test Files 12 passed (12); Tests 28 passed (28)
exit 0

pnpm --dir apps/desktop build
4641 modules transformed; built in 636ms
exit 0

pnpm --dir apps/desktop contracts:check
exit 0

git diff --check
exit 0
```

- 真实 Windows smoke：Credential Manager 唯一测试凭据写/读/删通过；唯一 HKCU Run 测试值默认关闭、开启、关闭与复读通过；已有 v8 SQLite 先一致性备份、再升级并保留数据；退出后数据库文件可重命名，证明无残留锁。
- 浏览器实机：1280px 与 720×512 均可到达完整生命周期面板，窄窗 `scrollWidth=705 < innerWidth=720`；控制台 0 error / 0 warn。
- workspace tests 首次并发链接因 C 盘空间不足出现 LNK1318/LNK1180，无测试断言失败；确认 `target` 为仓库内 24.13 GiB 可再生缓存后执行 `cargo clean`，以单并发干净重建并取得上述全绿结果。

### 回滚方式

- 回滚本票代码后关闭 Polaris 启动项与后台调度即可恢复旧生命周期；用户数据库、升级前备份和诊断导出文件均保留原位。
- 删除 `desktop.json` 或 Credential Manager 中的 Polaris 凭据属于用户数据操作，回滚不会自动执行；必须由用户在 Settings 明确选择。
