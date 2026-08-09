# P16J 教学回合对象与上下文回取（Teaching Turn + Context Recall）

状态：Queued；依赖 P01、P03H、P15B。P16D 提交前不得认领。

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
