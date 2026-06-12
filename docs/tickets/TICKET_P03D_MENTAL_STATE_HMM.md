# P03D 状态 HMM + 行为发射 + 离散时 hazard 放弃模型

状态：已完成

服务主命题环节：定位模糊 → 针对性补缺。

## 背景

`SPEC.md` 把心智动力学引擎列为三支柱之一；`docs/DATA_MODEL.md` §7 已冻结 P03D 的 attempt 级观测、6 状态 HMM、先验发射均值、转移矩阵、状态门控和放弃 hazard 门槛。本票只把这个 Tier 0 状态层接入现有内核，给后续 P03E/P03F/P04C 使用，不能让未过门的状态或 hazard 悄悄改变调度。

## 范围

- 新增领域无关的 `mental_state` 模块：
  - 6 个离散状态：心流、生产性困惑、挫败、无聊、焦虑、疲劳。
  - attempt 级 `HmmObservation`：`z_latency`、`min(hints,3)`、`resid = y - p_hat`、`consec_fail`、`conf_delta`、`interval_bucket`、`session_min`。
  - 使用 §7 给出的前 5 维均值表和 σ=1 对角高斯发射；另按 §7 文本约束让 `session_min` 区分疲劳、`interval_bucket` 区分无聊；转移矩阵初始值为对角 0.7、其余 0.06。
  - 前向滤波在线跑，输出归一化 posterior 与 dominant state。
- 接入 `Engine::submit`：
  - 每次 submit 继续先写 `attempts` 和 `latency` 行为事件。
  - 在提交同步路径内计算并记录 `behavior_events.type='mental_state'` 快照，payload 包含 attempt_id、features、posterior、dominant_state、gate 状态、hazard 估计。
  - HMM/hazard 默认只记录，不参与 `next_task` 和教学策略。
- 实现 hazard 评分与门控语义：
  - logistic 输入为 `[状态后验(6), calib_gap, consec_fail, hint_rate, sin/cos(时段), session_min]`。
  - 没有已验证 AUC 或 AUC < `hazard.auc_gate` 时，hazard 只记录，`participates=false`。
  - 状态层预测增益未过 `hmm.gate_auc_margin` 时，`strategy_enabled=false`。
- 不新增专用持久表；P03D v1 先复用 `behavior_events` 作为事件日志。若后续镜像报告/状态镜子需要物化视图，再单独开票。

## 禁区

- 不修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。
- 不引入临床标签、学习风格、MBTI 或不可证伪构念。
- 不让 HMM posterior 或 hazard 在未过验证门时影响 `next_task`、scheduler、teaching instruction。
- 不实现镜像报告、相图、MRT、摩擦曲线或 move 签名。
- 不发明新的顶层概念；命名使用 MASTER_PLAN / DATA_MODEL 已有词汇。
- 不把外部 AI 判断直接写入状态。

## 验收

必须真实运行并把输出粘贴到本票尾：

```powershell
cargo test -p polaris-core --test p03d_mental_state
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

额外人工核对：

- `behavior_events` 中存在 `mental_state` 快照，且 payload 内 posterior 归一化。
- hazard 门没过时 `participates=false`。
- `next_task` 不因 P03D 状态层改变排序。
- `docs/tickets/QUEUE.md` 只有 P03D 一张票处于 In Progress。

## 回滚

- 删除 `crates/polaris-core/src/mental_state.rs`。
- 从 `crates/polaris-core/src/lib.rs` 和 `engine.rs` 移除 P03D 接入。
- 删除 `crates/polaris-core/tests/p03d_mental_state.rs`。
- 移除本票和计划文件，并把 `docs/tickets/QUEUE.md` 中 P03D 恢复为未认领。

## 本轮范围（2026-06-12）

- 当前状态：P03C 已提交；QUEUE 状态文字滞后，已在本票认领时修正为 P03D In Progress。
- 已有非本票改动：`docs/MASTER_PLAN.md`、`docs/tickets/QUEUE.md` 中 P05A0 相关修改，漫画文档和 `TICKET_P05A0_COURSE_INTEGRATION_PROTOCOL.md`；本票不得回退这些改动。
- 本票预计修改面：`mental_state.rs`、`engine.rs`、`lib.rs`、P03D 测试、QUEUE、计划文档。

## 交付记录（2026-06-12）

### 变更清单

- 新增 `mental_state` 内核模块：
  - 6 状态枚举、attempt 级 `HmmObservation`、`StatePosterior`。
  - HMM 前向滤波：先验转移矩阵对角 0.7、其余 0.06，posterior 归一化。
  - 对角高斯发射：使用 DATA_MODEL §7 已冻结的前 5 维均值；同时按 §7 文本让 `interval_bucket` 偏无聊、`session_min` 偏疲劳。
  - hazard logistic 打分、显式 validation AUC 门控、L2 逻辑回归拟合函数。
- 接入 `Engine`：
  - `submit` 写入 `behavior_events.type='mental_state'` provisional 快照。
  - `apply_final_score`、`grade_pending` 成功路径追加 `score_source='final'` 修正版快照。
  - final 快照复用该 attempt 的 pre-attempt `p_hat` 与 prior，避免 residual 数据泄漏。
  - 延迟 final 快照不作为后续 attempt 的 latest prior；后续 HMM prior 只读 provisional 快照。
  - hazard 未拟合时 payload 标记 `model_status='unfit'`，`participates=false`。
  - `next_task`、scheduler、teaching instruction 未接入 P03D 状态层。
- 新增 P03D 集成测试：
  - HMM flow/frustration、transition smoothing、无聊/疲劳 temporal 特征。
  - hazard AUC gate 与 logistic fit。
  - submit 记录快照且 posterior 归一化。
  - final score 与 grade queue 成功路径追加 final 快照。
  - 延迟 final 不污染下一次 prior。
  - scheduler 忽略未过门 mental_state 事件。
- CodeGraph 本地索引目录加入 `.gitignore`：`/.codegraph/`。

### 验收输出

```text
cargo test -p polaris-core --test p03d_mental_state
running 10 tests
test hmm_prior_emission_distinguishes_flow_and_frustration ... ok
test hazard_requires_auc_gate_before_participating ... ok
test hmm_transition_smooths_toward_previous_posterior ... ok
test hmm_temporal_features_distinguish_boredom_and_fatigue ... ok
test hazard_logistic_fit_gates_only_when_auc_passes ... ok
test submit_records_mental_state_snapshot_without_enabling_strategy_or_hazard ... ok
test final_score_appends_corrected_mental_state_snapshot ... ok
test grade_pending_success_appends_final_mental_state_snapshot ... ok
test delayed_final_snapshot_does_not_become_next_prior ... ok
test scheduler_ignores_mental_state_events_until_gate_passes ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.12s
exit 0
```

```text
cargo test --workspace
polaris-cli: 9 passed
polaris-core unit: 45 passed
p02a_graph: 3 passed
p02b_diagnosis: 4 passed
p02c_teaching: 2 passed
p03a_mirt: 5 passed
p03b_consolidation: 3 passed
p03c_geometry: 7 passed
p03d_mental_state: 10 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅有 Git LF/CRLF 提示，无 whitespace 错误。
```

### 子 agent 审查

- Curie（`019eb9ac-434a-72e3-94ea-9864d210c7a2`）做 P03D 范围/风险只读审查：
  - 建议不新增专用表，复用 `behavior_events` 记录 `mental_state` / `hazard` 快照。
  - 提醒 `resid = y - p_hat` 必须使用 attempt 前预测，不能用更新后 θ。
  - 提醒 final score 回填应追加修正版快照。
- Fermat（`019eb9b7-0cc6-74e0-b763-7d6dd987f539`）做代码质量只读审查，已修复其 Important 问题：
  - LLM 成功路径和 `grade_pending` 成功路径补 final 快照。
  - 延迟 final 快照不作为后续 HMM latest prior。
  - temporal 特征纳入无聊/疲劳区分并加测试。
  - `pre_attempt_p_hat` 不吞掉 MIRT 层错误。
  - hazard 拟合改为显式 validation AUC。
  - `latency_z` 改用 SQL 聚合，避免同步路径拉全量历史到内存。
  - payload 增加 `schema_version`、hazard inputs、`model_status='unfit'`。
- Harvey（`019eb9b6-dd9c-7ee3-9474-c6b879fe9728`）规格审查多次等待超时，已关闭；未返回可执行反馈。

### 技术选择

- P03D v1 不新增表：当前 DATA_MODEL 事实源是 `attempts + behavior_events + pack seed`，现有 `behavior_events(type, payload_json)` 足够承载快照。
- HMM/hazard 未过验证门前只记录，不调度：payload 中 `strategy_enabled=false`，hazard `participates=false`。
- final 修正版追加事件，不覆盖 provisional：保留事件时间线，同时避免派生事件自我循环。

### 回滚方式

未提交前：

```powershell
git restore .gitignore crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs docs/tickets/QUEUE.md
git clean -f crates/polaris-core/src/mental_state.rs crates/polaris-core/tests/p03d_mental_state.rs docs/superpowers/plans/2026-06-12-p03d-mental-state-hmm.md docs/tickets/TICKET_P03D_MENTAL_STATE_HMM.md
```

提交后：

```powershell
git revert <P03D-commit-sha>
```
