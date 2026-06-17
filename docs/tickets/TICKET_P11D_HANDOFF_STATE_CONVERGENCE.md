# P11D 交接状态收敛

状态：Done（2026-06-17，通过验收）

## 背景

P11C 已完成并提交后，`docs/tickets/QUEUE.md` 已显示无 In Progress，且没有正式未完成票。但只读审计发现，`docs/PRODUCT_ROADMAP.md` 和 `docs/ENHANCEMENT_ROADMAP.md` 仍保留旧的“下一票”或“待裁决”文字，可能让新窗口重复认领已完成票。

本票只做文档状态收敛，避免下一位 AI 把历史路线图当成当前执行队列。

## 范围

1. 更新 `docs/tickets/QUEUE.md`，标明当前无可自动认领下一票；新增本票并保持单票制状态。
2. 更新 `docs/PRODUCT_ROADMAP.md`，把 P07A-P10A 执行序标为历史已完成，不再指向 P09A 作为下一票。
3. 更新 `docs/ENHANCEMENT_ROADMAP.md`，标明增强路线图为历史视图，当前状态以 QUEUE 为准。
4. 更新 `AGENTS.md` 与 `docs/AI_RUNBOOK.md`，补齐“QUEUE 无未完成正式票时不得从旧路线图自行认领”的边界。

## 禁区

- 不改代码、不改测试、不改数据模型。
- 不新增产品能力或重新设计路线图。
- 不修改冻结参考仓库。
- 不触碰当前无关脏文件。

## 验收

```powershell
rg -n "若无认领：按 §5 序号 1 起领|P03D \(HMM, in progress\)|建议执行序（待裁决）|排序待用户裁决|排序与建议执行序|候选票，排序待用户裁决|本节只立项不实现|当前真实状态（截至 2026-06-15）|检查 `docs/tickets/QUEUE.md` 是否已认领 P07A" AGENTS.md docs/AI_RUNBOOK.md docs/tickets/QUEUE.md docs/PRODUCT_ROADMAP.md docs/ENHANCEMENT_ROADMAP.md
```

预期：无匹配，命令返回 1。

```powershell
rg -n "无可自动认领下一票|历史执行序|不从旧路线图自行认领|状态收敛" AGENTS.md docs/AI_RUNBOOK.md docs/tickets/QUEUE.md docs/PRODUCT_ROADMAP.md docs/ENHANCEMENT_ROADMAP.md
```

预期：存在匹配，命令返回 0。

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

## 回滚方式

回滚本票对 `AGENTS.md`、`docs/AI_RUNBOOK.md`、`docs/PRODUCT_ROADMAP.md`、`docs/ENHANCEMENT_ROADMAP.md`、`docs/tickets/QUEUE.md` 的修改，并删除本票文件。

## 交付记录（2026-06-17）

### 变更清单

- `AGENTS.md` 与 `docs/AI_RUNBOOK.md`：补充 QUEUE 无未完成正式票时的处理边界，禁止从旧路线图自行认领。
- `docs/PRODUCT_ROADMAP.md`：将 P07A-P10A 执行序标为历史已完成，续跑说明改为以 QUEUE 为准。
- `docs/ENHANCEMENT_ROADMAP.md`：标注增强路线图为历史设计理据，去除“待裁决/当前立项”口吻。
- `docs/tickets/QUEUE.md`：登记 P11D，并将 Backlog 中旧候选段落改为历史候选来源说明。
- `docs/tickets/TICKET_P11D_HANDOFF_STATE_CONVERGENCE.md`：新增本票规格、验收和回滚方式。

### 审查记录

- 票据余量审计（Kierkegaard）：确认当前无正式功能票可继续实现，P11C 之后没有 P11D/P12 类正式下一票。
- 交接文档审计（Bernoulli）：指出 `PRODUCT_ROADMAP.md` §9 仍指向 P09A，建议开 docs-only hygiene 票。
- 规格复审（Hubble）：初审要求排除票外脏文件，并清理 QUEUE Backlog 的“建议执行序”残留；修复后复审通过。
- 中文文档复审（Kepler）：初审建议将 QUEUE、ENHANCEMENT、PRODUCT 中旧候选口吻改为历史口吻；修复后复审通过。

### 验收输出

> rg -n "若无认领：按 §5 序号 1 起领|P03D \(HMM, in progress\)|建议执行序（待裁决）|排序待用户裁决|排序与建议执行序|候选票，排序待用户裁决|本节只立项不实现|当前真实状态（截至 2026-06-15）|检查 `docs/tickets/QUEUE.md` 是否已认领 P07A" AGENTS.md docs\AI_RUNBOOK.md docs\tickets\QUEUE.md docs\PRODUCT_ROADMAP.md docs\ENHANCEMENT_ROADMAP.md

输出为空；退出码 1（符合预期，表示旧误导短语无匹配）。

> rg -n "无可自动认领下一票|历史执行序|不从旧路线图自行认领|状态收敛" AGENTS.md docs\AI_RUNBOOK.md docs\tickets\QUEUE.md docs\PRODUCT_ROADMAP.md docs\ENHANCEMENT_ROADMAP.md

```text
docs\AI_RUNBOOK.md:19:如果没有 In Progress 票，请先看 QUEUE 是否有未完成正式票；若没有，不要从旧路线图自行认领，先向用户报告当前无可自动认领下一票。
docs\PRODUCT_ROADMAP.md:196:3. 如果 QUEUE 有未完成正式票，按 QUEUE 顺序认领；不要从本路线图的历史执行序自行认领。
docs\PRODUCT_ROADMAP.md:197:4. 如果 QUEUE 没有未完成正式票，向用户报告当前无可自动认领下一票，并等待用户裁决或新开规划票。
docs\PRODUCT_ROADMAP.md:202:**本路线图状态**：v1，2026-06-15 产品经理交付；2026-06-17 经 P11D 标注历史执行序已完成。当前可执行状态以 `docs/tickets/QUEUE.md` 为准。
docs\tickets\QUEUE.md:4:当前无可自动认领下一票；后续如需继续开发，需用户裁决或新开规划票。
docs\tickets\QUEUE.md:87:- [x] **P11D 交接状态收敛**（`TICKET_P11D_HANDOFF_STATE_CONVERGENCE.md`）← 已实现并通过验收；清理旧路线图中的下一票误导，不改代码；服务环节：全环节（AI 协作可持续）
AGENTS.md:31:3. 如果已有 In Progress 票，只续做那张票；如果没有，先看 QUEUE 是否有未完成正式票。若 QUEUE 无未完成正式票，不从旧路线图自行认领，向用户报告当前无可自动认领下一票并等待裁决。
```

> cargo fmt --check

输出为空；退出码 0。

> cargo clippy --workspace --all-targets -- -D warnings

默认 `target/debug` 首次复跑失败于 Windows 目标目录写锁：`failed to write ... libpolaris_core-*.rmeta: 拒绝访问。 (os error 5)`。未出现 Rust/Clippy 诊断。改用隔离 target 后同参数通过：

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.05s
```

执行命令：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-p11d-target'; cargo clippy --workspace --all-targets -- -D warnings
```

> cargo test --workspace

使用同一隔离 target 执行：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-p11d-target'; cargo test --workspace
```

关键输出：

```text
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

> git diff --check

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'AGENTS.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/AI_RUNBOOK.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/ENHANCEMENT_ROADMAP.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/PRODUCT_ROADMAP.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
```

退出码 0；只有 CRLF 提示，无 whitespace error。

### 阻塞与裁决

- 无产品或设计阻塞。
- 默认 `target/debug` 存在 Windows 写锁，已用隔离 `CARGO_TARGET_DIR` 执行同参数 clippy/test。该问题不属于 P11D 文档变更范围。
- 当前工作区仍有本票外既有脏文件，提交时只纳入 P11D 范围文件。

### 回滚方式

回滚本票提交即可恢复交接文档状态；若手工回滚，撤销本票变更的 5 个文档文件并删除本票文件。
