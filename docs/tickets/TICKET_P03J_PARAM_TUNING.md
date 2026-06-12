# P03J 参数自调优 v1 (Param Self-Tuning, B 类·重放途径)

状态：Completed

服务主命题环节：定位模糊 → 针对性补缺（预测参数被个人数据接管，掌握度判断更准 → 缺口定位更准）

## 背景

DATA_MODEL §12：事件溯源的红利是估计类参数可以反事实重放——换一组参数把全部历史 attempts 重新 fold 一遍，在时间序留出（最后 20%）上算目标指标，离线完成、零风险。参数三类制（SPEC §3 / DATA_MODEL §10）规定只有 B 类且调优途径=重放的参数可被自调优接管；A 类（含一切验证门槛）禁止触碰——系统不许给自己改及格线；MRT 途径参数（sched.w_\*、friction.lambda、fsrs.r_\*）没有反事实数据，不许重放调。

本票实现夜间自调优 job 的 v1：BKT 观测模型参数的三点网格重放 + provisional 启发式的直接回归，全程 `param_tuning_runs` 审计。

## 范围

1. 新增 `crates/polaris-core/src/tuning.rs`：
   - **可调参数白名单（硬编码 + 单测锁死）**，全部满足 class=B ∧ route=Replay：
     - 轮转槽位：`[bkt.p_init, bkt.slip, bkt.guess, bkt.guess_explain, bkt.learn, grade.provisional(双参数对)]`。
     - 显式排除：A 类全部（含 `tuning.*` 自身——调优器不许调自己的门）、Manual 路由（`bkt.cut_hi/cut_lo` 改标签语义）、MRT 路由（`sched.w_*` 等）。
   - **指标→负责参数映射**：
     - `bkt_holdout_logloss` → bkt.* 五参数：prequential 重放——全量 attempts 按 (created_at, id) 时间序逐条 fold（每概念独立 `MasteryState`，复用 `mastery::fold_attempt`），落在留出段（最后 `tuning.holdout_frac`）的 attempt 在 fold 前先预测 `P(正确) = p·(1−slip) + (1−p)·guess(task_type)`，对二值结果（score ≥ cut_hi → 1，≤ cut_lo → 0，死区跳过）计 logloss。score 取 `final ?? provisional`（与 §0 fold 语义一致）；概念级 `concepts.p_init` 覆盖优先于候选 `bkt.p_init`。
     - `provisional_holdout_mae` → grade.provisional_base + slope（按 DATA_MODEL §10 登记的途径"直接回归历史 (conf, final) 对"）：训练段最小二乘 `final ≈ base + slope·conf_norm`（系数 clamp [0,1]，零方差降级 slope=0），留出段 MAE 对比现值。
   - **搜索**：bkt 单参数 = 三点 `{clamp(cur−step), cur, clamp(cur+step)}`，step = 登记边界宽度/8，逐点重放取最优；provisional 对 = 回归闭式解。
   - **接受规则**：`improvement = metric(现值) − metric(候选) ≥ tuning.accept_margin` 才写 meta，否则保持原值；无论接受与否每个被评估参数写一行 `param_tuning_runs(id, ran_at, param, old_value, new_value, metric, delta, status)`（accepted|rejected）。
   - **每晚预算**：每次运行最多评估 `tuning.max_params_per_run` 个参数（DATA_MODEL §12 规则 2，单参数计 1、provisional 对计 2）；轮转游标 `tuning.rotation_cursor` 持久化于 meta，槽位指标数据不足时跳过该槽并记录原因；预算不够装下一槽（对）时停止且不越过该槽。
   - **数据门**：可用样本 < `tuning.min_attempts` 或留出段二值结果 < 5 → 该指标不评估（skip 留痕于返回摘要，不写审计行）。
2. 参数登记（config registry）：
   - `tuning.accept_margin = 0.005`（**A**，验证门槛，不调）。
   - `tuning.holdout_frac = 0.20`（**A**，同巩固留出口径，不调）。
   - `tuning.min_attempts = 30`（**A**，证据门槛，不调）。
   - `tuning.max_params_per_run = 2`（**A**，治理：每晚 1-2 个，不调）。
   - `tuning.rotation_cursor = 0`（C，运行状态，登记仅为初始化）。
3. 引擎与 CLI 接入：`Engine::run_param_tuning()`；CLI `polaris tune` 打印评估结果与跳过原因。
4. 不实现（留后续增量）：mirt.\*/calib.\*/gu.\* 指标族（映射表结构已预留扩展位）；"最差指标优先"排序（v1 为确定性轮转，待 ≥2 个可比指标族后再上）；夜间定时编排（job 由外部触发）。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03j_param_tuning
```

额外人工检查：

```powershell
git diff --check
```

验收要求：

- 样本不足 → 不评估、零审计行、meta 与游标不动。
- 系统性失败史 → bkt.guess 向更优值移动：accepted 行 + meta 更新 + delta>0。
- margin 不满足 → rejected 行 + meta 保持原值。
- provisional 有偏 (conf, final) 史 → 回归对被接受，base/slope 双行审计，meta 更新。
- 预算：对（cost 2）在剩余预算 1 时不评估、游标不越过；单参数逐槽轮转并回绕。
- 白名单安全：A 类、Manual、MRT 路由参数永不出现在审计行且 meta 不变；白名单单测锁死 class=B ∧ route=Replay。
- 决定性：同库同游标两次运行（第二次前还原 meta/游标/审计）结论一致。

## 禁区

- 不碰 A 类参数（含 `tuning.*` 自身、一切验证门槛）；不碰 Manual/MRT 路由参数。
- 每次运行改动参数数不得超过预算；不得绕过 margin 直接写 meta。
- 不实现 MRT 在线对比（P04C 范围）；不做调度/评分行为变更。
- 不修改冻结参考仓库。

## 交付记录

### 2026-06-13 开工记录

- 当前状态：P03I 已提交（`62840bd feat(P03I): 实现镜像报告 v1`）；本票按 QUEUE 顺序与用户确认的推荐序认领为唯一 In Progress。
- 工作区检查：认领前 `git status --short` 仅余未跟踪 `.cursor/`（用户本地配置，不动）。
- 本票范围：如上 1-4；预计修改面：新增 `tuning.rs`、`config.rs`（tuning.* 登记）、`engine.rs`（入口）、`lib.rs`（导出）、`polaris-cli/src/main.rs`（tune 命令）、新增 `tests/p03j_param_tuning.rs`。
- 禁区：见上；尤其 A 类/Manual/MRT 防护与预算上限。
- 验收命令：见上。

### 2026-06-13 交付记录

变更清单：

- `crates/polaris-core/src/tuning.rs`（新增）：自调优 job。白名单槽位 `[bkt.p_init, bkt.slip, bkt.guess, bkt.guess_explain, bkt.learn, grade.provisional(对)]`；bkt 参数三点网格 + prequential 留出 logloss（复用 `mastery::fold_attempt` 逐概念反事实重放，预测 `P(正确)=p·(1−slip)+(1−p)·guess`，二值结果 cut_hi/cut_lo，死区跳过）；provisional 对按登记途径直接最小二乘回归 (conf, final) 并以留出 MAE 对比；margin 接受规则 + `param_tuning_runs` 全量审计；轮转游标持久化、每晚参数预算、指标数据门。
- `crates/polaris-core/src/config.rs`：登记 `tuning.accept_margin`(A)、`tuning.holdout_frac`(A)、`tuning.min_attempts`(A)、`tuning.max_params_per_run`(A)、`tuning.rotation_cursor`(C)，附注册表单测。
- `crates/polaris-core/src/engine.rs`：新增 `run_param_tuning()` 入口。
- `crates/polaris-core/src/lib.rs`：导出 `tuning` 模块。
- `crates/polaris-cli/src/main.rs`：新增 `polaris tune` 命令（打印 accepted/rejected/skipped）。
- `crates/polaris-core/tests/p03j_param_tuning.rs`（新增）：8 个集成测试，覆盖样本不足零审计、失败史压低 bkt.guess（accepted + meta 更新）、高 margin 拒绝且 meta 不动、provisional 回归对接受（双行审计 + clamp）、预算不足跳过对且游标不越过、A/Manual/MRT 参数永不被触碰且审计行全在白名单内、游标轮转与回绕、同态决定性。
- `docs/tickets/QUEUE.md`：P03J 标记完成。

技术选择说明：

- score 序列取 `final ?? provisional`，与 DATA_MODEL §0 fold 语义一致；预测口径用 BKT 观测模型 `P(正确)`（候选 slip/guess 同时进入 fold 与预测），而非直接拿 p_known 充当预测——这是 likelihood 的正确形态。
- 留出为时间序最后 20%（`tuning.holdout_frac`，与巩固同口径）；prequential 评估：留出段每条 attempt 先预测后 fold，预测始终只用历史。
- `tuning.*` 治理参数全部登记为 A 类（调优器不许调自己的门）；白名单由单测锁死 class=B ∧ route=Replay，负面单测显式断言 cut_hi/cut_lo（Manual）、sched.w_\*/friction.lambda/fsrs.r_\*（MRT）、hazard.auc_gate（A）不可调。
- 轮转 v1 为确定性游标轮转；"最差指标优先"留待 ≥2 个可比指标族（票面第 4 条已声明）。预算语义：每次运行被评估（无论接受与否）的参数数 ≤ `tuning.max_params_per_run`，provisional 对计 2。
- 一次 `cargo test --workspace` 出现过 exit 101 且无任何测试输出（编译期瞬时失败，疑似 Windows 链接器文件锁——上一条命令的测试二进制刚释放）；紧随其后的两次全量跑均全绿，判定为环境瞬态而非测试不稳定。

验收输出：

```powershell
> cargo fmt --check
# exit 0, no output
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.71s
```

```powershell
> cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core --test p03j_param_tuning
running 8 tests
test failing_history_tunes_guess_downward_and_audits ... ok
test insufficient_history_skips_without_audit_rows ... ok
test pair_skipped_when_budget_insufficient ... ok
test provisional_pair_regression_accepted_when_biased ... ok
test tuning_is_deterministic_for_same_state ... ok
test cursor_rotation_advances_and_wraps ... ok
test gate_manual_and_mrt_params_never_touched ... ok
test high_margin_rejects_change_and_keeps_meta ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

```powershell
> git diff --check
# exit 0（仅 CRLF 行尾告警，无空白错误）
```

CLI 冒烟（空库数据门实跑）：

```powershell
> polaris init --pack packs/rust
initialized
> polaris tune
skipped all:insufficient_history(0<30)
```

回滚方式：

```powershell
git restore crates/polaris-core/src/config.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs crates/polaris-cli/src/main.rs docs/tickets/QUEUE.md
Remove-Item crates/polaris-core/src/tuning.rs, crates/polaris-core/tests/p03j_param_tuning.rs, docs/tickets/TICKET_P03J_PARAM_TUNING.md
```
