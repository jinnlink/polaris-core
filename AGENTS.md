# AI 协作入口（polaris-core）

你（执行 AI）接手的是一个**设计已冻结**的项目：架构、数学、验收标准都已写定。你的工作是按票实现，不是再设计。

## 阅读顺序

1. `SPEC.md` —— 宪法。任何冲突它赢。
2. `docs/tickets/QUEUE.md` —— 当前该做哪张票。
3. 当前票全文 —— 范围、验收、禁区。
4. `docs/DATA_MODEL.md` —— 表结构与公式，实现的直接依据。
5. `docs/MASTER_PLAN.md` —— 完整设计蓝图，按需查对应章节（不需要通读后才动手）。

## 工作纪律

- **单票制**：同一时刻只有一张票 In Progress。票外发现的问题记入 QUEUE 的 Backlog，不顺手做。
- **每张票交付**：变更清单 + 验收命令的实跑输出 + 回滚方式。
- **验证先于宣称**：SPEC §6 基线全绿才能说"完成"，输出必须真实粘贴。
- **卡住或歧义**：在票内记录"阻塞点 + 你的建议方案"，请用户裁决；不要擅自改设计。
- **冻结仓库只读**：`C:\MyProject\Polaris`、`C:\MyProject\Learned` 可读作移植参考，禁止任何修改。
- **语言**：文档与 commit message 用中文；代码标识符用英文；注释只写代码无法表达的约束。

## 新窗口续跑协议

每个新 AI 窗口都必须先读本文件，再读 `docs/AI_RUNBOOK.md`。如果聊天记录缺失，以仓库内文件为准，不依赖上一个窗口的口头描述。

新窗口接手时必须做 5 件事：

1. 读 `SPEC.md`、`docs/tickets/QUEUE.md`、当前票、`docs/DATA_MODEL.md`。
2. 检查 `git status --short`，识别已有改动；不得回退自己没做的改动。
3. 如果已有 In Progress 票，只续做那张票；如果没有，按 QUEUE 的下一张票认领。
4. 开工前复述当前票的范围、禁区、验收命令和预计修改面。
5. 完工前跑当前票列出的全部验收，并把真实输出写入票尾。

若新窗口发现上一个窗口没有交接清楚，先补一段「当前状态」到当前票尾：已完成、未完成、阻塞点、下一步建议。不要猜测完成度。

## 移植参考映射（读旧库时按此定位，别自己重新发明）

| 要移植的东西 | 源头 |
|---|---|
| FSRS 算法（P01 对拍移植） | `C:\MyProject\Polaris\apps\web\src\lib\fsrs.ts` |
| strict-citation 校验器 | `C:\MyProject\Polaris\apps\analysis\src\llm\codex_report.ts` 的 `validateCitations` |
| SQLite 迁移形状 | `C:\MyProject\Polaris\apps\web\src\lib\db\migrate.ts` |
| pack 的 TOML 形状 | `C:\MyProject\Learned\rust-mastery-lab\domain\*.toml` |
| 错误卡/教学惯例 | `C:\MyProject\Learned\rust-mastery-lab`（`review/`、`course/`） |

## 启动一张票

读 QUEUE → 把票标 In Progress → 按票实现 → 跑验收（全绿）→ 在票尾写交付记录 → 等用户确认后 commit。

详细执行模板见 `docs/AI_RUNBOOK.md`。
