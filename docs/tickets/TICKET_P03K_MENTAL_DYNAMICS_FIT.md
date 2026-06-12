# P03K 心智动力学拟合层激活 (Mental Dynamics Fit)

状态：Completed

服务主命题环节：定位模糊 → 针对性补缺（放弃风险与状态感知从"记录"升级为"过门后可调策略"）

## 背景

P03D 落了状态 HMM 滤波与 hazard 记录层，P03I 落了镜像报告的 hazard AUC 门，但三个已登记的门至今是死字段：

1. `hazard.auc_gate`（0.70，A 类）：`fit_hazard_model` 纯函数存在且有单测，但引擎从未拟合过——快照里 β 恒为零向量、`validation_auc` 恒 null、`model_status='unfit'`，hazard 永远不参与调度与镜像报告（DATA_MODEL §7：AUC ≥ 0.70 才允许参与）。
2. `hmm.gate_auc_margin`（0.03，A 类）：状态层门控（"状态后验对'下一动作'的预测 AUC 必须比无状态基线高 ≥ 0.03，否则只记录、不得调策略"）从未被评估——`strategy_enabled` 硬编码 false，`observed_auc_margin` 恒 null，P03G 的状态感知 batch 实际永远走 Default。
3. `hmm.em_min_n`（200，B 类）：转移矩阵 EM 重估（"每周一次且 graded ≥ 200 才启用"）未实现——转移矩阵恒为先验常量。

本票把三者实现为一个周拟合 job，全程审计，门不过则行为与现状完全一致（graceful degradation）。

## 范围

1. 新增 `crates/polaris-core/src/mental_fit.rs`，入口 `run_mental_dynamics_fit(conn)`，三个子任务各自独立跳过/成功：
   - **hazard 周拟合**：训练样本 = 历史 `mental_state`（score_source='provisional'）事件的 `hazard.inputs`（12 维已存）；标签 = 同会话 10 分钟内出现 abandon 事件；时间序留出（`hazard.holdout_frac`）；`fit_hazard_model`（L2 逻辑回归，超参 `hazard.fit_l2/.fit_iterations/.fit_lr`）；样本 < `hazard.fit_min_n` 或验证集单类 → 跳过。结果追加写新表 `hazard_models(id, fitted_at, beta_json, validation_auc, n_train, n_validation)`。
   - **状态层门控评估**：目标 = "下一动作"二值化（事件后 10 分钟内出现 hint/abandon = 非继续）；状态模型 = 全 12 维输入；无状态基线 = 同输入但 6 维状态后验替换为均匀分布（移除信息、保留隐式截距，同一拟合器，无第二实现）；两者同一时间序留出上算 AUC；`margin = AUC_state − AUC_base`；过门 iff margin ≥ `hmm.gate_auc_margin`。结果追加写新表 `state_gate_evals(id, evaluated_at, baseline_auc, state_auc, margin, passes, n)`。
   - **EM 重估（转移矩阵）**：graded attempts < `hmm.em_min_n` → 跳过；否则从 `mental_state` 事件按会话重建观测序列，固定发射先验（DATA_MODEL §7 表是冻结先验），Baum-Welch 仅重估 6×6 转移矩阵（固定迭代次数，行归一 + 下限防吸收），写 meta `hmm.transitions`（C 类，登记仅为初始化语义）。
2. 引擎消费端接线（`engine.rs`）：
   - 快照 hazard 估计改用最新 `hazard_models` 行（β + validation_auc），`model_status` 如实反映 fitted/unfit。
   - `strategy_enabled` = 最新 `state_gate_evals.passes`；`state_gate.observed_auc_margin` 如实写入——过门后 P03G 状态感知 batch 自动解锁，不过门行为与现状逐字节一致。
   - 前向滤波改用 meta `hmm.transitions`（空/非法 → 内置先验，向后兼容）。
3. `mental_state.rs`：`forward_filter_with_transitions` + `reestimate_transitions`（纯函数，单测合成粘滞序列对角增大）；`forward_filter` 保持原签名为先验包装。
4. 镜像报告（`report.rs`）：新增 `hazard_risk_summary` 断言挖掘——**仅当** hazard 门通过（payload 实测 AUC ≥ 门）才生成；claim = 窗口内均值/峰值即时放弃风险 + 模型 AUC；置信度 = validation_auc；证据 = mental_state 事件 id；min_evidence 与红线过滤照常适用。门未过 → 整类缺席（P03I 既有测试继续锁死该行为）。
5. 参数登记：`hazard.fit_l2 = 0.01`（B，[0.001,0.1]，手动）、`hazard.fit_iterations = 300`（B，[50,2000]，手动）、`hazard.fit_lr = 0.5`（B，[0.01,2.0]，手动）、`hazard.fit_min_n = 50`（**A**）、`hazard.holdout_frac = 0.20`（**A**）、`hmm.transitions = "[]"`（C，运行态）。
6. CLI：`polaris mental-fit` 打印三个子任务的结果/跳过原因。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03k_mental_fit
```

额外人工检查：

```powershell
git diff --check
```

验收要求：

- 数据不足 → 三子任务各自跳过、零新表行、meta 不动、快照行为与现状一致。
- 可分离的 abandon 史 → hazard_models 行写入且 AUC>0.5；后续快照 payload 携带真实 validation_auc；AUC ≥ 0.70 时 participates=true。
- 状态后验携带基线之外信息的种子数据 → margin ≥ 0.03、passes=1、后续快照 strategy_enabled=true；基线即可分离的数据 → margin 小、不过门、strategy_enabled=false。
- graded ≥ 200 → hmm.transitions 写入合法 6×6（行和=1±1e-9），先验包装滤波不受影响；< 200 → 不写。
- 门通过后镜像报告出现 hazard_risk_summary（带事件证据 + AUC 置信度）；未过门绝不出现（既有 P03I 测试不回归）。
- 决定性：同库两次拟合（清表后）结论一致。

## 禁区

- 不改发射先验表与 6 状态构念（DATA_MODEL §7 冻结）；不引入临床标签。
- 门未过时不得让状态/hazard 影响任何调度、评分、报告行为。
- 不实现挫败前瞻 10 分钟预测、镜像报告叙事润色（后续票）。
- 不修改冻结参考仓库。

## 交付记录

### 2026-06-13 开工记录

- 当前状态：P03J 已提交（`d1e9436`）；本票按用户确认的推荐序认领为唯一 In Progress（QUEUE Backlog 提案转正式票）。
- 工作区检查：`git status --short` 仅余未跟踪 `.cursor/`（用户本地配置，不动）。
- 预计修改面：新增 `mental_fit.rs`、`db.rs`（hazard_models / state_gate_evals 表）、`config.rs`（6 个登记）、`mental_state.rs`（带参滤波 + EM 纯函数）、`engine.rs`（快照消费 + 入口）、`report.rs`（hazard_risk_summary 挖掘）、`lib.rs`、CLI、新增 `tests/p03k_mental_fit.rs`。
- 禁区与验收命令：见上。

### 2026-06-13 交付记录

变更清单：

- `crates/polaris-core/src/mental_fit.rs`（新增）：周拟合 job 三子任务。hazard 拟合（样本=历史快照 `hazard.inputs`，标签=同会话 10 分钟内 abandon，时间序留出，复用 `fit_hazard_model`，结果写 `hazard_models`）；状态层门控评估（目标="下一动作"非继续 = hint/abandon 跟随；基线=状态后验替换为均匀分布——移除信息但保留隐式截距，同一拟合器无第二实现；margin 过 `hmm.gate_auc_margin` 写 `state_gate_evals`）；EM 重估（graded ≥ `hmm.em_min_n` 时按会话重建观测序列，Baum-Welch 仅重估转移矩阵写 meta `hmm.transitions`）。各子任务数据不足独立跳过。
- `crates/polaris-core/src/mental_state.rs`：`forward_filter_with_transitions` + `prior_transitions` + `reestimate_transitions`（缩放前向-后向，发射先验冻结，行下限 0.01 经线性收缩保证 + 行归一），原 `forward_filter` 退化为先验包装；EM 单测（粘滞序列对角占优、随机矩阵性质、退化输入回先验）。
- `crates/polaris-core/src/db.rs`：新增 `hazard_models`、`state_gate_evals` 表。
- `crates/polaris-core/src/config.rs`：登记 `hazard.fit_l2/.fit_iterations/.fit_lr`（B）、`hazard.fit_min_n`（A）、`hazard.holdout_frac`（A）、`hmm.transitions`（C）。
- `crates/polaris-core/src/engine.rs`：快照路径消费最新拟合结果——hazard β/validation_auc/model_status 来自 `hazard_models`；`strategy_enabled` 与 `observed_auc_margin` 来自 `state_gate_evals`（过门即真，P03G 状态感知 batch 自动解锁）；滤波用 meta 转移矩阵（非法/空回先验）；新增 `run_mental_dynamics_fit()` 入口。无任何拟合数据时行为与 P03I 版本逐字段一致。
- `crates/polaris-core/src/report.rs`：新增 `hazard_risk_summary` 断言（仅 hazard 门通过时挖掘窗口内 fitted 快照的风险均值/峰值；置信度=validation_auc；证据=mental_state 事件 id；红线照常）。
- `crates/polaris-cli/src/main.rs`：新增 `polaris mental-fit`。
- `crates/polaris-core/tests/p03k_mental_fit.rs`（新增）：7 个集成测试——数据不足三跳过且零表行、可分离放弃史拟合出高 AUC 且快照消费（fitted/participates）、状态后验携带独有信息时过门且 strategy_enabled=true、基线已可分离时不过门且 strategy_enabled=false、graded≥200 时 EM 写合法随机矩阵且滤波路径可用、镜像报告 hazard 摘要仅过门后出现、同态决定性。
- `docs/tickets/QUEUE.md`：P03K 标记完成。

技术选择说明：

- 门控基线用"均匀后验替换"而非"清零"：状态后验恒和为 1，在无截距的逻辑回归里充当隐式截距；清零会同时移除截距使基线被不公平削弱，均匀替换只移除信息量，比较才公平。票面措辞已按此修正。
- 转移矩阵行下限用线性收缩 `p' = floor + (1−6·floor)·p` 而非"截断后再归一"：后者归一会把floored 项压回下限之下，前者精确保证 ≥ floor 且行和为 1（首次实现曾犯此错，被单测捕获后修正）。
- EM 的发射均值表保持 DATA_MODEL §7 冻结先验，只重估转移——n=1 数据量下全参数 EM 不可辨识，这是票面明确的边界。
- 实现过程记录：一次用 PowerShell 管道改测试文件导致中文注释 mojibake，整文件重写恢复；教训=文件编辑只走编辑工具，不走 shell。

验收输出：

```powershell
> cargo fmt --check
# exit 0, no output
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
# exit 0（首跑揪出 needless_range_loop，改 zip 迭代后通过）
```

```powershell
> cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core --test p03k_mental_fit
running 7 tests
test insufficient_data_skips_all_three_tasks ... ok
test state_gate_passes_when_posterior_carries_information ... ok
test em_reestimates_transitions_with_enough_graded_attempts ... ok
test state_gate_fails_when_baseline_already_separates ... ok
test separable_abandon_history_fits_hazard_model_and_snapshot_consumes_it ... ok
test mirror_report_includes_hazard_summary_only_after_gate_passes ... ok
test fit_is_deterministic_for_same_state ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
```

```powershell
> git diff --check
# exit 0（仅 CRLF 行尾告警，无空白错误）
```

CLI 冒烟（空库三跳过实跑）：

```powershell
> polaris init --pack packs/rust
initialized
> polaris mental-fit
hazard: skipped insufficient_samples(0<50)
state_gate: skipped insufficient_samples(0<50)
em: skipped insufficient_graded(0<200)
```

回滚方式：

```powershell
git restore crates/polaris-core/src/config.rs crates/polaris-core/src/db.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs crates/polaris-core/src/mental_state.rs crates/polaris-core/src/report.rs crates/polaris-cli/src/main.rs docs/tickets/QUEUE.md
Remove-Item crates/polaris-core/src/mental_fit.rs, crates/polaris-core/tests/p03k_mental_fit.rs, docs/tickets/TICKET_P03K_MENTAL_DYNAMICS_FIT.md
```
