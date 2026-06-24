# TICKET P13C：AI Interaction Profile v1

状态：已实现、通过验收并提交

服务主命题：针对性补缺 → 用户可控。

## 背景

P13A/P13B 已经让 AI IDE 能在课程仓库中连接同一个 Polaris MCP，P12C-P12E 也补齐了 raw capture → learner inbox → inbox practice 的学习闭环。现在缺一层很实际的使用偏好：学生希望 AI 像“安静陪跑”“详细老师”“严格教练”还是“苏格拉底追问”，以及 AI 应该多主动、多解释还是少打扰。

本票不是改教学数学，也不是把课程主导权交给 AI；它只是把用户对 AI 助手交互方式的偏好保存为本地结构化 profile，并通过 CLI/HTTP/MCP 暴露给 AI IDE。

## 范围

- 新增 AI interaction profile 本地结构：
  - `persona`：AI 性格/角色。
  - `verbosity`：话多话少。
  - `explanation_depth`：解释深度。
  - `proactivity`：主动程度。
  - `intervention_frequency`：介入频率。
  - `correction_style`：纠错风格。
  - `custom_notes`：可选补充说明，避免只有死板枚举。
- 提供默认 profile，适合作为普通学生的平衡设置。
- 提供 profile 更新校验：非法枚举不写入，避免 AI IDE 误传破坏本地偏好。
- CLI：
  - `polaris ai-profile show [--json]`
  - `polaris ai-profile set ... [--json]`
- HTTP：
  - `GET /ai-profile`
  - `POST /ai-profile`
- MCP：
  - `get_ai_interaction_profile`
  - `update_ai_interaction_profile`
- 输出中提供面向 AI IDE 的 `guidance` 文本，告诉外部 AI 如何按该 profile 说话与介入。
- 更新 README、AI IDE 使用指南和 API 合约。
- 增加一条真实使用 smoke flow：初始化数据库、设置 profile、通过 MCP/CLI/HTTP 读取，再跑 capture → inbox → practice → submit 基本链路。

## 禁区

- 不修改 mastery、FSRS、BKT/MIRT、相图、调度或评分公式。
- 不把 AI profile 当成学习能力或掌握度信号。
- 不让外部 AI 的评分、性格或主动程度影响 mastery facts。
- 不做完整桌面 UI / Aura UI；本票先交付可用的 CLI/HTTP/MCP 设置面，后续 UI 壳另开票。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并粘贴输出：

```powershell
cargo test -p polaris-core --test p13c_ai_interaction_profile
cargo test -p polaris-cli p13c
cargo test -p polaris-cli mcp_contract
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

真实使用 smoke flow 必须至少覆盖：

```powershell
cargo run -p polaris-cli -- --db target\p13c-real-use.sqlite init --pack packs\rust
cargo run -p polaris-cli -- --db target\p13c-real-use.sqlite ai-profile set --persona socratic_tutor --verbosity detailed --explanation-depth examples_first --proactivity stuck_only --intervention-frequency normal --correction-style guided
cargo run -p polaris-cli -- --db target\p13c-real-use.sqlite ai-profile show --json
cargo run -p polaris-cli -- --db target\p13c-real-use.sqlite capture --text "我刚学到所有权和 drop 有关。" --candidate-concept ownership
cargo run -p polaris-cli -- --db target\p13c-real-use.sqlite inbox list
```

## 回滚方式

- 删除 AI interaction profile 核心模块与测试。
- 从 Engine、CLI、HTTP、MCP 中移除 P13C 入口和测试。
- 恢复 `README.md`、`docs/AI_IDE_USAGE.md`、`docs/API_CONTRACT.md`、`docs/tickets/QUEUE.md` 与本票状态。
- 不需要 schema 迁移回滚；本票使用现有 `meta` 表保存 profile JSON。

## 本轮范围（2026-06-24）

- 用户明确要求“提交后继续做”，并裁决今天要完成 Polaris OS 剩余可用工作、开始实际使用测试。
- 预计修改面：AI profile core 模块、Engine facade、CLI/HTTP/MCP 出口、API 合约、README/AI IDE 使用说明、P13C 测试和真实使用 smoke flow 记录。

## 交付记录（2026-06-24）

### 变更清单

- 新增 `AiInteractionProfile` 与 `AiInteractionProfileInput`，使用现有 `meta` 表保存本地 AI 交互偏好 JSON，不新增 schema 迁移。
- 新增 Engine facade：读取默认 profile、部分更新 profile、非法枚举拒绝写入。
- 新增 CLI：
  - `polaris ai-profile show [--json]`
  - `polaris ai-profile set ... [--json]`
- 新增 HTTP：
  - `GET /ai-profile`
  - `POST /ai-profile`
- 新增 MCP tools：
  - `get_ai_interaction_profile`
  - `update_ai_interaction_profile`
- 输出面向 AI IDE 的 `guidance` 中文指导文本，让外部 AI 按学生设置的性格、话量、解释深度、主动程度、介入频率与纠错风格工作。
- HTTP/MCP 更新入口对传入类型做严格校验：字段存在但不是 string 时拒绝写入。
- `custom_notes` 最长 2000 字符；空字符串清除补充说明。
- 更新 `README.md`、`docs/AI_IDE_USAGE.md`、`docs/API_CONTRACT.md` 与 MCP contract 测试。

### 验收输出

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo test -p polaris-core --test p13c_ai_interaction_profile

running 5 tests
test update_ai_interaction_profile_rejects_invalid_values_without_mutation ... ok
test update_ai_interaction_profile_persists_student_preferences ... ok
test default_ai_interaction_profile_is_balanced_and_read_only ... ok
test update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation ... ok
test update_ai_interaction_profile_trims_blank_custom_notes ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```powershell
> cargo test -p polaris-cli p13c

running 4 tests
test tests::p13c_ai_profile_commands_parse_show_and_set ... ok
test mcp::tests::p13c_mcp_gets_and_updates_ai_interaction_profile ... ok
test http::tests::p13c_http_ai_profile_gets_and_updates_preferences ... ok
test tests::p13c_ai_profile_set_command_persists_preferences ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 89 filtered out; finished in 0.16s
```

```powershell
> cargo test -p polaris-cli mcp_contract

running 5 tests
test mcp::tests::mcp_contract_document_names_stable_surface_and_policy ... ok
test mcp::tests::mcp_contract_lists_stable_tools_resources_and_templates ... ok
test mcp::tests::mcp_contract_initialize_keeps_stable_handshake_fields ... ok
test mcp::tests::mcp_contract_errors_keep_stable_shape ... ok
test mcp::tests::mcp_contract_resource_reads_keep_stable_top_level_fields ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 88 filtered out; finished in 0.05s
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings

    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on build directory
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s
```

```powershell
> cargo test --workspace

running 93 tests
...
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
...
running 5 tests
test update_ai_interaction_profile_rejects_invalid_values_without_mutation ... ok
test update_ai_interaction_profile_persists_student_preferences ... ok
test default_ai_interaction_profile_is_balanced_and_read_only ... ok
test update_ai_interaction_profile_trims_blank_custom_notes ... ok
test update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 真实使用 smoke flow

```powershell
> $db = 'target\p13c-real-use.sqlite'
> Remove-Item -LiteralPath $db -ErrorAction SilentlyContinue
> Remove-Item -LiteralPath "$db-shm" -ErrorAction SilentlyContinue
> Remove-Item -LiteralPath "$db-wal" -ErrorAction SilentlyContinue
> cargo run -p polaris-cli -- --db $db init --pack packs\rust
initialized

> cargo run -p polaris-cli -- --db $db ai-profile set --persona socratic_tutor --verbosity detailed --explanation-depth examples_first --proactivity stuck_only --intervention-frequency normal --correction-style guided
AI 交互偏好
persona: socratic_tutor
verbosity: detailed
explanation_depth: examples_first
proactivity: stuck_only
intervention_frequency: normal
correction_style: guided
custom_notes: -
guidance: 性格：苏格拉底式追问，先让学生想，再给提示。话量：详细，多解释推理过程和取舍。解释深度：优先用例子、反例和类比解释。主动程度：学生卡住、连续失败或信心低时再主动介入。介入频率：中频，在关键节点提醒。纠错方式：先引导学生自查，再给修正。

> cargo run -p polaris-cli -- --db $db ai-profile show --json
{
  "version": 1,
  "persona": "socratic_tutor",
  "verbosity": "detailed",
  "explanation_depth": "examples_first",
  "proactivity": "stuck_only",
  "intervention_frequency": "normal",
  "correction_style": "guided",
  "custom_notes": null,
  "guidance": "性格：苏格拉底式追问，先让学生想，再给提示。话量：详细，多解释推理过程和取舍。解释深度：优先用例子、反例和类比解释。主动程度：学生卡住、连续失败或信心低时再主动介入。介入频率：中频，在关键节点提醒。纠错方式：先引导学生自查，再给修正。"
}

> cargo run -p polaris-cli -- --db $db capture --text "我刚学到所有权和 drop 有关。" --candidate-concept ownership
capture_id: 02ef48de-7c06-4e6d-9dae-70ded7e0c699
evidence_id: 4dc8b69e-feb3-41cc-b19a-02bfd17018f1
status: pending
learner_kind: reference
recorded_only: true
message: 已保存为学习资料，不会直接算作掌握。

> cargo run -p polaris-cli -- --db $db inbox list
学习收件箱：1 条
1. 已保存，稍后帮你整理 [pending]
   capture_id: 02ef48de-7c06-4e6d-9dae-70ded7e0c699
   摘要: 我刚学到所有权和 drop 有关。
   可能相关: Ownership
   可选: 转成一道小题(accept) / 稍后再看(defer) / 忽略(ignore)
```

### 回滚方式

- 移除 `crates/polaris-core/src/ai_profile.rs` 与 `crates/polaris-core/tests/p13c_ai_interaction_profile.rs`。
- 从 `Engine`、CLI、HTTP、MCP 中移除 `ai-profile` / `/ai-profile` / profile MCP tools 入口和测试。
- 恢复 `README.md`、`docs/AI_IDE_USAGE.md`、`docs/API_CONTRACT.md`、`docs/tickets/QUEUE.md` 中 P13C 相关说明。
- 无需数据库迁移回滚；已写入的 `meta.ai.interaction_profile` 是孤立偏好 JSON，不影响学习事实、掌握度、调度或评分。
