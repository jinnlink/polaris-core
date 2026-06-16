# P03O BKT-MIRT 融合不确定度传播 shadow gate

状态：In Progress（2026-06-17）
服务主命题环节：定位模糊（抽象引擎 p_known 的可信度表达）

## 背景

`DATA_MODEL.md` §4 当前定义的 BKT-MIRT 融合是：

```text
p̂_known = λ·BKT + (1-λ)·σ(q·θ-b-d_t)，λ = n_c/(n_c+5)
```

`ENHANCEMENT_ROADMAP.md` 将「融合不确定度传播」列为数学深化候选：BKT 路与 MIRT 路各带后验方差，用逆方差加权得到 shadow 融合，并输出不确定度，供镜像报告和后续探针任务派发消费。当前代码的单一出口是 `crates/polaris-core/src/mirt.rs::fused_p_known`，调度仍消费其中 `p_known`。

本票只做 shadow gate：计算并测试逆方差融合与不确定度，但不替换主 `p_known`，不改变调度或产品行为。

## 范围

1. 扩展 MIRT 融合输出：
   - `FusedPKnown` 保留现有 `p_known`、`bkt_p_known`、`mirt_p_hat`、`lambda` 语义不变。
   - 新增 shadow 字段，表达逆方差融合结果、BKT 方差、MIRT 方差、shadow 权重与融合方差。
   - 无有效方差或样本不足时，shadow 必须可解释地退回当前 λ 融合。
2. 方差定义：
   - BKT 方差使用 Bernoulli posterior 工程近似，随 `attempt_count` 增加而收缩，并设置 floor。
   - MIRT 方差使用 θ AdaGrad `g2` 与 q 的工程近似，不把 `g2` 宣称为严格协方差；必须设置 floor/fallback。
   - 所有概率和方差结果必须 finite、clamp 到合理范围。
3. 测试：
   - 旧 λ 融合的 `p_known` 不变。
   - shadow 逆方差融合在有效方差下可计算，并位于 BKT 与 MIRT 两路之间。
   - BKT 样本数提高时 BKT 方差下降。
   - MIRT `g2` 累积提高时 MIRT 方差下降。
   - 无效/缺失方差时退回旧 λ 融合。
4. 文档：
   - `DATA_MODEL.md` §4 增加 shadow gate 说明，明确当前不改变主融合行为。
   - `QUEUE.md` 标记本票 In Progress。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p03a_mirt
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- `fused_p_known(...).p_known` 对既有 fixture 保持旧 λ 行为。
- 新增 shadow 字段不被 `task_selection` 用作主调度依据。
- 不修改数据库 schema，不提升 `CURRENT_SCHEMA_VERSION`。
- 不修改 BKT、FSRS、MRT、G_u、报告或调度公式。

## 禁区

- 不把 inverse-variance shadow 融合切成默认 `p_known`。
- 不把 AdaGrad `g2` 宣称为严格统计协方差。
- 不新增 DDL、meta 参数或迁移。
- 不做留出训练 job，不写 `param_tuning_runs`。
- 不改 `task_selection`、`submit_pipeline`、报告生成、MCP/HTTP 行为。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 本轮范围（2026-06-17）

- 当前状态：P11B 已提交（`dd75360`），QUEUE 无 In Progress；本票按 `ENHANCEMENT_ROADMAP.md` 的「融合不确定度传播」候选转正式票并认领。
- 已有非本票改动：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`、`target_codex_reviewNndQmJ/`。本票不得回退或混入。
- 子 agent 数学审查结论：BKT-MIRT 逆方差融合最适合下一张小而硬的数学票；必须先 shadow 计算，默认不改变行为。
- 预计修改面：`crates/polaris-core/src/mirt.rs`、`crates/polaris-core/src/engine.rs`（仅类型导入如需要）、`crates/polaris-core/tests/p03a_mirt.rs`、`docs/DATA_MODEL.md`、`docs/tickets/QUEUE.md` 和本票文件。

## 交付记录（2026-06-17）

变更清单：
- `crates/polaris-core/src/mirt.rs`：扩展 `FusedPKnown`，新增 `shadow_p_known`、`bkt_variance`、`mirt_variance`、`shadow_bkt_weight`、`shadow_variance`、`shadow_uses_inverse_variance`；主 `p_known` 保持旧 `λ·BKT + (1-λ)·MIRT` 不变。
- `crates/polaris-core/src/mirt.rs`：新增 BKT Bernoulli posterior 工程方差、MIRT `q × g2` 工程方差、逆方差 shadow 融合与 fallback；预测路径只保证 θ row，不修复坏 `g2`，让 shadow 可诊断无效方差。
- `crates/polaris-core/tests/p03a_mirt.rs`：新增 shared 与 isolated pack 的 shadow 覆盖，验证旧 λ 不变、无 BKT evidence fallback、BKT 方差随样本数下降、MIRT 方差随 `g2` 信息量下降、坏 shared/isolated `g2` fallback。
- `docs/DATA_MODEL.md`：补齐主融合公式 `d_t`，增加 P03O shadow gate 说明，明确 `g2` 只是工程信息量近似，`g2=0` 是高不确定度冷启动估计。
- `docs/tickets/QUEUE.md`：P03O 标记为已实现并通过验收。

子 agent 审查处理：
- 规格审查指出 `DATA_MODEL.md` 主融合公式漏 `d_t`：已修正为 `σ(q·θ−b−d_t)`。
- 代码质量审查指出 “无 evidence” 语义不清：已在 `DATA_MODEL.md` 明确为 BKT 无样本 evidence 时 fallback，`g2=0` 视为有效但高不确定度估计。
- 代码质量审查指出 isolated pack 分支缺覆盖：已新增 `pack_theta.g2` 信息量收缩与坏 `pack_theta.g2` fallback 测试。

验收输出：

```powershell
> cargo fmt --check
# exit 0
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.50s
```

普通沙箱首次运行 `cargo clippy --workspace --all-targets -- -D warnings` 因 Windows `target/debug/deps/*.rmeta` 写入拒绝访问失败；按流程用同一命令提升权限重跑后通过，未出现 clippy warning。

```powershell
> cargo test -p polaris-core --test p03a_mirt
running 13 tests
test shadow_fusion_falls_back_to_legacy_when_isolated_pack_mirt_variance_is_invalid ... ok
test fused_p_known_moves_from_mirt_prior_toward_bkt_with_evidence ... ok
test init_pack_initializes_theta_adagrad_accumulator ... ok
test shadow_fusion_falls_back_to_legacy_when_mirt_variance_is_invalid ... ok
test shadow_fusion_falls_back_to_legacy_without_evidence ... ok
test init_pack_initializes_q_and_theta ... ok
test shadow_mirt_variance_shrinks_with_adagrad_information ... ok
test shadow_bkt_variance_shrinks_with_more_evidence ... ok
test shadow_mirt_variance_uses_isolated_pack_adagrad_information ... ok
test degraded_provisional_submit_does_not_update_theta_or_attempt_version ... ok
test final_score_updates_theta_and_attempt_version ... ok
test final_score_accepts_legacy_free_explain_task_type ... ok
test repeated_theta_updates_use_adagrad_accumulator_to_reduce_step ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

```powershell
> cargo test --workspace
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
...
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> git diff --check
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/mirt.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/tests/p03a_mirt.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/DATA_MODEL.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
# exit 0
```

回滚方式：
- 未提交前：`git restore -- crates/polaris-core/src/mirt.rs crates/polaris-core/tests/p03a_mirt.rs docs/DATA_MODEL.md docs/tickets/QUEUE.md`，并删除 `docs/tickets/TICKET_P03O_BKT_MIRT_UNCERTAINTY_SHADOW.md`。
- 提交后：`git revert <P03O-commit>`。
