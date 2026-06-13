# P04E 学习模拟端到端测试 (Learning Simulation Test)

状态：Completed

服务主命题环节：验证真懂 → 定位模糊 → 针对性补缺（全闭环验证）

## 背景

学习系统最致命的 bug 不是崩溃，而是"看起来在跑、实际没在教"——调度死锁、掌握度不涨、相变不合理、HMM 状态卡死。这些问题需要端到端模拟才能暴露。当前单元测试和集成测试验证的是模块正确性，缺少"一个虚拟学习者用 30 天能不能真的学会"的全局断言。

本票实现虚拟学习者模拟器，是内核最重要的集成测试。模拟器可配置不同能力画像（强/弱/混合），跑完整 30 天学习轨迹，验证闭环系统的宏观属性。

## 范围

1. 虚拟学习者模型：
   - `VirtualLearner` 结构体，配置项：
     - `ability: Vec<f64>`（K 维真实能力向量，模拟 θ_true）。
     - `noise: f64`（响应噪声标准差，模拟 slip/guess 变异）。
     - `confidence_bias: f64`（自信偏差，正=过度自信，负=不自信）。
     - `fatigue_rate: f64`（每 session 疲劳累积速率）。
     - `session_pattern: Vec<usize>`（30 天每天 session 数量）。
   - 预设画像：
     - `strong`：ability 全 1.5，noise 0.1，confidence_bias 0.0。
     - `weak`：ability 全 −0.5，noise 0.3，confidence_bias +0.3（弱且过度自信）。
     - `mixed`：ability 交替 1.0/−0.5（部分维度强/弱），noise 0.2。

2. 响应生成：
   - 给定 concept_id 和 task_type，虚拟学习者按 `P(correct) = σ(q_c · ability - b_c - d_t + N(0, noise))` 生成 score。
   - self_confidence = `clamp(1 + 4·σ(q_c · ability - b_c + confidence_bias), 1, 5)` 取整。
   - latency_ms = `max(500, base_latency · (1 + fatigue_factor) · (1 + difficulty_factor) + N(0, 500))`。

3. 模拟循环：
   - `simulate_learning(learner: &VirtualLearner, days: usize, engine: &mut Engine) -> SimulationReport`。
   - 每天：
     a. 按 session_pattern 创建 session。
     b. 每 session 调用 `get_next_task`（或 `get_interleaved_batch` 若已实现）获取任务。
     c. 虚拟学习者生成 response score/confidence/latency。
     d. `Engine::submit` 提交 attempt。
     e. 模拟 grading（直接用 response score 作为 final_score，跳过 LLM grader）。
     f. 每天结束后可选触发 nightly consolidation。

4. 断言（SimulationReport 的验证）：
   - **掌握度单调性**：strong 学习者 30 天后 `mean(p_known)` ≥ 0.7；weak 学习者 `mean(p_known)` 应持续缓慢上升（斜率 > 0）。
   - **无调度死锁**：每天每 session 都能取到 ≥1 个任务（不得返回空）。
   - **HMM 状态转移合理性**：不出现某状态占比 >90% 超过连续 5 天（卡死检测）。
   - **相变合理性**：strong 学习者至少有概念达到 Transfer 相；weak 学习者不应在 attempt_count < 5 时出现 Transfer 相。
   - **校准收敛**：过度自信的 weak 学习者 `calib_gap` 应在 30 天内收敛（绝对值减小）。
   - **MIRT θ 跟踪**：strong 学习者的 θ 与 ability 的余弦相似度应 > 0.5。

5. 测试入口：
   - `tests/p04e_simulation.rs`，含 3 个测试用例（strong/weak/mixed），每个跑 30 天模拟。
   - 测试预算：单个模拟 <10s（纯 Tier 0 无 LLM）。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p04e_simulation -- --nocapture
```

额外人工检查：

```powershell
git diff --check
```

验收要求：
- 3 个画像（strong/weak/mixed）的 30 天模拟全部通过上述断言。
- 无 panic、无死循环、无 SQLite 死锁。
- SimulationReport 输出包含每日 summary（平均 p_known、active concepts、dominant HMM state、phase distribution）。
- 模拟不依赖 LLM 或网络。

## 禁区

- 不修改引擎核心逻辑——模拟器只是引擎 API 的消费者。
- 不引入外部测试框架或 benchmark harness。
- 不模拟 LLM grading——直接以虚拟学习者 score 作为 final_score。
- 不修改冻结参考仓库。

## 交付记录

## AI 交付记录（2026-06-13）

### 本轮范围

- 按单票制认领 P04E，只实现学习模拟端到端测试与模拟器支撑代码。
- 不修改调度、掌握度 fold、MIRT、HMM、相图判定等核心引擎规则。
- 模拟器作为 `Engine` API 消费者：通过 `get_interleaved_batch`/`submit`/`apply_final_score` 跑完整闭环；最终评分由虚拟学习者直接写入 final_score，不依赖 LLM 或网络。

### 变更清单

- 新增 `crates/polaris-core/src/simulation.rs`
  - `VirtualLearner`：包含 `ability`、`noise`、`confidence_bias`、`fatigue_rate`、`session_pattern`，并提供 `strong`/`weak`/`mixed` 三个预设。
  - `simulate_learning`：运行 30 天端到端模拟，输出 `SimulationReport` 与每日 summary。
  - 模拟期间临时屏蔽 `POLARIS_LLM_FAST_*` / `POLARIS_LLM_STRONG_*` 环境变量，避免 `Engine::submit` 触发外部 LLM。
  - 为当前 pack 概念写入模拟用 K 维 q 面与轻量难度，保证 MIRT θ 跟踪断言有可观测维度。
- 更新 `crates/polaris-core/src/lib.rs` 导出 `simulation` 模块。
- 新增 `crates/polaris-core/tests/p04e_simulation.rs`
  - strong：断言 30 天后 `mean(p_known) >= 0.7`、至少一个概念到 Transfer、θ/ability 余弦 > 0.5、无死锁、HMM 不长期卡死。
  - weak：断言 `mean(p_known)` 斜率 > 0、校准 gap 绝对值收敛、attempt_count < 5 时不进入 Transfer、无死锁、HMM 不长期卡死。
  - mixed：断言闭环持续运行、平均掌握度上升、每日 summary 含 active concepts 与 phase distribution。
- 更新 `docs/tickets/QUEUE.md`：P04E 从 Queued 改为 In Progress。

### 验收输出

```powershell
> cargo fmt --check
# exit code: 0
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
warning: failed to garbage collect incremental compilation session directory `\\?\C:\MyProject\polaris-core\target\debug\incremental\...`: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: could not compile `polaris-core` (lib) due to 1 previous error
# exit code: 1
```

默认 `target/debug` 目录存在 Windows 文件锁/访问拒绝，非代码告警。使用隔离 target 重跑同一 Clippy 检查：

```powershell
> $env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-target-clippy'; cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.49s
# exit code: 0
```

```powershell
> cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.75s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    Finished `test` profile [unoptimized + debuginfo] target(s) in 28.26s
# exit code: 0
```

```powershell
> cargo test -p polaris-core --test p04e_simulation -- --nocapture
running 3 tests
day 01: mean_p_known=0.272 active_concepts=4 dominant_hmm=fatigued phase_distribution=PhaseCounts { counts: {"fluctuation": 2, "undetermined": 22} }
...
day 30: mean_p_known=0.520 active_concepts=23 dominant_hmm=bored phase_distribution=PhaseCounts { counts: {"fluctuation": 1, "transfer": 5, "undetermined": 18} }
test mixed_virtual_learner_keeps_running_and_reports_daily_summaries ... ok
...
day 30: mean_p_known=0.295 active_concepts=24 dominant_hmm=fatigued phase_distribution=PhaseCounts { counts: {"fluctuation": 2, "regression": 5, "transfer": 3, "undetermined": 14} }
test weak_overconfident_virtual_learner_improves_without_early_transfer ... ok
...
day 30: mean_p_known=0.998 active_concepts=24 dominant_hmm=fatigued phase_distribution=PhaseCounts { counts: {"generation": 1, "transfer": 12, "undetermined": 11} }
test strong_virtual_learner_reaches_transfer_without_deadlock ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.68s
# exit code: 0
```

```powershell
> git diff --check
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/TICKET_P04E_LEARNING_SIMULATION_TEST.md', LF will be replaced by CRLF the next time Git touches it
# exit code: 0
```

### 阻塞与裁决记录

- 默认 `target/debug` 上的原始 Clippy 命令仍被 Windows 文件锁挡住，表现为 rmeta 写入 `拒绝访问 (os error 5)`。
- 已确认没有残留 `cargo`/`rustc`/`clippy`/`rustdoc` 进程；同一 Clippy 检查在 `%TEMP%\polaris-core-target-clippy` 隔离 target 下通过。
- 未执行 `cargo clean` 或删除 `target/`，避免破坏用户已有构建产物。

### 回滚方式

- 回滚本票代码与测试：删除 `crates/polaris-core/src/simulation.rs`、`crates/polaris-core/tests/p04e_simulation.rs`，并从 `crates/polaris-core/src/lib.rs` 移除 `pub mod simulation;`。
- 回滚票据状态：将 `docs/tickets/QUEUE.md` 中 P04E 从 `In Progress` 改回 `Queued`，删除本交付记录。
