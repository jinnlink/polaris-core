# P16H 会话收口（Session Close-out）

状态：已通过验收，待提交；依赖 P01、P03E。P16G 已提交（`b558fae`）。

服务主命题：定位模糊 → 针对性补缺。

## 背景

`sessions` 表当前只有 `id`、`started_at`、`context_json`：没有结束时间，没有小结，没有关闭动作。唯一的阶段性总结是镜像报告，而它是**周**级（`report.window_days` 默认 7）且必须手动触发。

真实使用形态是「和 AI 聊一整天」。一天结束时该有的东西——今天碰了哪些概念、卡在哪、明天从哪接——在系统里不存在。

第二个后果更隐蔽：**没有收口就没有强制取舍。** 老师课末只讲三件事，因为他必须选。系统不逼收口，外部 AI 就倾向于把所有能讲的都讲一遍，表现为「抓不到重点」。这不是模型判断失误，是缺少做减法的机制。

session 级结构本就在蓝图内（MASTER_PLAN 家族八「双 session 型」「session 内序位效应」、家族四 SRL 三段循环），只是从未落地。

## 范围

1. schema：`sessions` 增加 `ended_at`、`closed_at`；新增 `session_summaries`（`session_id`、`concepts_touched_json`、`attempts_count`、`top_stuck_concept_id`、`next_entry_concept_id`、`assertions_json`、`generated_at`）。
2. Core：`close_session(session_id) -> SessionCloseSummary`。**纯 Tier 0 确定性计算，不调用 LLM。**
3. 小结内容只允许来自本 session 的 `attempts` 与 `behavior_events`：碰过的概念、每概念最高/最低分、卡住点（最低分或 hint/abandon 最多者）、下次入口概念。
4. 下次入口概念复用既有 `next_task` 结果，**不新造调度逻辑**。
5. **强制取舍**：断言最多 3 条。候选超过 3 个时按既有效用排序取前 3。这是本票的产品要点，不是可调项。
6. 每条断言带 `evidence_ids`，说不出证据的不许进小结（沿用 P03I 红线）。
7. CLI：`polaris session close --session <id> [--json]`、`polaris session show --session <id> [--json]`。
8. HTTP/MCP 只读出口 + P11B 合同测试。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p16h_session_closeout
cargo test --workspace
```

专项要求：

- 空 session、单 attempt session、多概念 session 三种夹具。
- 候选超过 3 个时**必须**截断到 3，且有专门用例断言此行为。
- 每条断言的 `evidence_ids` 非空。
- 重复 `close_session` 幂等，不产生第二行小结。
- schema 迁移测试：迁移中断后台账与 `user_version` 保持原状（沿用 P11A/P16D 做法）。

## 禁区

- 不调用 LLM。叙事润色是 P06D 的范围，本票不碰。
- 不改 mastery、调度、相判据、评分公式。
- 不自动关闭 session。必须显式调用；系统不猜「一天结束了」。
- 不提供绕过 3 条上限的参数。
- 不与镜像报告合并，周报仍是周报。
- 不修改冻结仓库。

## 开工前复述（2026-08-09）

- 范围：schema v4、确定性 close/show、最多 3 条 evidence-bound 断言、复用 next_task 的下一入口、CLI 与 HTTP/MCP 只读出口。
- 禁区：不调用 LLM，不改 mastery/调度/相图/评分，不自动关闭，不提供 3 条上限参数，不合并周报。
- 验收命令：票内专项、`cargo fmt --check`、workspace Clippy `-D warnings`、`cargo test --workspace`、`git diff --check`。
- 预计修改面：`db.rs`、独立 session 模块、Engine facade、CLI/HTTP/MCP、API/DATA_MODEL、专项与合同测试、QUEUE 与本票。

## 回滚

删除 `session_summaries` 表与 `sessions` 新增列（schema 版本回退按 P11A 策略：旧二进制拒绝 + 备份恢复）；移除 CLI/HTTP/MCP 出口与测试。

## AI 交付记录（2026-08-09）

- schema 升至 v4：`sessions` 增加 `closed_at`，新增 `session_summaries`、查询索引与原子迁移；故障注入证明 v4 失败时保持 v3 台账、`user_version` 和列形状。
- Core 新增确定性 `close_session` / `session_close_summary`：只折叠当前 session 的 attempts 与 behavior events，统计概念、分数区间、hint/abandon、卡点；下一入口复用 `next_task`。
- 强制最多 3 条断言，按既有效用排序，且每条断言携带非空 evidence ids；重复关闭返回同一小结，不生成第二行。
- CLI 新增 `session close/show`；HTTP 新增只读 `GET /session?session=...`；MCP 新增只读 `get_session_summary`。HTTP/MCP 均不能关闭 session。
- API 合约与数据模型已更新；schema v4 引起的 doctor/P16D 固定版本断言同步升级。

### 最终验收输出

```text
> cargo test -p polaris-core --test p16h_session_closeout
running 3 tests
test schema_v4_adds_closeout_fields_table_and_atomic_migration ... ok
test multi_concept_closeout_is_session_bound_evidence_bound_and_idempotent ... ok
test empty_and_single_attempt_sessions_close_with_stable_shapes ... ok
test result: ok. 3 passed; 0 failed

> cargo fmt --check
exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> cargo test --workspace
polaris-cli: 110 passed; polaris-core: 81 passed
p16d_global_profile_governance: 12 passed
p16h_session_closeout: 3 passed
all discovered suites: exit 0

> git diff --check
exit 0
```

### 真实 CLI 冒烟

```text
polaris --db <temp> init --pack packs/rust
initialized
polaris --db <temp> next --session cli-smoke
concept: ownership
polaris --db <temp> session close --session cli-smoke --json
"session_id": "cli-smoke", "attempts_count": 0, "next_entry_concept_id": "ownership"
polaris --db <temp> session show --session cli-smoke
会话 cli-smoke 已收口：0 次作答，触及 1 个概念。
下次从这里接：ownership
```

- 回滚：执行 `git revert <P16H-commit-sha>` 移除代码与出口；已升级的真实数据库不做破坏性降级，按 P11A 策略使用升级前备份恢复，旧二进制会拒绝写入 v4 数据库。
