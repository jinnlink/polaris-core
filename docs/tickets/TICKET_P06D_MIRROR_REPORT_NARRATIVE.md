# P06D 镜像报告 Tier 1 叙事润色

状态：已完成

服务主命题：定位模糊

## 背景

P03I 已交付 evidence-bound 镜像报告 v1：断言、假设、建议均带证据 id 与置信度。P06A 将报告生成/读取暴露给 MCP，P06C 又补了稳定性属性测试。强化轴线还剩一个实用性小票：让 Tier 1 在用户显式请求时把断言列表润色成周报叙事，但必须继续遵守 strict-citation 和降级路径。

本票只做叙事层，不改变报告断言生成规则。

## 本轮范围

1. `MirrorReport` 新增可选 narrative：
   - narrative 仅在显式请求 Tier 1 润色且通过 strict-citation 后填充。
   - citation 的 `evidence_id` 使用报告 item id；`quote` 必须是该 item `claim` 的原文子串。
2. 新增静态响应测试入口：
   - 用于红绿测试，不依赖真实 LLM。
   - 合法 JSON 接受并持久化 narrative。
   - 非法 citation / malformed JSON 降级为 `narrative = None`，原始断言列表仍可用。
3. CLI/MCP 显式入口：
   - `polaris report --narrative` 才尝试 Tier 1 润色；默认 `polaris report` 不触发 LLM。
   - MCP `run_mirror_report` 增加可选 `narrative: true` 参数；默认不触发 LLM。

## 验收

必须通过：

```powershell
cargo test -p polaris-core p06d
cargo test -p polaris-cli report_narrative
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不改变 P03I 断言/假设/建议的生成公式、阈值、证据门。
- 不让默认 `run_mirror_report` 自动发起 LLM 网络调用。
- 不把 LLM narrative 当作新的证据或掌握度输入。
- 不在 narrative citation 中引用原始 attempts / behavior evidence；本票只引用报告 item 原文。
- 不处理 `.gitignore`、`.cursor/`、`docs/visuals/` 等票外改动。
- 不修改 frozen 参考仓库。

## 交付记录

### 变更清单

- `MirrorReport` 新增可选 `narrative` 字段与 `MirrorReportNarrative` 结构。
- 新增显式 Tier 1 叙事入口：
  - `run_mirror_report_with_config` 使用 OpenAI-compatible LLM 配置生成叙事。
  - `run_mirror_report_with_static_narrative` 供红绿测试使用，不依赖真实 LLM。
  - 默认 `run_mirror_report` 保持纯 Tier 0，不触发 LLM。
- narrative strict-citation 证据源限定为报告 item：
  - `evidence_id` 为 report item id。
  - `quote` 必须是该 item `claim` 的原文子串。
  - malformed JSON、空文本、缺 citation、非法 citation、缺 LLM 配置均降级为 `narrative = None`。
- CLI 增加 `polaris report --narrative`；默认 `polaris report` 不触发 LLM。
- MCP `run_mirror_report` 增加可选 `narrative: true` 参数；默认不触发 LLM。
- 新增 P06D 集成测试与 CLI/MCP 入口测试，覆盖默认无叙事、合法叙事持久化、非法 citation 降级、原始 evidence quote 冒充 report item claim 降级、malformed 降级和序列化。

### 红灯记录

```text
cargo test -p polaris-core p06d
error[E0432]: unresolved import `polaris_core::report::MirrorReportNarrative`
error[E0609]: no field `narrative` on type `MirrorReport`
error[E0599]: no method named `run_mirror_report_with_static_narrative` found for struct `Engine`
exit 101
```

说明：首轮红灯来自测试先行暴露的缺失结构、字段和静态 narrative 测试入口；随后补齐生产实现。

```text
cargo test -p polaris-cli report_narrative
error[E0026]: variant `Commands::Report` does not have a field named `narrative`
exit 101
```

说明：CLI 红灯确认默认 `Report` 命令尚无显式 narrative flag；随后补 `--narrative` 和 MCP 参数。

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
error: length comparison to zero
crates\polaris-cli\src\mcp.rs:963:17
exit 1
```

说明：隔离 clippy 捕获测试断言 `len() > 0`；已改为 `!is_empty()` 并重跑通过。

### 验收输出

```text
cargo test -p polaris-core p06d
p06d_narrative_serializes_with_report ... ok
p06d_default_report_keeps_narrative_absent ... ok
p06d_invalid_narrative_citation_degrades_to_raw_report ... ok
p06d_original_evidence_quote_is_rejected_even_with_report_item_id ... ok
p06d_static_narrative_accepts_strict_citation_to_report_claim ... ok
p06d_empty_or_malformed_static_narrative_degrades_without_losing_assertions ... ok
6 passed; 0 failed
exit 0
```

```text
cargo test -p polaris-cli report_narrative
tests::report_narrative_flag_parses_explicit_tier1_request ... ok
mcp::tests::mcp_report_narrative_argument_degrades_without_llm_config ... ok
2 passed; 0 failed
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
exit 1
```

说明：默认 target 目录仍受 Windows 文件锁影响；按既有工作区处理方式改用隔离 target 目录验证代码本身。

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.85s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 29 passed
polaris-core unit: 68 passed
integration suites: all passed, including p03i_mirror_report 14 passed, p03k_mental_fit 7 passed, p06c_property_expansion 3 passed, p06d_mirror_report_narrative 6 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

### 技术选择

- 默认报告路径继续走 `NarrativeSource::None`，避免同步路径和默认 CLI/MCP 触发 LLM。
- LLM narrative 只作为报告展示层持久化，不回写证据、掌握度、参数或调度输入。
- 叙事证据集由 `assertions`、`hypotheses`、`suggestions` 的 `id + claim` 构成，复用已有 strict-citation 校验器。
- LLM 调用失败、无配置或两次响应均无法通过 strict-citation 时，返回原始镜像报告列表并将 `narrative` 留空。

### 子 agent 审查

Helmholtz（`019ec596-983c-7b31-9dfe-f3051771e77b`）只读审查结论：

- 核心 P06D 代码未发现阻塞问题。
- 默认路径确认不触发 LLM：`run_mirror_report` 走 `NarrativeSource::None`，CLI 只有 `--narrative` 进入叙事路径，MCP 默认 `narrative=false`。
- citation 边界确认正确：叙事 evidence 集只由 report item 的 `id + claim` 构成，未引用原始 attempts / behavior evidence。
- 未发现 P03I 断言、假设、建议公式、阈值或证据门被改动。
- 必须处理项：工作树存在票外 `.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`。处理方式：不回退用户或其他窗口改动，提交时只白名单 stage P06D 文件，确保这些票外改动不进入本票提交。
- 建议项：补一条合法 report item id 但 quote 来自原始 evidence 而非 item claim 的降级测试。已新增 `p06d_original_evidence_quote_is_rejected_even_with_report_item_id` 并通过验收。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/report.rs crates/polaris-core/src/engine.rs crates/polaris-cli/src/main.rs crates/polaris-cli/src/mcp.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/tests/p06d_mirror_report_narrative.rs docs/tickets/TICKET_P06D_MIRROR_REPORT_NARRATIVE.md
```

提交后：

```powershell
git revert <P06D-commit-sha>
```
