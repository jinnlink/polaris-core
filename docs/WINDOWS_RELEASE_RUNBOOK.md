# Windows 发布运行手册

P17G 只把 Windows x64 发布链做到可审核的 Draft Release，不代表批准发布 1.0。最终发布裁决属于 P18B。

## 发布物与信任边界

- 安装包采用 NSIS 当前用户安装，不要求管理员权限。
- 安装器内置 WebView2 bootstrapper。Windows 10/11 通常已有 WebView2；缺失时由 bootstrapper 获取运行时。
- updater 只在 release 构建中启用。开发构建和 PR 构建不连接更新端点，也不生成签名更新物。
- Tauri updater 会在安装前验证签名；该验证不能关闭。应用先展示版本和变更说明，只有用户点击「确认下载并安装」后才开始下载。
- GitHub Release 始终先创建为 draft。人工复核安装、变更说明、hash、签名与 VM smoke 后，才允许发布。

## GitHub Environment

新建受保护环境 `windows-release`，并配置以下 Secrets：

| Secret | 用途 | 是否允许进入仓库 |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | 生成 updater detached signature | 否 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 解密签名私钥 | 否 |
| `POLARIS_UPDATER_PUBLIC_KEY` | 写入本次 release 构建配置，供客户端验签 | 可以公开，但当前统一由环境注入 |

私钥丢失后，已安装客户端无法信任新密钥。必须离线备份，不能把私钥写入 `.env`、日志、Actions artifact 或诊断包。

## PR 模式

`windows-release.yml` 在 Pull Request 中执行以下工作：

1. 运行 Rust、前端、桌面、axe、键盘、视觉回归、200% 缩放、强制颜色与性能门。
2. 构建无 updater 签名的 NSIS 安装包。
3. 生成并复核 `SHA256SUMS.txt`。
4. 上传名为 `polaris-windows-pr-unsigned` 的临时 artifact。

PR 不读取 release environment，也不接触任何签名 Secret。无 Secret 是正常状态，不是降级错误。

## 创建 Draft Release

1. 更新 `apps/desktop/src-tauri/tauri.conf.json` 中的版本，并确保 workspace 版本一致。
2. 在 GitHub Actions 手动运行 `Windows release`，输入完全匹配的标签，例如 `v0.1.0`，并填写变更说明。
3. workflow 先检查 3 个 Secret。release 环境缺任一项会立即失败，不会退回无签名更新。
4. workflow 生成临时 Tauri override config。该文件只存在于 runner 临时目录，不进入仓库或 artifact。
5. 构建完成后生成 `latest.json`、安装包 detached signature 和 `SHA256SUMS.txt`，随后创建 Draft Release。

## 干净 Windows VM smoke

测试 Windows 10 x64 与 Windows 11 x64。每个系统至少跑两台全新快照：一台验证保留数据卸载，一台验证删除数据卸载。

```powershell
./scripts/Invoke-WindowsReleaseSmoke.ps1 `
  -InstallerPath C:\release\Polaris_0.1.0_x64-setup.exe `
  -UninstallMode PreserveData `
  -ConfirmDisposableVm

./scripts/Invoke-WindowsReleaseSmoke.ps1 `
  -PreviousInstallerPath C:\release\Polaris_0.0.9_x64-setup.exe `
  -InstallerPath C:\release\Polaris_0.1.0_x64-setup.exe `
  -UninstallMode DeleteData `
  -ConfirmDisposableVm

./scripts/Invoke-WindowsUpdateFailureSmoke.ps1 `
  -BaselineInstallerPath C:\release\Polaris_0.0.9_x64-setup.exe `
  -InvalidUpdatePath C:\release\Polaris_corrupt_x64-setup.exe `
  -ConfirmDisposableVm
```

脚本验证安装注册、首次数据库、主窗口冷启动、旧数据升级保留、两种卸载语义，以及失败更新后旧二进制与数据库仍可使用。它会修改当前用户安装，只能在一次性 VM 中运行。失败更新输入应是从候选安装包复制后截断的文件，不能使用任意未知可执行文件。

默认卸载保留学习数据。交互卸载时，用户可以明确选择删除默认应用数据；静默 smoke 通过一次性 marker 表达同一选择。若用户把数据库改选到默认目录之外，卸载器不会越界删除，必须先在 Settings 使用「全部清除」。

## 更新失败与回滚

- 下载、hash 或签名失败时，不安装新版本；当前版本和数据库保持可用。
- 更新前的数据库 schema 升级由 P17F 建立备份。新版本无法启动时，先导出诊断，再回到旧二进制和升级前数据库备份。
- 未发布 Draft Release 可以撤回。已经公开的版本不能替换同版本产物或签名，只能提高版本号发布修复版。
- updater 私钥疑似泄露时，立即停止发布并撤回未发布 draft；在评估已安装客户端迁移方案前，不得临时换钥后继续发版。

## 人工审核清单

- [ ] Windows 10/11 x64 的 PreserveData 与 DeleteData smoke 全绿。
- [ ] `latest.json` 的版本、HTTPS URL 与 detached signature 正确。
- [ ] `SHA256SUMS.txt` 在独立下载目录复核通过。
- [ ] 安装器、应用图标、中文/英文安装界面和 WebView2 前置处理正常。
- [ ] 更新界面展示版本与变更说明，安装前存在明确确认动作。
- [ ] 200% 缩放、Windows 高对比、键盘全流程和 reduced-motion 可用。
- [ ] 10k 地图与冷启动性能未超预算。
- [ ] Draft Release 仍为 draft；P18B 尚未做最终发布裁决。
