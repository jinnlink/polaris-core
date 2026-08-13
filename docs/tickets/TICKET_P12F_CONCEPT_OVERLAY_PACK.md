# P12F Concept Suggestion + Overlay Pack

状态：In Progress（已实现并通过验收，待用户确认提交）；依赖 P17G。

> 2026-08-13 用户裁决：当前机器不重启，P17G 保持 Deferred 且不视为完成；允许 P12F 先行开发。该例外不解除 P17G 后续发布验收责任。

服务主命题：定位模糊 → 针对性补缺。

## 范围

- 对无法可靠映射到现有概念的 Capture 生成候选 concept/schema/typed edge/misconception；每条必须包含 evidence id、精确 quote、候选理由和模型版本。
- Tier 1 只生成 suggestion，strict-citation 失败时保持 raw capture，不生成候选。
- 建立每个 base Pack 的个人 overlay pack：base 不可变、overlay 独立版本、来源账本、差异预览、接受/拒绝和整版回滚。
- 用户明确接受后依次通过 `pack validate` 与 `pack sandbox` 才安装 overlay；概念 id 冲突、循环 prerequisite、未知边或缺 provenance 必须拒绝。
- 接受 overlay 只扩展可学习图谱，不创建 attempt、不写 mastery；后续必须经正常练习取证。

## 禁区

- 不直接修改正式 Pack，不让 LLM 自动接受建议。
- 不把阅读材料、候选或 pack 安装视为掌握证据。
- 不写域特定逻辑，不绕过 strict-citation/validator/sandbox。

## 验收

- 可映射/不可映射、citation 失败、概念/边/图式建议、冲突/环、接受/拒绝、overlay 升级/回滚和 base Pack 不变测试。
- 接受前后 attempts/mastery 不变；UI 始终显示建议状态和来源。
- SPEC §6 基线、Pack validator/sandbox、桌面集成与 `git diff --check` 全绿。

## 回滚

停用或回滚 overlay 版本并重建派生图索引；不删除 raw evidence、attempt 或 base Pack。

## 交付记录（2026-08-13）

### 变更清单

- schema 升至 v10，新增 suggestion、overlay 版本、完整实体快照与 provenance 账本；旧库迁移保持幂等。
- Tier 1 对未映射 Capture 生成 concept/schema/typed edge/misconception 候选；复用 strict-citation，失败或模型不可用时不落候选且 raw capture 原样保留。
- 接受前以“不可变 base + 完整 overlay”执行冲突、边端点、边型与 prerequisite 环校验，再复用 `pack validate` 与 `pack sandbox`；安装、升级、回滚均保持 attempts/mastery 不变。
- Desktop Inbox 增加“分析新知识点”、证据/模型来源、明确接受/拒绝、整版撤销及可恢复错误反馈；Tier 1 外发内容已纳入隐私清单与 Tier0-only 降级。
- `docs/DATA_MODEL.md` 补齐 v10 DDL 与版本语义；Rust/TypeScript 契约同步生成。

### 验收实跑

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -j 1 -- -D warnings`（`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`）：通过，`Finished dev profile`，0 warning。
- `cargo test --workspace -j 1`（同上关闭调试符号与增量缓存，沙箱外执行 Windows 注册表只读测试）：通过；CLI 120、Core unit 83、P12F 4、Desktop lib 16、Desktop foundation 20，所有集成与 doc tests 0 failed。
- `npm --prefix apps/desktop test`：13 files / 33 tests passed。
- `npm --prefix apps/desktop run typecheck`：通过。
- `npm --prefix apps/desktop run build`：通过，4644 modules transformed；随后将桌面版本 DTO 从 i64 收窄为安全的 i32/number，消除构建中的 BigInt target 警告。
- `npm --prefix apps/desktop run contracts:check`：通过；完整 Rust foundation 的 `generated_typescript_contract_is_current` 再次通过。
- P12F 集成测试内真实执行 validator 与 3 天 sandbox：`validation=pass`、`sandbox!=fail`；v1 → v2 → v1 → 空版本回滚通过，base/attempts/mastery 不变。
- `git diff --check`：通过（仅工作区 CRLF 转换提示）。

### 回滚方式

- 产品内对当前 base Pack 执行“撤销这一版个人知识”，会整版恢复 parent version；重复执行可回到无 overlay 状态。
- 代码回滚时仅反向撤销本票提交；schema v10 表保留不会影响旧学习事实。不得删除 `evidence_items`、`attempts`、`mastery_states` 或 base Pack。
