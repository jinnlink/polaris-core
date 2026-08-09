# P16G1 低自信校准动作（Underconfidence Action）

状态：In Progress；依赖 P01、P04C 与 P16G 红灯证据。

服务主命题环节：定位模糊 → 针对性补缺。

## 背景

P16G 的确定性证据臂证明：高分低自信样本会形成显著负向 `calib_gap`，但当前 `U(c)` 只消费正向校准缺口，因此 `underconfident` 与普通 `mastery` 的 next/batch 输出完全相同。系统看见了学习者持续低估自己，却没有给出任何可观察动作。

本票补最小动作闭环，不借机重写调度公式：概念排序仍由既有 `U(c)` 决定；当概念已经掌握、证据充足且持续低自信时，只把过浅的 `recall` 提升为 `explain`，让学习者说明为什么答案成立，以可验证、低摩擦的方式校准自我判断。更深的既有 move 不降级。

## 冻结合同

1. underconfidence 判定必须同时满足：
   - `p_known >= bkt.cut_hi`；
   - `attempt_count >= calib.phantom_n`，复用现有最小校准证据数；
   - `calib_gap <= -calib.underconfidence_gap`。
2. 新增 `calib.underconfidence_gap=0.25`：A 类、Manual、边界 `[0.15,0.40]`。它决定用户可见动作门，不允许自动调优改及格线。
3. 命中后，基础 move 为 `recall` 时提升为 `explain`；`explain/apply/analyze/evaluate/create/transfer` 保持原深度，不得降级。
4. `next_task()` 与 `get_interleaved_batch()` 必须共享同一动作规则；原因或审计上下文需可识别 `underconfidence_calibration`，不能只改文案而不改任务。
5. 不改变候选概念排序、`U(c)`、BKT/MIRT/FSRS、校准 fold、相图判据、HMM 或 schema。

## 验收标准

- 高 `p_known`、足够样本、显著负 `calib_gap`：next 与 batch 中原本的 recall 变为 explain，并有校准原因。
- 同样掌握度但校准正常：保持原 move；P16G 的 mastery 与 underconfident 输出产生真实分歧。
- 样本不足、未达到掌握门或负向 gap 未过门时均不触发。
- 基础 move 已深于 explain 时不降级。
- 参数注册表锁定默认值、类型、边界和调优途径。
- P16G 只允许 underconfidence 相关红灯转绿；Flow/Phantom 红灯留给 P16G2，不在本票顺手修。

## 禁区

- 不新增 `sched.w_underconfidence`，不改变 `sched.w_*` 单纯形或 `U(c)`。
- 不把低自信当低分，不回写 mastery，不制造伪尝试。
- 不改 P07D 的 Flow/phase 优先级。
- 不修改冻结参考仓库，不夹带用户其他工作区文件。

## 开工前复述

- 范围：新增 underconfidence 动作判定、共享 move 后置调整、可观察原因/审计和专项测试。
- 禁区：不改排序公式、掌握度、相图、HMM、schema 或 P16G2 范围。
- 验收命令：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p16g1_underconfidence_action -- --nocapture
cargo test -p polaris-core --test p16g_rigidity_gate -- --nocapture
git diff --check
```

- 预计修改面：`config.rs`、`engine.rs`、`engine/task_selection.rs`、`pedagogy.rs`、专项测试、`DATA_MODEL.md`、`QUEUE.md` 与本票。

## 回滚方式

未提交前恢复上述产品/文档文件并删除专项测试；提交后执行 `git revert <P16G1-commit-sha>`。无 schema 变更，回滚不需要迁移。
