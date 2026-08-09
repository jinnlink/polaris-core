# P16D Global Learner Profile 数据与治理

状态：已实现并通过验收，已获用户提交确认；依赖 P16A（`fbcad7b`）。

## 本轮范围（2026-08-08）

- 建立 schema v3 画像治理底座、强校验量表注册、追加式原始回答事件、导出和两级清除边界。
- 优先保障用户能清楚看到“是否启用、是否已说明、是否向本地集成分享、收集了什么、怎么导出/重置/清除”，不用单一人格类型或审计噪声代替可用治理。
- 不实现 P16E 的画像估计、EMA 频率、验证门计算或调度消费；不实现 P17E UI。

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

提交前自动备份测试库。代码回滚使用本票提交的反向提交；真实用户库不做破坏性自动降级，旧二进制必须拒绝 schema v3，并从用户确认保留的备份恢复。

## 交付记录（2026-08-08）

- SQLite schema 升至 v3；画像设置、分维度派生状态、验证运行、非敏感重置记录、三个查询索引、迁移账本与 `user_version` 在同一事务提交。fresh、v2 upgrade、幂等和故意失败回滚均有测试，已有学习事实与用户设置不被覆盖。
- 新增强校验本地量表注册表：发行物只含 16 个学习相关 IPIP 分面条目与 10 题英文 GSE；登记来源、版本、引用、许可、语言、计分、施测模式和解释边界。PALS 没有再分发许可，不含任何题目。
- 原始回答只追加到 `behavior_events(type='profile_measurement')`，写入前校验说明确认、启用/暂停状态、量表版本、题目、语言、施测模式和分值范围；记录结果明确为 `recorded_only`，不创建 attempt、不写 mastery、不影响调度。
- 画像默认本地启用、首次回答前需确认说明；HTTP/MCP 摘要分享默认关闭。关闭画像会同步关闭分享，重新开启不会偷偷恢复分享；派生 provenance/验证元数据拒绝原始回答字段和非 ID 证据引用。
- Core/CLI 提供设置、量表查看、回答记录、无原始回答概览、显式完整导出、仅画像重置和全部学习数据清除。仅画像重置在单事务删除回答、派生状态和验证结果，保留 attempts、设置与非敏感计数回执。
- 全部清除必须在 Engine 打开前走路径级命令，要求精确确认短语；可选备份不得覆盖已有文件或与主库/WAL/SHM/journal 重叠。清除前建立仅在事务期间存在的一致性恢复快照；旧库与 sidecar 先隔离，本机密钥清理失败恢复原文件，旧文件清理失败优先恢复逻辑数据库；成功后删除临时快照，在原路径保留当前 schema 的空库并报告真实删除数量。CLI 尚无 Credential Manager 存储，因此本票 smoke 如实报告本机密钥删除数 0，实际密钥接线留给 P17F。
- HTTP 新增只读 `GET /profile`，MCP 新增只读 `get_global_profile`；只有用户显式开启本地集成分享才返回分维度摘要，原始回答、导出、重置和全部删除能力永不暴露给 HTTP/MCP。
- 更新数据模型、API 稳定合约、隐私文档和量表资源说明。本票没有实现 P16E 的估计、EMA 频率、验证门计算或调度消费，也没有越界实现 P17E UI。

## 验收记录（2026-08-08）

```text
> cargo test -p polaris-core --test p16d_global_profile_governance
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

> cargo test -p polaris-core profile::tests
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 80 filtered out

> cargo test -p polaris-core db::tests
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 70 filtered out

> cargo test -p polaris-core ops::tests
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 78 filtered out

> cargo test -p polaris-cli global_profile
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 106 filtered out

> cargo test -p polaris-cli p16d_profile
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 105 filtered out

> cargo fmt --check
(exit 0; no output)

> cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.35s

> cargo test --workspace
474 tests across workspace suites; 474 passed; 0 failed

> git diff --check
(exit 0; no whitespace errors; Git reported only expected LF/CRLF conversion warnings for current working-copy files)
```

真实 CLI smoke 使用独立测试库完成：

```text
profile set --acknowledge-disclosure
profile record --instrument gse --item gse_01 --response 4
profile show
profile export
backup
doctor => ok=true, schema_version=3, migration_count=3, integrity_ok=true

profile delete-all --confirm "DELETE ALL POLARIS LEARNING DATA" --backup ...
=> files_deleted=3, local_secrets_deleted=0, empty_database_created=true
doctor(new empty database) => ok=true, schema_version=3, migration_count=3

profile delete-all --confirm "DELETE ALL POLARIS LEARNING DATA"  # no optional backup
=> backup_path=null, files_deleted=3, empty_database_created=true
doctor(new empty database) => integrity_ok=true; temporary recovery artifacts=0
```

提交前测试库备份保留于 `target/p16d-acceptance-20260808/profile-backup.sqlite`；全量删除 smoke 的用户选择备份保留于 `target/p16d-delete-acceptance-20260808/delete-profile-backup.sqlite`。

## 当前状态

- 阻塞点：无。
- 下一步建议：仅暂存并提交本票代码、资源、测试、契约和票据；不混入既有 `.gitignore`、漫画/视觉文件、SQLite、target、编辑器目录或执行规划文件。提交后按用户新增规划审计 P16G–P16L，再依 QUEUE 认领唯一下一票。
