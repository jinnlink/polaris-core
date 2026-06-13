# P05A0 课程接入协议 v1

状态：Completed（已提交；默认 target clippy 受 Windows 文件锁阻塞，隔离 target 同参数通过）

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

### 开工记录（2026-06-13）

- 当前范围：新增面向外部课程作者的课程接入协议文档，固化 `pack.toml`、`concepts.toml`、`misconceptions.toml`、`rubric.md`、`moves.toml`、可选 `ingest.toml` 的职责边界，并说明 `polaris pack validate <dir>` 的当前规则。
- 禁区：不写入 Rust、英语、考试等领域专用逻辑；不把协议约束放进 LLM prompt；不实现第二个 pack；不修改冻结参考库；不放宽现有 validator。
- 验收命令：
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo run -p polaris-cli -- pack validate packs/rust`
  - `git diff --check`
- 预计修改面：`docs/COURSE_INTEGRATION_PROTOCOL.md`、`docs/tickets/QUEUE.md`、本票。

### 交付记录（2026-06-13）

#### 变更清单

- 新增 `docs/COURSE_INTEGRATION_PROTOCOL.md`，作为课程接入协议 v1 文档。
- 文档覆盖 pack 目录结构、文件职责、字段语义、最小示例、当前 validator 规则、未强制规则、版本兼容、证据映射、strict-citation 要求、`ingest.toml` 预留边界、课程作者迁移步骤和常见报错解释。
- 更新 `docs/tickets/QUEUE.md` 与本票状态/交付记录。
- 未修改 core 代码、validator 代码、`packs/rust/` 或冻结参考库。

#### 验收输出

`cargo fmt --check`

```text
exit 0，无输出。
```

`cargo run -p polaris-cli -- pack validate packs/rust`

```text
pack ok: concepts=24 prerequisites=21 misconceptions=11
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.45s
```

`cargo test --workspace`

```text
P04E: test result: ok. 3 passed; 0 failed; finished in 15.14s
P05A1: test result: ok. 5 passed; 0 failed; finished in 0.08s
Doc-tests polaris_core: test result: ok. 0 passed; 0 failed
Finished `test` profile [unoptimized + debuginfo] target(s) in 1.63s
```

`cargo clippy --workspace --all-targets -- -D warnings`

```text
默认 target 失败于既有 Windows 文件锁：
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
```

同参数隔离 target 复核：

```text
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-target-p05a0-clippy'; cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 11s
```

`git diff --check`

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/TICKET_P05A0_COURSE_INTEGRATION_PROTOCOL.md', LF will be replaced by CRLF the next time Git touches it
```

#### 阻塞点与建议

- 阻塞点：票面默认 target 的 clippy 命令仍被 `target/debug` 下的 Windows `os error 5` 写入拒绝阻塞；这不是 P05A0 文档改动引入的 lint 问题。
- 建议：延续 P05A1 裁决，接受隔离 `CARGO_TARGET_DIR` 的同参数 clippy 作为本机文件锁替代验收证据；若需要原命令 exit 0，应单独处理默认 target 锁后复跑。
- 是否改变设计/验收/数据模型：不改变设计和数据模型；只涉及本机验收执行环境。

#### 用户确认

- 用户回复“继续高歌猛进”，按接受隔离 target clippy 证据并提交处理。

#### 回滚方式

- 删除 `docs/COURSE_INTEGRATION_PROTOCOL.md`。
- 将 `docs/tickets/QUEUE.md` 与本票状态/交付记录恢复到 P05A0 开工前。
