# P02B 图谱感知诊断实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 `test-driven-development`；完成后使用 `requesting-code-review` 做只读审查。步骤使用复选框语法跟踪进度。

**目标：** 为 P02B 增加确定性的图谱诊断接口：失败后前置传播，confusion 边生成辨析任务。

**架构：** 新建 `diagnosis` 模块承载纯内核逻辑，`Engine` 只做薄封装，CLI 增加只读 `diagnose` 命令。诊断结果不写库、不改调度器排序。

**技术栈：** Rust、rusqlite、现有 `meta` 参数读取、现有 `concepts/edges/attempts/mastery_states` 表。

---

## 文件职责

- `crates/polaris-core/src/diagnosis.rs`：诊断结构体、前置传播、confusion 辨析任务生成。
- `crates/polaris-core/src/engine.rs`：暴露 `diagnose_concept`。
- `crates/polaris-core/src/lib.rs`：导出 `diagnosis` 模块。
- `crates/polaris-cli/src/main.rs`：新增 `diagnose --concept <id>` 只读命令。
- `crates/polaris-core/tests/p02b_diagnosis.rs`：P02B integration tests。
- `docs/tickets/TICKET_P02B_GRAPH_AWARE_DIAGNOSIS.md`：验收与交付记录。
- `docs/tickets/QUEUE.md`：单票状态。

## 任务 1：写失败测试

- [ ] 新建 `crates/polaris-core/tests/p02b_diagnosis.rs`。
- [ ] 覆盖：
  - `X` 最近失败，`Y -> X` 前置未达标，焦点为 `Y`。
  - 最近分数未失败时不触发前置传播焦点。
  - 多个未达标前置按 `p_known ASC, weight DESC, id ASC` 排序。
  - `confusion` 边生成 `discriminate` 辨析任务。
- [ ] 修改 CLI parse 测试，加入 `polaris diagnose --concept ownership`。
- [ ] 运行 `cargo test -p polaris-core --test p02b_diagnosis`，确认因接口缺失失败。

## 任务 2：实现内核诊断

- [ ] 新建 `diagnosis.rs`。
- [ ] 用 `meta('bkt.cut_lo')` 判断最近失败；用 `meta('sched.prereq_p')` 判断前置达标。
- [ ] 查询 `prerequisite` 入边与 `confusion` 相邻边，输出确定性排序。
- [ ] `Engine::diagnose_concept` 调用诊断模块。
- [ ] 运行 P02B 目标测试，确认通过。

## 任务 3：实现 CLI 只读接口

- [ ] `Commands` 新增 `Diagnose { concept }`。
- [ ] 输出 concept、latest_failed、focus、prerequisite gaps、confusion tasks。
- [ ] 更新 CLI parse 测试。

## 任务 4：验收与交付

- [ ] 运行 `cargo fmt --check`。
- [ ] 运行 `cargo clippy --workspace --all-targets -- -D warnings`。
- [ ] 运行 `cargo test --workspace`。
- [ ] 运行 `cargo test -p polaris-core --test p02b_diagnosis`。
- [ ] 请求子 agent 做只读审查。
- [ ] 写票尾交付记录。
