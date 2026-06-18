# P12A 无感学习入口与外部知识入库规划

## 状态

Done（2026-06-18，通过验收）

## 服务主命题

验证真懂 -> 定位模糊 -> 针对性补缺。

本票不实现新能力，只把「学生如何自然开始学习」和「仓库外知识如何先入库再验证」写成可执行路线图，避免后续 P12 实现继续暴露 pack、SQLite、MCP 等内部概念给零基础学生。

## 背景

用户明确指出：如果学生学了 `Learned` 仓库以外的知识，也必须入库；学生入口不能复杂，不能要求零基础学习者理解 pack、数据库和工程参数。当前系统已有 P05C ingest 适配器、P07B learner mirror、P10A trust panel 和 `Learned` Aura 原型，但缺一份把这些能力组织成学生可用体验的路线图。

用户随后补充：学习项目本身也应该有一个接入 P-OS 的声明，否则学 Rust、英语、生物都要回到 `polaris-core` 说「开工」仍然不自然。本票将该反馈纳入路线图追补：后续 P12B 先做学习项目声明 v1，再做 capture queue。

## 范围

1. 新增 `docs/LEARNER_CAPTURE_ROADMAP.md`：
   - 明确 `Learned` 是课程教室，不是学习边界。
   - 明确学习项目通过 `p-os.toml` 声明接入 P-OS。
   - 明确 raw evidence、suggestion、mastery attempt 三层边界。
   - 明确学习收件箱状态、学生动作和第一版成功标准。
   - 明确 Aura、learner mirror、atlas 等视觉资产的使用位置。
2. 新增 `docs/superpowers/plans/2026-06-18-p12-learner-capture-inbox.md`：
   - 将 P12 拆成 P12B-P12G。
   - 给 P12B 项目声明和 P12C capture queue 写到文件、测试、命令级别的实现计划。
3. 更新 `docs/PRODUCT_ROADMAP.md`：
   - 补充 2026-06-18 产品轴线，链接本路线图。
4. 更新 `docs/tickets/QUEUE.md`：
   - 登记 P12A 为唯一 In Progress。
   - 不把 P12B-P12G 标为当前可自动认领票。

## 禁区

- 不改 Rust 代码。
- 不改数据库 schema。
- 不新增 HTTP/MCP/CLI 行为。
- 不修改 `C:\MyProject\Learned` 或 `C:\MyProject\Polaris`。
- 不把未来 P12B-P12G 伪装成已经认领的正式票。
- 不改变 `SPEC.md`、`DATA_MODEL.md` 的权威规则。

## 预计修改面

- `docs/LEARNER_CAPTURE_ROADMAP.md`
- `docs/superpowers/plans/2026-06-18-p12-learner-capture-inbox.md`
- `docs/PRODUCT_ROADMAP.md`
- `docs/tickets/QUEUE.md`
- `docs/tickets/TICKET_P12A_LEARNING_CAPTURE_ROADMAP.md`

## 验收

```powershell
rg -n "无感学习入口|学习收件箱|p-os.toml|学习项目声明|raw evidence|mastery attempt|Aura|recorded_only" docs\LEARNER_CAPTURE_ROADMAP.md docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md docs\PRODUCT_ROADMAP.md docs\tickets\TICKET_P12A_LEARNING_CAPTURE_ROADMAP.md
```

预期：存在匹配，退出码 0。

```powershell
rg -n "P12B.*当前可自动认领|P12C.*当前可自动认领|P12D.*当前可自动认领|P12E.*当前可自动认领|P12F.*当前可自动认领|P12G.*当前可自动认领|raw evidence 会直接改掌握度|raw evidence 可直接改掌握度|外部 AI 的判断.*直接改掌握度|外部 AI 评分作为掌握度权威" docs\LEARNER_CAPTURE_ROADMAP.md docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md docs\PRODUCT_ROADMAP.md docs\tickets\QUEUE.md
```

预期：无匹配，退出码 1。此命令不扫描本票文件，避免匹配到本节中的验收正则本身。

```powershell
rg -n "^- \[ \].*P12[BCDEFG].*In Progress|P12[BCDEFG].*当前可自动认领" docs\tickets\QUEUE.md
```

预期：无匹配，退出码 1。

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

若默认 target 遭遇 Windows 文件锁，可追加同参数隔离 target clippy/test，但必须保留默认失败原文。

## 回滚方式

删除本票新增的 2 个文档，回滚 `docs/PRODUCT_ROADMAP.md` 和 `docs/tickets/QUEUE.md` 中的 P12A 相关修改，并删除本票文件。

## 交付记录（2026-06-18）

### 变更清单

- 新增 `docs/LEARNER_CAPTURE_ROADMAP.md`，把无感学习入口、学习项目声明、外部知识入库、raw evidence / suggestion / mastery attempt 三层边界、学习收件箱、视觉化承接和 P12B-P12G 拆分写成路线图。
- 新增 `docs/superpowers/plans/2026-06-18-p12-learner-capture-inbox.md`，给后续 P12B 学习项目声明和 P12C Capture Queue 写到文件、测试、CLI/HTTP 和验收级别的实施计划，并列出 P12D-P12G 的边界。
- 更新 `docs/PRODUCT_ROADMAP.md`，补 2026-06-18「无感学习入口」产品轴线，明确 Learning Aura 是学生入口优先承接面，atlas 只面向开发者。
- 更新 `docs/tickets/QUEUE.md`，登记并完成 P12A，同时标明 P12B-P12G 仍是候选拆分，不是可直接认领的正式票。

### 审查记录

- 子 agent Bohr 做只读文档审查，未发现阻塞问题。
- 已按审查建议修复：
  - 负向验收命令避免自匹配票据中的验收正则。
  - 在 P12B 详细任务前补充「只有 QUEUE 将 P12B 标为唯一 In Progress 后才能执行」。
  - 标注「个人 overlay pack / pack validate / pack sandbox」是后台或维护者术语，不出现在学生界面。
  - 将第一版成功标准里的 `confidence` 统一为反馈前 `self_confidence`。
  - 明确 P12C 只允许新写入 `Pending`，`Mapped` / `PracticeReady` / `Practiced` 留给 P12D/P12E。
  - 用户追补指出各学习项目也要声明接入 P-OS 后，已补 `p-os.toml` 学习项目声明，并将后续顺序调整为 P12B 项目声明、P12C Capture Queue。

### 验收输出

> rg -n "无感学习入口|学习收件箱|raw evidence|mastery attempt|Aura|recorded_only" docs\LEARNER_CAPTURE_ROADMAP.md docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md docs\PRODUCT_ROADMAP.md docs\tickets\TICKET_P12A_LEARNING_CAPTURE_ROADMAP.md

摘录：

```text
docs\LEARNER_CAPTURE_ROADMAP.md:1:# 无感学习入口与外部知识入库路线图
docs\LEARNER_CAPTURE_ROADMAP.md:45:| 记录我刚学到的 | 把别处看到的东西存起来 | 写入 raw evidence，不直接影响掌握度 |
docs\LEARNER_CAPTURE_ROADMAP.md:183:## 7. 学习收件箱
docs\LEARNER_CAPTURE_ROADMAP.md:284:| Learning Aura | `C:\MyProject\Learned\rust-mastery-lab\aura` | 学生 | 优先复用为学生入口 |
docs\LEARNER_CAPTURE_ROADMAP.md:318:- 返回 `recorded_only`，明确不影响掌握度。
docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md:1:# P12 学习收件箱实现计划
docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md:247:recorded_only: true
docs\PRODUCT_ROADMAP.md:200:## 10. 2026-06-18 补充：无感学习入口
```

退出码 0。

> rg -n "P12B.*当前可自动认领|P12C.*当前可自动认领|P12D.*当前可自动认领|raw evidence 会直接改掌握度|raw evidence 可直接改掌握度|外部 AI 的判断.*直接改掌握度|外部 AI 评分作为掌握度权威" docs\LEARNER_CAPTURE_ROADMAP.md docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md docs\PRODUCT_ROADMAP.md docs\tickets\QUEUE.md

输出为空；退出码 1（符合预期）。

> rg -n "^- \[ \].*P12[BCDEF].*In Progress|P12[BCDEF].*当前可自动认领" docs\tickets\QUEUE.md

输出为空；退出码 1（符合预期）。

> cargo fmt --check

输出为空；退出码 0。

> cargo clippy --workspace --all-targets -- -D warnings

默认 target 失败于 Windows 目标目录写锁，未出现 Rust/Clippy 诊断：

```text
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
```

同参数隔离 target 通过：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-p12a-target'; cargo clippy --workspace --all-targets -- -D warnings
```

```text
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 44s
```

> cargo test --workspace

默认 target 通过。输出摘要：

```text
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Doc-tests polaris_core
```

> git diff --check

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/PRODUCT_ROADMAP.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
```

退出码 0；只有 CRLF 提示，无 whitespace error。

### 用户追补：学习项目声明

用户指出：如果学 Rust、英语、生物都必须回到 `polaris-core` 说「开工」，入口仍然不对。已追补：

- `docs/LEARNER_CAPTURE_ROADMAP.md`：新增 `p-os.toml` 学习项目声明，明确每个学习项目自己声明接入 P-OS。
- `docs/superpowers/plans/2026-06-18-p12-learner-capture-inbox.md`：后续顺序调整为 P12B Learning Project Manifest v1、P12C Capture Queue v1，原 P12C-P12F 后移到 P12D-P12G。
- `docs/PRODUCT_ROADMAP.md` 与 `docs/tickets/QUEUE.md`：同步 P12B-P12G 候选拆分，仍不得直接认领。

追补验收：

> rg -n "无感学习入口|学习收件箱|p-os.toml|学习项目声明|raw evidence|mastery attempt|Aura|recorded_only" docs\LEARNER_CAPTURE_ROADMAP.md docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md docs\PRODUCT_ROADMAP.md docs\tickets\TICKET_P12A_LEARNING_CAPTURE_ROADMAP.md

摘录：

```text
docs\LEARNER_CAPTURE_ROADMAP.md:94:### 学习项目声明
docs\LEARNER_CAPTURE_ROADMAP.md:102:| 学习项目声明 | `p-os.toml` | 说明这个文件夹是一个学习现场，默认用哪个 pack、怎么开工、哪些路径可作为证据 | 不需要 |
docs\LEARNER_CAPTURE_ROADMAP.md:131:1. 从当前工作目录向上查找 `p-os.toml`。
docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md:19:| P12B | Learning Project Manifest v1 | 每个学习项目用 `p-os.toml` 声明已接入 P-OS |
docs\PRODUCT_ROADMAP.md:207:- 每个学习项目应有自己的 `p-os.toml` 接入声明。学生在 Rust、英语、生物等项目里说「开工」，入口先按当前项目声明解析，不要求回到 `polaris-core`。
```

退出码 0。

> rg -n "P12B.*当前可自动认领|P12C.*当前可自动认领|P12D.*当前可自动认领|P12E.*当前可自动认领|P12F.*当前可自动认领|P12G.*当前可自动认领|raw evidence 会直接改掌握度|raw evidence 可直接改掌握度|外部 AI 的判断.*直接改掌握度|外部 AI 评分作为掌握度权威" docs\LEARNER_CAPTURE_ROADMAP.md docs\superpowers\plans\2026-06-18-p12-learner-capture-inbox.md docs\PRODUCT_ROADMAP.md docs\tickets\QUEUE.md

输出为空；退出码 1（符合预期）。

> rg -n "^- \[ \].*P12[BCDEFG].*In Progress|P12[BCDEFG].*当前可自动认领" docs\tickets\QUEUE.md

输出为空；退出码 1（符合预期）。

> cargo fmt --check

输出为空；退出码 0。

### 阻塞与裁决

- 无产品或设计阻塞。
- 默认 `target/debug` clippy 仍受 Windows 写锁影响；已记录原文，并用隔离 `CARGO_TARGET_DIR` 同参数通过。
- 当前工作区仍有本票外既有脏文件，提交时只应纳入 P12A 范围文件。

### 回滚方式

回滚本票提交即可恢复；若手工回滚，删除 `docs/LEARNER_CAPTURE_ROADMAP.md`、`docs/superpowers/plans/2026-06-18-p12-learner-capture-inbox.md`、`docs/tickets/TICKET_P12A_LEARNING_CAPTURE_ROADMAP.md`，并撤销 `docs/PRODUCT_ROADMAP.md` 与 `docs/tickets/QUEUE.md` 中的 P12A 相关修改。
