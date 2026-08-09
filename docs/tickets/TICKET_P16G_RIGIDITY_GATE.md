# P16G 还死板吗门（Rigidity Gate）

状态：已实现并通过验收，等待提交；依赖 P01、P03D、P03E、P03H、P04C。P16G1（`825f8a7`）、P16G2（`2735169`）已完成。

## 本轮范围（2026-08-09）

- 只建立确定性的稳定性、响应性、方向性回归门与 7 组证据 arm，不改变产品默认行为。
- 先证明当前机制实际到达输出；若门红，保留事实并按票据要求报告，不通过改公式或放宽断言“修绿”。
- 预计修改面：`crates/polaris-core/tests/p16g_rigidity_gate.rs`；只有现有 API 无法提供测试确定性时，才考虑最小的测试专用注入点。

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

删除 `crates/polaris-core/tests/p16g_rigidity_gate.rs`，并恢复 `docs/tickets/QUEUE.md` 与本票状态。

无 schema 变更、无产品行为变更，回滚零影响。

## AI 交接记录（2026-08-09）

- 当前状态：测试门实现完成但真实红灯；用户已裁决拆出 P16G1、P16G2 两张前置修复票，完成后回到本票复验；未改产品代码。
- 已完成：7 个同构 evidence arm、A/B/C 三段独立断言、有序 next/batch 输出签名、分歧矩阵、`mrt.epsilon=0` 确定性控制、测试进程级人为死板化变异开关。
- 确定性结果：A 稳定性 7/7 arm 重放一致。
- 分歧矩阵：

```text
arm            col mas fai und pha mis beh
cold           0   1   1   1   1   1   1
mastery        1   0   1   0   1   1   1
fail           1   1   0   1   1   1   1
underconfident 1   0   1   0   1   1   1
phantom        1   1   1   1   0   1   1
misconception  1   1   1   1   1   0   1
behavioral     1   1   1   1   1   1   0
```

- 已证实通过的方向：mastery 从 cold 前进；fail batch 从 mastery 的 analyze+recall 降为全 recall；misconception 把 `borrowing` 提到 next；behavioral 只改 latency+hints+abandon 后改变 batch 顺序。
- 阻塞 1：underconfident（p_known=0.994、calib_gap=-0.699）与 mastery（p_known=0.994、calib_gap=0.061）的 next/batch 完全相同。当前调度只消费正向校准缺口，没有已冻结的“高分低自信”动作契约；修复需要新增或改变产品行为，违反本票禁区。
- 阻塞 2：phantom arm 已真实形成 `phase=phantom`（p_known=0、calib_gap=0.684、attempts=4），但同次 HMM 输出 `flow`，现有 P03G 合同明确要求 Flow 保持 slot shape 且压住 phantom transfer（`p03g_interleaved.rs::phase_action_loop_flow_strategy_keeps_existing_slot_shape`）。P16G 要求 PhantomChallenge，与既有冻结行为直接冲突，不能由执行 AI 自行改优先级或放宽断言。
- 人为死板化红灯：

```text
> $env:POLARIS_P16G_FORCE_RIGID='1'; cargo test -p polaris-core --test p16g_rigidity_gate responsiveness_different_evidence_arms_diverge -- --nocapture
P16G divergence matrix: all cells = 0
panic: at least five distinct targeted outputs are required, got 1
test result: FAILED. 0 passed; 1 failed; 2 filtered out
```

- 已跑验证：

```text
> cargo test -p polaris-core --test p16g_rigidity_gate -- --nocapture
test stability_same_evidence_sequence_replays_identically ... ok
test responsiveness_different_evidence_arms_diverge ... FAILED
test directionality_each_divergence_hits_its_expected_target ... FAILED
test result: FAILED. 1 passed; 2 failed

> cargo fmt --check
exit 0

> cargo clippy -p polaris-core --test p16g_rigidity_gate -- -D warnings
exit 0

> git diff --check
exit 0
```

- 未跑验证：全 workspace 测试；P16G 专项已确定红灯，workspace 必然包含同一失败，不能伪报全绿。
- 裁决结果：保持门和失败证据不变，P16G1 定义 underconfidence 的可验证动作契约，P16G2 裁决 Flow 与 PhantomChallenge 的优先级；两票完成后回到 P16G 复验。用户已于 2026-08-09 明确同意该方案并要求继续。

## AI 交付记录（2026-08-09）

- 最终状态：P16G1（`825f8a7`）让 underconfident 与 mastery 输出分离；P16G2（`2735169`）让 Phantom 反证优先于 Flow；本票只提交确定性活性门测试与状态记录，不再改产品代码。
- A 稳定性：7 个 arm 各自重放两次，next 与有序 batch 签名完全一致。
- B 响应性：7 个 arm 得到 7 个不同输出签名；mastery/fail、mastery/underconfident、mastery/behavioral 均明确分歧。
- C 方向性：mastery 前进；fail 整体降深；underconfident 的 ownership 从 analyze 升为 evaluate；phantom 的 next/batch 均为 ownership transfer；misconception 提升 borrowing；behavioral 改变 batch 顺序。
- 最终分歧矩阵：

```text
arm            col mas fai und pha mis beh
cold           0   1   1   1   1   1   1
mastery        1   0   1   1   1   1   1
fail           1   1   0   1   1   1   1
underconfident 1   1   1   0   1   1   1
phantom        1   1   1   1   0   1   1
misconception  1   1   1   1   1   0   1
behavioral     1   1   1   1   1   1   0
```

- 人为死板化：设置 `POLARIS_P16G_FORCE_RIGID=1` 后所有 arm 被压成 cold 输出，矩阵全 0，响应性测试以 `at least five distinct targeted outputs are required, got 1` 失败，退出 101。

### 最终验收输出

```text
> cargo test -p polaris-core --test p16g_rigidity_gate -- --nocapture
test directionality_each_divergence_hits_its_expected_target ... ok
test responsiveness_different_evidence_arms_diverge ... ok
test stability_same_evidence_sequence_replays_identically ... ok
test result: ok. 3 passed; 0 failed

> cargo fmt --check
exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> cargo test --workspace
polaris-cli: 108 passed; polaris-core: 81 passed
p03g_interleaved: 16 passed; p16g1_underconfidence_action: 4 passed
p16g_rigidity_gate: 3 passed; 0 failed
all discovered suites: exit 0

> git diff --check
exit 0
```

- 回滚：只撤销本票执行 `git revert <P16G-commit-sha>`，会移除活性门但不改变产品行为；若要连同两处行为裁决一起回滚，再按逆序撤销 `2735169`、`825f8a7`。无 schema 变更。
