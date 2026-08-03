# P16D Global Learner Profile 数据与治理

状态：Queued；依赖 P16A。

服务主命题：验证真懂 → 定位模糊。

## 范围

- 新增版本化画像迁移：设置、派生画像状态、验证运行和非敏感清除审计；原始回答写追加式 `behavior_events(type='profile_measurement')`。
- 画像维度保存 scope（global/pack/goal）、均值、方差、证据数、模型版本、门状态和 provenance；不得合成单一人格类型。
- 建立量表注册资源，强制 instrument/version/citation/license/locale/item/scoring/admin_mode 元数据。首批只打包可再分发 IPIP 分面和 CC BY 4.0 GSE；无许可 PALS 不进入发行物。
- 设置默认本地启用，首次说明；原始回答不经 HTTP/MCP 暴露。画像摘要的本地集成分享默认关闭。
- 实现导出、仅画像重置和全部学习数据清除的 Core/CLI/Tauri 安全边界；全部清除必须先关闭 Engine 并处理 SQLite sidecar 与本机密钥。

## 禁区

- 禁 MBTI、学习风格、诊断性或不可证伪标签。
- 本票不估计画像、不改变任务选择、不实现 UI；不得把缺许可量表题目写进仓库。
- “仅画像重置”不得删除 attempts；“全部清除”不得由无确认的 HTTP/MCP 调用触发。

## 验收

- fresh/upgrade/rollback 迁移、默认设置、许可注册校验、原始回答事件、导出、关闭/重启和两级清除测试。
- 清除失败事务安全；画像关闭后不再生成测量事件，mastery 闭环仍工作。
- SPEC §6 基线、迁移账本、`doctor`、专项测试与 `git diff --check` 全绿。

## 回滚

提交前自动备份测试库；回滚代码并按迁移账本降级。真实用户库不做破坏性自动降级，只允许旧二进制拒绝新 schema 并从备份恢复。
