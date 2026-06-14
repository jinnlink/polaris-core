# P06C 属性测试扩面

状态：已完成

服务主命题：全环节（验证稳定性）

## 背景

Phase 3/4 已有相图、调度、掌握度 fold 的属性测试。强化轴线还要求补齐三块稳定性检查：G_u 生命周期决定性、镜像报告稳定字段决定性、HMM 滤波数值稳定。本票只加可靠性护栏，不改变学习算法与产品行为。

## 本轮范围

1. G_u 生命周期决定性：
   - 对同一批跨概念 pattern attempts，输入顺序变化后 `run_gu_induction` 的规则快照应一致。
   - 覆盖 candidate / validated / active / resolved 的稳定字段。
2. 镜像报告稳定字段决定性：
   - 对属性生成的 phantom 概念样本，连续两次 `run_mirror_report` 的稳定字段一致。
   - 忽略 `id`、`generated_at` 等本应变化的字段。
3. HMM 滤波数值稳定：
   - 对极端、非有限或长序列观测，`forward_filter` 输出始终 finite、归一化、非负。

## 验收

必须通过：

```powershell
cargo test -p polaris-core p06c
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不改 BKT/FSRS/MIRT/相图/HMM/G_u/报告公式。
- 不调整阈值、门槛或参数默认值。
- 不新增 LLM、HTTP、MCP 或后台任务能力。
- 不处理 `.gitignore`、`.cursor/`、`docs/visuals/` 等票外改动。
- 不修改 frozen 参考仓库。

## 交付记录

### 变更清单

- 新增 `crates/polaris-core/tests/p06c_property_expansion.rs`。
- 新增 3 条属性测试：
  - `p06c_gu_lifecycle_is_deterministic_under_attempt_insertion_order`：同一批 G_u attempt 正序/逆序插入后，candidate / validated / active / resolved 稳定快照一致，并显式断言每阶段期望状态、生命周期状态序列、alpha/beta、correct_streak 与 confusion edge 数。
  - `p06c_mirror_report_stable_fields_are_deterministic_for_generated_phantoms`：生成 phantom 概念样本后，连续两次报告的稳定字段一致；稳定快照覆盖 schema/week/window、assertions/hypotheses/suggestions 的 kind/subject/stats、skipped、hazard gate 与 reflection prompts，并确认报告 `id` 会变化。
  - `p06c_hmm_filter_stays_finite_normalized_for_extreme_sequences`：对含 `NaN`、正负无穷、极大值和最长 127 步序列的 HMM 观测，滤波后验始终有限、归一、非负，且同输入同输出。
- 更新 `docs/tickets/QUEUE.md`，将 roadmap 候选转为 P06C 正式票并标记 In Progress。

### 红灯记录

```text
cargo test -p polaris-core p06c
error[E0382]: use of moved value: `posterior`
exit 1
```

说明：首轮红灯是新增测试自身的 move 语义错误，修正为比较引用后进入真实属性测试。未发现需要改生产代码的失败。

```text
cargo test -p polaris-core p06c
error: there is no argument named `snapshot`
error[E0382]: borrow of moved value: `left`
error[E0507]: cannot move out of `snapshot.lifecycle_statuses`
exit 1
```

说明：处理子 agent 审查后补强断言时暴露测试宏格式参数与借用错误；修正为显式格式参数、引用比较与 `Vec<String>` 期望值。

### 验收输出

```text
cargo test -p polaris-core p06c
p06c_hmm_filter_stays_finite_normalized_for_extreme_sequences ... ok
p06c_mirror_report_stable_fields_are_deterministic_for_generated_phantoms ... ok
p06c_gu_lifecycle_is_deterministic_under_attempt_insertion_order ... ok
3 passed; 0 failed
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

```text
cargo clippy --workspace --all-targets -- -D warnings
failed: Windows target 目录文件锁，报错为 target/debug/deps/libpolaris_core-*.rmeta 写入被拒绝（os error 5），并伴随 incremental 目录 GC warning。
exit 1
```

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.93s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 27 passed
polaris-core unit: 68 passed
integration suites: all passed, including p03h_gu_induction 8 passed, p03i_mirror_report 14 passed, p03k_mental_fit 7 passed, p06c_property_expansion 3 passed
doc-tests: 0 passed
exit 0
```

### 技术选择

- P06C 不修改生产代码，只把现有确定性约束固化为属性测试。
- G_u 快照忽略随机 lifecycle event id 与 `updated_at`，只比较规则语义字段、图边数量与生命周期状态序列。
- 镜像报告稳定字段只比较报告 item 的稳定语义字段，显式排除 `MirrorReport.id` / `generated_at`。
- HMM 属性测试覆盖非有限输入，但只要求后验有限、归一、非负和确定性，不新增策略门或公式变更。

### 子 agent 审查

Mendel（`019ec53a-7b93-7b83-8093-f5acd2cd3f90`）只读审查结论：

- Critical：无。
- Important 1：G_u 生命周期测试只比较左右一致，可能同错同过；已补每阶段期望状态、生命周期序列、alpha/beta、correct_streak 和 confusion edge 断言。
- Important 2：镜像报告稳定字段取样过窄；已扩展到 `MirrorReport` 稳定外层字段、item 的 `kind`/`subject`/`stats`、`skipped`、`hazard_gate` 与 `reflection_prompts`。
- Important 3：票外文件污染风险；提交时只 stage P06C 文件，禁用 `git add -A`。
- Minor 1：HMM 长序列偏短；已将生成序列上限从 31 步扩到 127 步。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md
git clean -f docs/tickets/TICKET_P06C_PROPERTY_TEST_EXPANSION.md
git restore crates/polaris-core/tests/p06c_property_expansion.rs
```

提交后：

```powershell
git revert <P06C-commit-sha>
```
