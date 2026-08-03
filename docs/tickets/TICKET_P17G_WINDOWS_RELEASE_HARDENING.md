# P17G Windows 发布硬化

状态：Queued；依赖 P17F。

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
