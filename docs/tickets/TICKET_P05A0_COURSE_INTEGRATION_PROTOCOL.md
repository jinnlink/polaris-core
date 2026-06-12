# P05A0 课程接入协议 v1

状态：未开始

服务主命题：验证真懂 → 定位模糊 → 针对性补缺

## 背景

Polaris Core 的内核是领域无关的。外部课程不能直接把课件、章节或题库塞进内核，而是要实现一套稳定的课程接入协议：Domain Pack API / Course Integration Protocol。

当前仓库已有 `packs/rust` 样例、`pack validate`、`SPEC.md` 中的 Domain Pack 边界，以及 `docs/MASTER_PLAN.md` 中的多域插拔设计。但这些内容还没有整理成面向外部课程作者的稳定协议文档。没有这层协议，后续接入英语、考试、专业课或第三方课程时，会变成每次靠 AI 临场理解 pack 形状，产品能力不稳定。

## 范围

1. 新增课程接入协议文档：
   - 说明外部课程必须提供哪些文件。
   - 说明每个文件的字段、语义、不变量和最小示例。
   - 明确 `pack.toml`、`concepts.toml`、`misconceptions.toml`、`rubric.md`、`moves.toml`、`ingest.toml` 的职责边界。
2. 固化 validator 规则：
   - 文档列出 `polaris pack validate <dir>` 必须检查的结构完整性、引用完整性、字段合法性和版本兼容规则。
   - 如果现有 validator 缺失关键检查，补最小实现和测试。
3. 明确证据映射协议：
   - 外部课程如何声明哪些输入可以成为 `evidence_items`。
   - 哪些 evidence 可以生成 attempt，哪些只能作为辅助上下文。
   - strict-citation 对课程内容、作答证据和评分反馈的要求。
4. 明确评分与教学协议：
   - `rubric.md` 如何定义深度判定、通过标准和示例。
   - `moves.toml` 如何声明 recall / explain / apply / transfer 等练习动作。
   - 常见误解如何进入 `misconceptions.toml`，并与 G_u 误解语法保持兼容。
5. 写给外部课程作者的接入指南：
   - 从一门现有课程到 pack 的迁移步骤。
   - 最小可用 pack 示例。
   - 常见错误和 validator 报错解释。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p polaris-cli -- pack validate packs/rust
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不在内核写入 Rust、英语、考试等领域特定逻辑。
- 不把课程协议设计成 LLM prompt 约定；必须是可验证的文件协议。
- 不在本票实现第二个 pack。
- 不修改冻结参考仓库。
- 不放宽现有 pack validator 以迁就坏数据。

## 交付记录

待填写。
