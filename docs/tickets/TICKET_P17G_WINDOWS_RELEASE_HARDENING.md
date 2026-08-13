# P17G Windows 发布硬化

状态：Deferred（未完成；2026-08-13 用户裁决当前机器不重启，先推进 P12F）；依赖 P17F。

服务主命题：全环节（公开发布）。

## 范围

- 支持 Windows 10/11 x64，打包 NSIS 与 WebView2 前置处理；代码保持其他平台可移植但本票不发布其他平台。
- 建立 GitHub Actions：Rust/前端测试、Tauri build、产物 hash、安装包、签名更新清单和 GitHub Releases draft。
- Tauri updater 使用签名验证，发现更新后展示版本/变更并由用户确认安装；开发构建或无签名密钥时明确禁用。
- 完成 axe、键盘、200% 缩放、Windows 高对比、reduced-motion、视觉回归、10k 地图和冷启动性能门。
- 在干净 Windows VM 做安装、首次 DB、已有 DB 升级、保留数据卸载、删除数据卸载和更新失败回滚 smoke。

## 禁区

- 不提交私钥、token 或真实 API Key；不允许未签名生产更新。
- CI 成功不等于发布 1.0；P18B 才执行最终发布裁决。

## 验收

- 全部 Rust/前端/桌面测试；安装/卸载/更新/回滚脚本；产物签名与 hash 校验。
- 发布 workflow 可在无 secret 的 PR 模式安全跳过签名，在 release 环境缺 secret 时硬失败。
- SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

撤销 workflow/打包/更新配置并撤回未发布 draft；已发布版本只能发布更高版本修复，不能替换同版本签名产物。

## 本轮范围（2026-08-12）

- 先盘点并复用 P17A–P17F 已有桌面能力，在不改变冻结产品设计的前提下补齐发布配置、签名更新、CI、可访问性、视觉/性能门与 Windows smoke。
- 生产更新只接受签名清单；开发构建和缺少公钥的构建明确禁用更新，不提交任何私钥、token 或真实 API Key。
- 自动化覆盖可在本机和 CI 真实验证的部分；干净 Windows VM 的安装、升级、双卸载语义与失败回滚以可重复脚本和留痕清单交付，不伪造未执行的 VM 结果。
- 预计修改 `apps/desktop`、`.github/workflows`、`scripts` 与本票交付记录；票外问题只登记，不顺手扩张。

## 当前状态（2026-08-13）

### 已完成

- Tauri 2 已接入签名 updater。开发构建和未显式启用更新的构建显示禁用原因；生产更新先展示版本、日期与变更说明，再由用户确认下载和安装。
- Windows 打包固定为 x64 NSIS，安装器内置 WebView2 bootstrapper；交互式卸载支持“保留学习数据（默认）/删除默认数据”，静默删除必须带显式标记，外部数据库不在卸载器删除范围内。
- 新增 Windows 发布 workflow：PR 仅跑无签名构建与验证；受保护的 `windows-release` 环境缺少签名 secret 时硬失败；签名成功后生成 `latest.json`、SHA-256 清单、安装包并创建 Draft Release。
- 新增安装/升级/卸载 smoke 与更新失败回滚 smoke。更新失败脚本同时校验旧版本仍可启动、数据库哈希和 sentinel 不变。
- 发布门覆盖 9 个工作区的 axe、键盘跳转、Windows 强制色、reduced motion、100%/200% 视觉回归、全局 10k Pack 聚合与冷启动预算。
- 根据真实 axe 结果修正正文结构、可聚焦跳转、可信区间可读语义及低对比文本；没有改动冻结的 Atlas 产品构成。
- 已补充《Windows 发布运行手册》，明确密钥边界、PR/release 行为、干净 VM 清单、失败回滚和人工复核责任。

### 本机实跑结果

- `pnpm lint`：通过，0 warning。
- `pnpm typecheck`：通过。
- `pnpm test`：13 个测试文件、31 个测试全部通过。
- `pnpm build`：通过，Vite 生产构建 4,644 modules。
- `pnpm contracts:check`：通过。
- `pnpm test:release`（系统 Chrome）：18 个发布门全部通过，含 6 张视觉基线。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `CARGO_BUILD_JOBS=1 cargo test --workspace`：通过；基础桌面测试 20/20，其他 workspace 单元、集成与文档测试全部为 0 failed。
- `pnpm tauri build --bundles nsis`：通过，生成 `Polaris_0.1.0_x64-setup.exe`（7,268,825 bytes），SHA-256 为 `9f77e7739875627a6ba643e2f17fc18a817624a29081b8116ae5f8cd93b3a986`。
- 五个 PowerShell 发布脚本全部通过语法解析；实际无签名安装包的 hash 校验通过。
- 使用临时测试密钥完成安装包 detached signature、`latest.json`、`SHA256SUMS.txt` 全链验证，`-RequireSignature` 通过；临时私钥和公钥已删除，未进入工作树。
- `git diff --check`：通过。

### 待完成的外部发布门

- 尚未在 GitHub 受保护环境使用生产签名密钥运行 release job，因此不能把本地测试签名描述为生产签名。
- 当前机器不是一次性干净 Windows VM；安装、已有数据库升级、两种卸载语义和失败更新回滚脚本尚未在干净 VM 留下真实 transcript。
- 恢复本票后建议：先观察 PR 无 secret 路径；取得用户授权并配置受保护环境后运行 signed draft；最后在一次性 Windows 10/11 x64 VM 执行两份 smoke 脚本并回填 transcript。以上结果齐全前，P17G 保持 `Deferred` 且不视为完成。

### 用户裁决（2026-08-13）

- 用户决定暂不配置生产 updater 签名密钥。
- 禁止后续执行 AI 自行生成长期生产私钥、写入本机凭据存储或上传 GitHub Secret；恢复该步骤必须再次取得用户明确授权。
- 开发构建、无签名安装包与本地测试签名继续保持“自动更新禁用”语义，不得描述为生产发布。
- P17G 标记为 `Deferred` 且不视为完成；生产签名 Draft Release 与一次性 Windows 10/11 VM transcript 延后。用户已明确裁决允许先开发 P12F。

### 回滚方式

- 未发布时，撤销本票 workflow、updater、NSIS hook、发布脚本与测试基线即可；默认数据和外部数据库不随代码回滚删除。
- 已生成的本地安装包位于忽略的 `target/release/bundle/nsis`，可直接删除后重建。
- Draft Release 可撤回；已经公开发布的签名版本不得替换同版本产物，只能以更高版本修复。

## AI 续跑复验记录（2026-08-13）

- 当前状态：本地实现、交付完整性审计与可在本机执行的验证门均已完成；P17G 为 Deferred 且未完成，等待恢复生产签名环境和干净 Windows 10/11 x64 VM 留痕。
- 审计结论：PR job 不挂 release environment，正常不读取签名 secret；手动 release job 挂 `windows-release` 受保护环境并在构建前检查私钥、密码和公钥，缺一即失败。更新 UI 只在签名 release 构建启用，展示版本与说明后才允许用户确认安装。
- `pnpm lint`：通过，0 warning。
- `pnpm typecheck`：通过。
- `pnpm test`：13 个测试文件、31 个测试全部通过。
- `pnpm build`：通过，Vite 生产构建 4,644 modules。
- `pnpm contracts:check`：通过。
- `pnpm test:release`（系统 Chrome）：18/18 通过。
- `cargo test -p polaris-desktop --test foundation`：20/20 通过。
- 5 个 PowerShell 发布脚本：全部通过 PowerShell AST 语法解析。
- `Test-WindowsReleaseArtifacts.ps1 -RequireSignature`：现有本地签名安装包、detached signature、`latest.json` 与 SHA-256 清单校验通过。
- `cargo fmt --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `CARGO_BUILD_JOBS=1 cargo test --workspace`：沙箱外完整重跑通过，全部测试 0 failed；沙箱内曾因 HKCU 启动项测试被拒绝访问而单点失败，不作为产品失败结论。
- `git diff --check`：通过。
- 未完成：尚未使用生产签名密钥运行 GitHub signed draft job；尚未在一次性 Windows 10/11 x64 VM 执行安装、升级、PreserveData、DeleteData 与失败更新回滚并保存 transcript。不得把本地临时签名或当前机器结果替代这两项。
- 下一步建议：在得到提交/推送授权后精准提交 P17G 文件，排除漫画文档、设计图片、本机 SQLite、编辑器与规划文件；随后观察 PR 无 secret job，配置受保护环境运行 signed draft，最后执行 VM 清单并回填 transcript。
