# P16G1 低自信校准动作（Underconfidence Action）

状态：已实现并通过验收，等待提交；依赖 P01、P04C 与 P16G 红灯证据。

服务主命题环节：定位模糊 → 针对性补缺。

## 背景

P16G 的确定性证据臂证明：高分低自信样本会形成显著负向 `calib_gap`，但当前 `U(c)` 只消费正向校准缺口，因此 `underconfident` 与普通 `mastery` 的 next/batch 输出完全相同。系统看见了学习者持续低估自己，却没有给出任何可观察动作。

本票补最小动作闭环，不借机重写调度公式：概念排序仍由既有 `U(c)` 决定；当概念已经掌握、证据充足且持续低自信时，在既有基础 move 上只提升一级，让学习者用稍强证据确认自己确实会了。transfer 封顶，绝不降级。

## 冻结合同

1. underconfidence 判定必须同时满足：
   - `p_known >= bkt.cut_hi`；
   - `attempt_count >= calib.phantom_n`，复用现有最小校准证据数；
   - `calib_gap <= -calib.underconfidence_gap`。
2. 新增 `calib.underconfidence_gap=0.25`：A 类、Manual、边界 `[0.15,0.40]`。它决定用户可见动作门，不允许自动调优改及格线。
3. 命中后，基础 move 沿 `recall→explain→apply→analyze→evaluate→create→transfer` 只提升一级；transfer 封顶，不得降级或跨多级加压。
4. `next_task()` 与 `get_interleaved_batch()` 必须共享同一动作规则；原因或审计上下文需可识别 `underconfidence_calibration`，不能只改文案而不改任务。
5. 不改变候选概念排序、`U(c)`、BKT/MIRT/FSRS、校准 fold、相图判据、HMM 或 schema。

## 验收标准

- 高 `p_known`、足够样本、显著负 `calib_gap`：next 与 batch 的基础 move 只提升一级，并有校准原因。
- 同样掌握度但校准正常：保持原 move；P16G 的 mastery 与 underconfident 输出产生真实分歧。
- 样本不足、未达到掌握门或负向 gap 未过门时均不触发。
- 基础 move 已较深时仍只提升一级，transfer 保持 transfer。
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

## AI 交付记录（2026-08-09）

- 变更清单：新增 A/Manual 参数 `calib.underconfidence_gap=0.25`；候选仍按原 `U(c)` 排序，只派生 underconfidence 动作标记；next 与 batch 共用“基础 move 只升一级、transfer 封顶”规则；MRT 预登记写入 `underconfidence_calibration`；更新数据模型、参数表和 4 个端到端专项测试。
- 设计校准：首版“仅 recall→explain”专项虽绿，但 P16G 的 apply 证据使基础 move 已为 analyze，真实矩阵仍与 mastery 合并；据此改为沿完整 Bloom move 阶梯只升一级。复跑后 underconfident 的 ownership 从 analyze 变为 evaluate，矩阵与 mastery 分离。
- 范围证据：P16G 当前 2/3 通过；唯一剩余红灯为 Phantom/Flow，留给 P16G2，本票未修改其优先级。
- 工作区基线说明：`p16g_rigidity_gate.rs` 是未纳入本票、且已知仍有 P16G2 红灯的未跟踪测试。运行 workspace 基线时暂时移出 Cargo 测试发现目录，命令结束后已原样恢复；另单独实跑该文件并保留 2 通过/1 预期失败的真实输出。

### 验收输出

```text
> cargo fmt --check
exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> cargo test -p polaris-core --test p16g1_underconfidence_action -- --nocapture
running 4 tests
test result: ok. 4 passed; 0 failed

> cargo test -p polaris-core --test engine_task_selection
test result: ok. 3 passed; 0 failed

> cargo test -p polaris-core --test p04c_mrt
test result: ok. 12 passed; 0 failed

> cargo test --workspace
# 未跟踪的 blocked P16G test 暂移出 tests/，finally 中原样恢复
polaris-cli: 108 passed; polaris-core: 81 passed
p16g1_underconfidence_action: 4 passed; 0 failed
all discovered suites: exit 0

> cargo test -p polaris-core --test p16g_rigidity_gate -- --nocapture
test stability_same_evidence_sequence_replays_identically ... ok
test responsiveness_different_evidence_arms_diverge ... ok
test directionality_each_divergence_hits_its_expected_target ... FAILED
directionality failures: phantom phase did not trigger the phantom challenge direction
test result: FAILED. 2 passed; 1 failed
```

- 回滚：`git revert <P16G1-commit-sha>`；无 schema 迁移，回滚后 P16G underconfidence 红灯会按预期重新出现。
