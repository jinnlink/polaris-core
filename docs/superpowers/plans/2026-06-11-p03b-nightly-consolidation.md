# P03B 夜间巩固 v1 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现 P03B 夜间巩固的可审计慢通路：残差统计、θ 快照/收缩、候选簇检测、验证门拒绝记录。

**架构：** 新增 `polaris-core::consolidation` 模块承载离线逻辑；`Engine` 只暴露 `run_nightly_consolidation`。本票不把候选写回 q，不新增维度，只写审计记录。

**技术栈：** Rust 2021、rusqlite、serde_json、现有 MIRT 向量与参数。

---

## 文件结构

- 创建 `crates/polaris-core/src/consolidation.rs`：残差统计、theta 快照、候选检测、consolidation_runs 审计。
- 修改 `crates/polaris-core/src/lib.rs`：导出模块。
- 修改 `crates/polaris-core/src/engine.rs`：新增 `run_nightly_consolidation`。
- 创建 `crates/polaris-core/tests/p03b_consolidation.rs`：P03B 集成测试。
- 修改 `docs/tickets/QUEUE.md`、`docs/tickets/TICKET_P03B_NIGHTLY_CONSOLIDATION.md`：状态和交付记录。

## 任务 1：红灯测试

- [ ] 写 `p03b_consolidation.rs`，覆盖：
  - residual_stats 按周写入 mean residual。
  - nightly run 写 theta_history 并收缩/increment theta version。
  - 相关残差簇写入 consolidation_runs 且 status=rejected，不改 q 维数。
- [ ] 运行：`cargo test -p polaris-core --test p03b_consolidation`，预期缺模块失败。

## 任务 2：实现 residual_stats

- [ ] 从 final_score attempts 读取 concept、task_type、theta_version、created_at。
- [ ] 按 attempt 时的 theta 版本计算 p_hat。
- [ ] 分组写入 `residual_stats`。
- [ ] 跑 P03B 定向测试。

## 任务 3：实现 nightly audit run

- [ ] 插入当前 theta snapshot 到 `theta_history`。
- [ ] 应用 `mirt.shrink` 并递增 theta.version。
- [ ] 基于 residual_stats 计算 Pearson 相关与连通簇。
- [ ] 写 `consolidation_runs(status='rejected', holdout_delta=0.0)`。
- [ ] 跑 P03B 定向测试。

## 任务 4：验收与审查

- [ ] 运行全量验收命令。
- [ ] 派子 agent 审查：不得越界到 P03C/P04；候选不过门不得生效；残差/θ 版本语义正确。
- [ ] 修复反馈，填写交付记录，提交。
