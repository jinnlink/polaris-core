# P16G2 Flow 与幻影优先级（Flow / Phantom Precedence）

状态：Queued；依赖 P03D、P03E、P07D 与 P16G 红灯证据。

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
