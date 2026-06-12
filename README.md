# polaris-core

个人学习引擎内核（Rust）。主命题：**验证真懂 → 定位模糊 → 针对性补缺**。

这是 Polaris（`C:\MyProject\Polaris`）与 rust-mastery-lab（`C:\MyProject\Learned`）两次实现的内核提取与重构。两个旧仓库已冻结为只读参考。

## 当前状态

- Phase 0（设计与交接包）：**完成** —— 设计冻结于 `docs/MASTER_PLAN.md`，宪法在 `SPEC.md`。
- Phase 1（walking skeleton）：**P01 已实现并完成子 agent 审查补修** —— 票在 `docs/tickets/TICKET_P01_WALKING_SKELETON.md`。

## 快速开始（P01）

```powershell
cargo run -p polaris-cli -- pack validate packs/rust
cargo run -p polaris-cli -- --db target\p01-quickstart.db init --pack packs/rust
cargo run -p polaris-cli -- --db target\p01-quickstart.db next --session quickstart
cargo run -p polaris-cli -- --db target\p01-quickstart.db submit --concept ownership --response "Ownership controls which binding can drop a value." --confidence 4 --session quickstart
cargo run -p polaris-cli -- --db target\p01-quickstart.db status
cargo run -p polaris-cli -- --db target\p01-quickstart.db grade-pending
```

实跑要点：

- `pack validate` 输出：`pack ok: concepts=24 prerequisites=21 misconceptions=11`
- `next` 首题：`concept: ownership`，`task_type: recall`
- `submit` 输出：`provisional_score=0.700 degraded=true`
- `grade-pending` 输出：`processed=0 pending=1`

## 给人类

读 `SPEC.md` 前两节即可知道这是什么；想看全部设计读 `docs/MASTER_PLAN.md`。
开工方式：把下面的启动提示词发给你的执行 AI（Codex/GPT 等）。更完整的新窗口续跑规则见 `docs/AI_RUNBOOK.md`。

## 执行 AI 启动提示词（复制即用）

```text
请读取 AGENTS.md 和 docs/AI_RUNBOOK.md，并严格遵守其中的阅读顺序、工作纪律和新窗口续跑协议。
然后读 docs/tickets/QUEUE.md。如果已有 In Progress 票，只续做那张票；如果没有，认领下一张票并按单票制实现。
设计已冻结：不要再设计、不要引入新概念、不要做票范围外的事。
有歧义就在票内记录阻塞点和你的建议方案，停下来请我裁决。
完成的定义 = SPEC §6 验收基线全绿且实跑输出贴在票尾。
```

## 文档地图

| 文件 | 作用 |
|---|---|
| `SPEC.md` | 工程宪法：主命题、三支柱、铁律、边界、验收基线（冲突时它赢） |
| `AGENTS.md` | 执行 AI 的入口：阅读顺序、纪律、移植参考映射 |
| `docs/AI_RUNBOOK.md` | 新窗口续跑手册：启动提示词、接手清单、交接模板、防错规则 |
| `docs/ENHANCEMENT_ROADMAP.md` | P03E+ 增强优先级；姊妹文档见 `C:\MyProject\Learned\rust-mastery-lab\docs\ENHANCEMENT_ROADMAP.md` |
| `docs/MASTER_PLAN.md` | 完整设计蓝图（含抽象引擎、心智动力学引擎、五个原创框架、教学法纲要 v3、分阶段与验证门） |
| `docs/DATA_MODEL.md` | 表结构 DDL 与全部公式（实现的直接依据） |
| `docs/tickets/QUEUE.md` | 票队列（单票制，P01→P05 对应 Phase 1→5） |
