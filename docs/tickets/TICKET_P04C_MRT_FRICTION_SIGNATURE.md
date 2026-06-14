# P04C MRT 微随机化 + 签名后验 + 摩擦曲线

## 状态

Done

## 服务主命题

验证真懂 -> 定位模糊 -> 针对性补缺。

## 背景

P03F/P03G 已经有 Bloom move 和交错调度，P04B 已经打开本地 HTTP 门。DATA_MODEL §8 冻结了 P04C 的三件事：MRT 预登记审计、签名后验收缩估计、个人摩擦曲线。本票只把这些机制接入本地 `next` 选 move 决策点，不做 UI、不做 LLM、不改变概念调度排序。

## 范围

1. 在 `polaris-core` 增加 P04C move 选择层：
   - 决策点在 `Engine::next_task` 选定 concept 后、生成 prompt 前。
   - 不改变 `ranked_task_candidates()` 的概念排序和 U(c) 算法。
   - 输出仍保持 `NextTask` 现有字段，不要求 UI/HTTP/MCP 改契约。
2. MRT 微随机化：
   - 默认读取 `mrt.epsilon`。
   - 预登记 JSON 写入 `mrt_log`，包含窗口、epsilon、候选集、context_hash、主效应假设、最小样本说明。
   - 随机化记录 `randomized=1`，非随机基线记录 `randomized=0`。
3. 教法签名后验：
   - 读取 `moves_effects(move, context_hash)` 的 Beta 样本。
   - 使用 `thompson.prior_n` 加收缩先验，避免小样本过度摆动。
   - `apply_final_score` 对已评分 attempt 写入同一 `context_hash` 下的 `moves_effects`，使后验可随真实成绩更新。
4. 摩擦指数：
   - 按 DATA_MODEL §8 固定权重读取 `friction.w1..w4`，计算 0..1 的 friction score。
   - move utility 使用 `posterior_mean - friction.lambda * friction`。
   - 不自调 `friction.w1..w4`；`friction.lambda` 只读取，不在本票调参。

## 禁区

- 不实现 Tauri/Web UI。
- 不引入 LLM 或网络调用。
- 不改掌握度 fold、phase 判据、概念 U(c) 排序公式。
- 不新增领域特定逻辑；move 集合仍来自既有 Bloom move / pack move 语义。
- 不让外部评分直接写 mastery、theta、moves_effects。
- 不修改冻结参考仓库。

## 验收

```powershell
cargo test -p polaris-core --test p04c_mrt
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如默认 target 的 clippy 遇到 Windows 文件锁，可使用隔离 target 重跑同参数，并在交付记录写明。

## 本轮范围（2026-06-14）

- 新增 P04C core 模块与专测。
- `next_task` 接入 move 选择层并写 `mrt_log`。
- `apply_final_score` 将 final outcome 汇入 `moves_effects`。
- 只改 `polaris-core` 与票据/队列，不改 CLI/HTTP 契约。

## 交付记录（2026-06-14）

### 变更清单

- 新增 `crates/polaris-core/src/pedagogy.rs`：在 concept 排序之后、prompt 生成之前执行 P04C move 选择层；写入 MRT 预登记 `mrt_log`，读取 `moves_effects` + `thompson.prior_n` 后验，并按 `posterior_mean - friction.lambda * friction` 排序候选。
- `Engine::next_task` 现在返回 MRT 决策元数据；新增 `Engine::record_next_task_event`，CLI/HTTP/MCP 的 next 入口统一把 `mrt_prereg_id`、`mrt_context_hash`、`move` 写入 `behavior_events(type='next')`。
- final outcome 不再按评分后的 state/phase 重算 context，而是从同 session/concept/task_type 的 next 事件恢复预登记链路；7 天窗口内同概念成功写 success，窗口未过期的失败不写 beta，窗口过期且无成功才写 failure。
- 补 `crates/polaris-core/tests/p04c_mrt.rs` 覆盖预登记、cold-start MRT、签名后验、7d success/failure、same-concept success 回填、context/prereg 复用。
- 稳定旧测试：P03B residual 夹具改为相对当前日期的同 ISO week；P03F/P03G 纯旧调度测试显式 `mrt.epsilon=0`，避免默认 MRT 探索影响历史契约。

### 子 agent 审查

- 第一轮审查发现两个阻塞问题：outcome 未复用 next 预登记 context；预登记写 7d 但实现用即时分数。已修复。
- 第二轮审查发现 7d 窗口内“同概念另一 attempt 成功”未回填 success。已补 fan-out 和过期扫描 success 回填，并增加专测。
- 剩余已知风险：当前 submit 仍未显式携带 assignment/prereg id；短期按 `session_id + concept_id + task_type + at` 归因，长期 UI/API 可显式传 `mrt_prereg_id` 进一步收紧。

### 验收输出

```powershell
> cargo test -p polaris-core --test p04c_mrt
running 8 tests
test cold_start_mrt_randomization_can_replace_the_base_move ... ok
test forced_mrt_randomization_replaces_selected_move_and_marks_audit ... ok
test next_task_writes_mrt_preregistration_audit ... ok
test signature_posterior_can_select_non_default_move_without_randomization ... ok
test seven_day_success_updates_the_preregistered_context ... ok
test failing_attempt_waits_for_the_seven_day_window_before_beta_update ... ok
test later_same_concept_success_settles_pending_preregistration ... ok
test expired_window_without_success_records_failure ... ok
test result: ok. 8 passed; 0 failed

> cargo fmt --check
exit code: 0

> cargo clippy --workspace --all-targets -- -D warnings
error: failed to write ... target\debug\deps\libpolaris_core-*.rmeta: 拒绝访问。 (os error 5)

> cargo clippy --workspace --all-targets --target-dir "$env:TEMP\polaris-p04c-clippy-serial" -j 1 -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.98s

> cargo test --workspace
test result: ok. 23 passed; 0 failed
test result: ok. 66 passed; 0 failed
...
test result: ok. 5 passed; 0 failed
Doc-tests polaris_core: ok
```

### 回滚方式

```powershell
git revert <P04C-commit>
```
