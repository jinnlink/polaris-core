# P16J 教学回合对象与上下文回取（Teaching Turn + Context Recall）

状态：已提交（`c4d0c51`）；依赖 P01、P03H、P15B。P16D 已提交（`7a478cc`）。

服务主命题：针对性补缺 → 验证真懂。

## 背景

引擎交给外部导师的教学载荷里没有任何历史。

- `NextTask`（`crates/polaris-core/src/engine.rs:105`）只有 `concept_id`、`move_id`、`task_type`、`prompt_text`、`reason`、`mrt_*`。
- `TeachingInstruction`（`crates/polaris-core/src/teaching.rs:11`）的 `anchor` 是 `render_move_prompt(&template, &name)`，即 pack 模板把 `{concept}` 替换成概念名。

**没有一次教学交付携带该概念上以前发生过什么。** 不带历史错句、不带上次讲过什么、不带上次卡在哪。两个后果：

1. 外部 AI 每次从零开始讲，同样的内容反复讲，学习者的时间被重复消耗。
2. 讲解本身不是对象，因此 attempt 低分时引擎**只有一条归因通道**：学习者掌握度低。它在数据结构上无法表达「这次讲解讲砸了」。结果是自我强化的坏循环——无效讲解 → 记成学习者失败 → `p_known` 下降 → 调度加重该概念 → 很可能用同样方式再讲一遍。

`moves_effects` / `mrt_log` / `bred_moves` 的粒度是 move 类型 × `context_hash`，不是单次讲解。P15B 已把「取题 → 作答 → 提交」关联为可审计回合，钩子已在。

## 范围

### A. 上下文回取（只读，零 schema 变更）

1. `TeachingInstruction` 与 `NextTask` 增加 `context` 字段，内容取自现有 `attempts` / `evidence_items` / `gu_rules`：
   - 该概念最近 N 条 attempt 摘要：时间、`task_type`、`final_score`、`self_confidence`、`misconception_id`
   - 最近一次失败的学习者作答原文摘要
   - 该概念上的活跃 G_u 规则
   - 上一次交付的 `anchor`
2. N 为 A 类参数，默认 3。
3. 严格只读：不改选题、不改评分、不改 mastery。

### B. 教学回合对象（最小 schema）

4. 新增 `teaching_turns(id, session_id, concept_id, attempt_id NULL, instruction_json, explanation_evidence_id NULL, created_at)`。
5. 新增入口：外部导师可把「我刚才是这样讲的」登记为 evidence 并关联到本回合。
6. **只建立关联，不做任何自动归因。** attempt 低分且本回合有讲解时，系统只记录事实，不判定讲解是否有效。有效性判定需要留出验证门，留后续票。

## 关键禁区（务必保留）

**讲解 evidence 不得进入 strict-citation 的可引用集合。** 当前 `grader.rs:186` 的 `evidence_for_attempt` 只取 `attempts.response_evidence_id`，即只允许引用学习者本人的作答。本票绝不能放宽这一点——否则 LLM 会引用自己的讲解给学生打分，评分闭环立即失效。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-core --test p16j_teaching_turn_context
cargo test --workspace
```

专项要求：

- 无历史时 `context` 为空且不报错。
- 有 N+2 条历史时只返回最近 N 条，顺序确定。
- 活跃 G_u 规则确实出现在 `context` 中。
- **反向断言**：登记讲解 evidence 后，`evidence_for_attempt` 返回集合不变，grader 无法引用讲解原文。此用例必须存在。
- 选题结果在加入 `context` 前后完全一致（回归断言）。
- P11B 合同测试更新，HTTP/MCP 新字段纳入稳定面。

## 禁区

- 不自动判定讲解有效性。
- 不改选题、评分、mastery、`U(c)`。
- 不放宽 strict-citation 的证据集合。
- 不存储讲解全文以外的推断产物。
- 不修改冻结仓库。

## 回滚

移除 `context` 字段与 `teaching_turns` 表；恢复 P11B 合同基线；删除测试。A 部分与 B 部分可分别回滚。

## 开工记录（2026-08-09）

- 范围：只读回取最近 3 条概念历史、最近失败原文摘要、活跃 G_u 与上次 anchor；新增最小 `teaching_turns` 关联对象和讲解 evidence 登记入口。
- 禁区：不改选题、评分、mastery、`U(c)`，不自动归因讲解有效性，讲解 evidence 永不进入 strict-citation 可引用集合。
- 验收：实跑票面 fmt、workspace clippy、P16J 专项与 workspace 全测，并更新 P11B HTTP/MCP 合同稳定面。
- 预计修改面：schema v6/迁移、Core 教学上下文与教学回合 API、CLI/HTTP/MCP 契约、DATA_MODEL/API 文档及测试。

## 交付记录（2026-08-09）

### 变更清单

- schema v6 新增 `teaching_turns`、两条查询索引与原子迁移；新增 A 类参数 `teaching.context_attempt_limit=3`。
- `TeachingInstruction` / `NextTask` 增加同一只读 `context`：最近 N 条 attempt、最近失败作答摘要、活跃 G_u、上次实际交付 anchor；无历史序列化为 `null`。
- 新增教学回合与讲解登记：HTTP `/next`、MCP `get_next_task` 返回 `teaching_turn_id`；HTTP `/teaching-turn/explanation`、MCP `record_teaching_explanation` 只记录导师讲解事实。
- 严格任务回执通过 `next` 事件精确关联对应教学回合与后续 attempt；多个未提交回合不会错连。
- grader 可引用集合仍只查询 `attempts.response_evidence_id`；讲解 evidence 引用会降级并进入重试，不参与评分闭环。
- 更新 P11B HTTP/MCP 稳定合同、数据模型、参数文档、迁移计数回归及 P16J 专项测试。

### 验收实跑

```text
> cargo fmt --check
exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> cargo test -p polaris-core --test p16j_teaching_turn_context
running 6 tests
test result: ok. 6 passed; 0 failed

> cargo test -p polaris-cli p16j_
running 2 tests
test result: ok. 2 passed; 0 failed

> cargo test --workspace --quiet
polaris-cli: 115 passed; polaris-core: 81 passed
p16j_teaching_turn_context: 6 passed
all discovered suites: exit 0

> git diff --check
exit 0（仅工作区既有 CRLF 转换警告）
```

### 回滚

- 执行 `git revert c4d0c51` 移除上下文、教学回合与 API；已升级真实库不做破坏性降级，使用升级前备份恢复，旧二进制会按 P11A 拒绝写入 schema v6。
