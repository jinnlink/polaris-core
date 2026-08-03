# P12F Concept Suggestion + Overlay Pack

状态：Queued；依赖 P17G。

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
