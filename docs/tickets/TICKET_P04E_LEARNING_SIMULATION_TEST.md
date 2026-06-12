# P04E 学习模拟端到端测试 (Learning Simulation Test)

状态：Queued

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

待填写。
