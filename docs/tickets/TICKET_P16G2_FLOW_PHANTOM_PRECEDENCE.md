# P16G2 Flow 与幻影优先级（Flow / Phantom Precedence）

状态：已实现、通过验收并提交（`2735169`）；依赖 P03D、P03E、P07D 与 P16G 红灯证据。

服务主命题环节：针对性补缺 → 验证真懂。

## 背景

P16G 证明同一证据序列可同时得到 `phase=phantom` 与 HMM `flow`。P07D 的旧合同让 Flow 无条件保护原 slot shape 和 move，结果明确的“高自信但答错”反证被短期状态推断压住，系统没有安排硬题确认。

本票裁决冲突：Phantom 是概念级、由多次校准证据支持的反证，优先于 session 级 Flow 推断；fatigue/bored 的降负荷保护仍优先，避免在用户明显疲劳时强推高摩擦任务。Settling/Regression 不借本票改变与 Flow 的既有关系。

## 冻结合同

1. 当 HMM 策略为 Flow 且 ranked candidates 存在 Phantom 时，选择 `PhantomChallenge`：batch 优先包含 phantom 概念，move 至少为 transfer，next 走同一相响应。
2. 当 Flow 下不存在 Phantom 时，保留“两弱/新 + 一复习”的原 slot shape。
3. EasyReviews（fatigue/bored/disengagement）仍保护降负荷策略，即使存在 Phantom 也不覆写。
4. Settling 与 Regression 在 Flow 下仍保持既有保护，本票不扩大到其他相。
5. 更新 P07D/P03G 的冲突测试与文字记录，明确这是后续裁决替代旧 Flow 无条件优先合同。

## 验收标准

- Flow + Phantom → PhantomChallenge，phantom 概念进入 batch 且 move=transfer。
- Flow + 非 Phantom → 既有 Flow slot shape 不变。
- EasyReviews + Phantom → 保持 easy reviews，不升 transfer。
- P16G phantom 方向红灯转绿，其他 7-arm 行为不退化。
- 无公式、schema、相判据和 HMM 估计变化。

## 禁区

- 不改 HMM 发射/转移矩阵，不伪造 dominant state。
- 不改变 Phantom 判据，不让所有 phase 都压过 Flow。
- 不削弱 fatigue/bored 的用户负荷保护。
- 不修改冻结参考仓库，不夹带用户其他工作区文件。

## 开工前复述（2026-08-09）

- 范围：只让 `Flow + Phantom` 选择 PhantomChallenge，并更新冲突回归；next 与 batch 继续共用已有 phase move 覆写。
- 禁区：不改 HMM、相判据、公式、EasyReviews 保护或 Flow 无 Phantom 的 slot shape。
- 预计修改面：`engine/task_selection.rs`、`p03g_interleaved.rs`、P07D 历史合同说明、QUEUE 与本票。
- 验收命令：见下节，并额外实跑 P16G 全门确认 3/3。

## 验收命令

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03g_interleaved phase_action_loop -- --nocapture
cargo test -p polaris-core --test p16g_rigidity_gate -- --nocapture
git diff --check
```

## 回滚方式

恢复 `task_selection.rs`、P03G/P16G 相关测试与文档；提交后执行 `git revert <P16G2-commit-sha>`。无 schema 变更。

## AI 交付记录（2026-08-09）

- 变更清单：单题出口仅对 `Flow + 当前 Phantom` 解除 phase 保护；batch 仅允许 PhantomChallenge 覆盖 Flow；更新 P03G 冲突测试与 P07D 后续裁决说明。
- 保护边界：`flow_batch_allows_two_weak_concepts` 证明无 Phantom 的 Flow slot shape 不变；fatigue/bored + Phantom 两条 EasyReviews 测试继续通过；Settling/Regression 未改变。
- P16G 结果：正常门 3/3 通过；phantom next=`ownership/transfer`，batch 首项=`ownership/transfer/phantom`；7 arm 两两分歧矩阵除对角线外全为 1。
- 变异证明：`POLARIS_P16G_FORCE_RIGID=1` 后矩阵全 0，响应性断言以 `unique=1` 失败，测试进程退出 101。

### 验收输出

```text
> cargo test -p polaris-core --test p03g_interleaved -- --nocapture
test result: ok. 16 passed; 0 failed

> cargo test -p polaris-core --test p16g_rigidity_gate -- --nocapture
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

- 回滚：`git revert <P16G2-commit-sha>`；无 schema 迁移。回滚后旧 Flow 无条件保护恢复，P16G phantom 方向门会重新变红。
