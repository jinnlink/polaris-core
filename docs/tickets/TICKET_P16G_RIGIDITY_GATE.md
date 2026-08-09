# P16G 还死板吗门（Rigidity Gate）

状态：Queued；依赖 P01、P03D、P03E、P03H、P04C。P16D 提交前不得认领。

服务主命题：全环节。本票是主命题闭环的活性判据本身，不是新能力。

## 背景

`MASTER_PLAN.md:7` 记录了本项目存在的理由：rust-mastery-lab 把"理解用户"硬写成 9 个写死的桶，**必然死板**。

`MASTER_PLAN.md:108` 把它升为内核验收判据：

> 判据：这个闭环在单 track 上"活、不死板"，内核就站住了。

`MASTER_PLAN.md:454` 把它升为阶段门：

> **每阶段必过的"还死板吗"检查**：换不同证据，下一步动作是否有意义地改变。

全仓检索 `死板` / `换不同证据` / `非僵硬` 只命中 `MASTER_PLAN.md` 六行，与 `TICKET_P13C_AI_INTERACTION_PROFILE.md:22` 一处字段说明。

对照：MASTER_PLAN 登记的其它门全部已票化——巩固回滚门 P03B、hazard AUC 与 HMM gate P03K、G_u precision P03H、F1/F3 P04C、F2 P03E、F5 预登记 P05B、校准 P06F、性能预算 P06E；P10A 还专门做了 `polaris trust show` 暴露 F1–F5 门状态。

**"还死板吗"是唯一被写成"每阶段必过"、却从未票化、从未跑过、也不在信任面板上的门。** 本票补这个缺口。

## 核心设计判断

**死板与乱动都是失败。** 朴素断言"换证据后输出不同"无法区分灵活与随机：P04C 的 MRT 微随机化本身就会制造差异，一个纯随机调度器能轻松通过朴素断言。因此本门必须同时成立三段断言，缺一不可：

- **A 稳定性**：同一证据序列重跑，输出一致。排除随机冒充响应。
- **B 响应性**：不同证据 arm 之间，输出发生分歧。
- **C 方向性**：分歧方向命中该 arm 的预期靶向。排除"动了但乱动"。

## 范围

### 1. 确定性前置

本门断言 A 要求确定性。已知抖动源：P04C 在 `next` 选 move 决策点的微随机化、P03C/P03N 的 HNSW 候选池。

必须先把 fixture 置于确定性控制下（固定种子或在 fixture 内走确定性路径），处理方式参考 P03N 的确定性夹具先例。

**若无法在不改变产品行为的前提下取得确定性，记为阻塞点并请用户裁决；不得通过放宽断言绕过。**

### 2. 证据 arm 夹具

7 组 arm，除注入证据外完全同构（同 pack、同初始状态、同 session 结构）：

| arm | 注入证据 | 预期靶向 |
|---|---|---|
| `cold` | 空证据 | 冷启动基线，供其余 arm 对照 |
| `mastery` | 高分 + 高自信 | 深度上移或引入新概念 |
| `fail` | 低分 + 低自信 | 降深度或回前置 |
| `underconfident` | 高分 + 低自信 | 校准方向动作；**不得与 `mastery` 输出相同** |
| `phantom` | 低分 + 高自信 | 幻影相靶向（P07D 已实现 `BatchStrategy::PhantomChallenge`） |
| `misconception` | 失败且带 `misconception_id` | 该概念修复优先级上升 |
| `behavioral` | 分数与自信同 `mastery`，仅高 latency + hints + abandon 不同 | 状态调制生效，输出应偏离 `mastery` |

`behavioral` 是关键 arm：它是唯一只在行为层不同的对照，用于证明 P03D HMM 与行为观测是否真正到达调度输出。

### 3. 分歧矩阵产物

测试必须输出 arm × arm 的分歧矩阵，比较维度至少含 `concept_id`、`task_type`、batch 策略。

**副产物即本票的第二价值**：任意两个 arm 输出完全相同，说明区分它们的那套机制没有到达调度输出。矩阵因此同时是一次"哪些机制是承重的"审计，结果贴在票尾。

### 4. 反向证明（必须）

提交一个人为死板化变异体的实跑红灯输出，例如把 `next` 固定为返回 `seed_order` 最小的概念，证明本门会失败。

**不能变红的门等于没有门。** 无红灯证据的交付不予接受。

### 5. 落点

`crates/polaris-core/tests/p16g_rigidity_gate.rs`，纳入 `cargo test --workspace`。

因为 SPEC §6 要求每张票都跑 workspace 测试，落在这里等于把 MASTER_PLAN 的"每阶段必过"升级为**每票必过**，强制力强于面板展示。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p16g_rigidity_gate
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- 断言 A、B、C 三段全部实现且独立可失败。
- 7 个 arm 全部存在，`behavioral` 与 `underconfident` 不得因实现方便被合并或删除。
- 分歧矩阵实跑输出贴在票尾。
- 人为死板化变异体的红灯输出贴在票尾。
- 确定性处理方式写明；若走固定种子，说明种子注入点与它不影响产品默认行为的理由。

## 禁区

- 不改调度公式、相判据、mastery 数学、move 选择策略、FSRS/BKT/MIRT/G_u/HMM 任何参数。**本票只观测，不调优。**
- 不为了过门而放宽断言、改阈值或删 arm。门红了是发现，不是障碍。
- 不新增 schema、DDL、meta 参数或迁移。
- 不做 `trust show` / HTTP / MCP 出口。workspace 测试已是每票必过，强制力高于面板；可见性增强留后续票（候选 P16H）。
- 不引入领域逻辑；fixture 只用现有 `packs/rust` 或 `packs/algorithms`。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 排队建议

建议置于 P16E 之前认领。

理由：P16E 是在既有约 15 套推断机制之上再加一层画像估计。本票的分歧矩阵会先给出"现有哪些机制真正改变了输出"的事实。若已有机制大量不承重，应先处理该事实，再决定 P16E 的范围。

本票零产品行为风险（只加测试），可在 P16D 提交后立即插入。

## 回滚

删除 `crates/polaris-core/tests/p16g_rigidity_gate.rs` 与其夹具；恢复 `docs/tickets/QUEUE.md` 中 P16G 条目；删除本票文件。

无 schema 变更、无产品行为变更，回滚零影响。
