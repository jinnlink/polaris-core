# P03H G_u 自动归纳 (G_u Auto-Induction)

状态：Completed

服务主命题环节：定位模糊 → 针对性补缺

## 背景

DATA_MODEL §9 定义了个人误解语法 G_u 的 8 类 pattern 和 Beta 后验退役机制。MASTER_PLAN F4 将 G_u 定位为原创理论贡献——跨域个人误解生成规则的自动归纳与前瞻预测。当前引擎已有 `misconceptions.toml` 静态库和 `misconception_active(c)` 检测，但缺少从重复错误模式自动发现新 G_u 规则的管线。

本票实现误解模式自动发现、验证和生命周期管理。当同一 `pattern_tag` 在 3+ 次失败 attempt 中跨概念出现时，系统自动生成 G_u 候选，经巩固验证后升级为超图中的误解节点。

科学锚点：Brown & Burton 1978 BUGGY 诊断模型、Siegler Rule Assessment、VanLehn mal-rules（见 `docs/COGNITIVE_SCIENCE_ANCHORS.md`）。

## 范围

1. 误解模式聚合：
   - 每条 graded attempt 的 `grader_json` 中提取 `pattern_tags: Vec<String>`（grader 输出的错误类型标注，复用 §9 的 8 类 pattern）。
   - 新增引擎内 `MisconceptionCandidate` 结构：`{ pattern, concept_ids, attempt_ids, first_seen, count, status }`。
   - 触发规则：同一 pattern_tag 出现在 ≥3 条不同概念的 failed attempt（score < `bkt.cut_lo`）中 → 生成候选。
   - 去重：已有相同 pattern + 概念集超集的 active 规则不重复生成。

2. 巩固验证门：
   - 候选必须过夜间巩固 holdout 门才能升级：
     - 从候选 attempt 之后的 graded attempts 中抽取留出集。
     - 检验"该 pattern 在相关概念上的再现率"是否显著高于基线（未标注该 pattern 的概念）。
     - Beta 后验 `P(precision ≥ 0.3) > 0.5` → 升级为 validated。
   - 未过门的候选保持 candidate 状态，30 天无新证据自动过期。

3. 超图接入：
   - validated G_u 规则生成新的 misconception 节点（`concepts.kind='misconception_induced'`）。
   - 自动创建 `confusion` 边连接到涉及的概念。
   - 边的 `provenance='engine'`，`evidence_ids_json` 引用触发的 attempt ids。

4. G_u 生命周期状态机：
   - `candidate` → 过巩固门 → `validated` → 首次消费 → `active` → 连续 N 次相关概念正确 → `resolved`。
   - `candidate` → 30 天无新证据 → `expired`。
   - `validated/active` → Beta 后验 `P(precision < 0.3) > 0.8` → `retired`（§9 退役规则）。
   - 状态变更写 `behavior_events`：`type='gu_lifecycle'`。

5. 消费端集成：
   - `misconception_active(c)` 扩展：除查静态 `misconceptions.toml` 外，也查 `active` 状态的 G_u 规则。
   - grader prompt 注入：有 active G_u 规则时，评分提示词附加"该学习者在 {concepts} 上反复出现 {pattern} 错误，重点核查"。
   - 前瞻预测：新概念装入时，若其超图邻域有 active G_u 关联概念，标记预测风险 `gu_risk`（调度器可提升 U(c) 优先处理）。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03h_gu_induction
```

额外人工检查：

```powershell
git diff --check
```

验收要求：
- 3 条跨概念同 pattern 失败 → 自动生成候选。
- 候选不过 holdout 门不升级（停在 candidate）。
- 过门后 `confusion` 边出现在超图中，`provenance='engine'`。
- 连续正确后 active → resolved。
- Beta 后验不达标时 → retired。
- 30 天无新证据的 candidate → expired。
- G_u 不干扰无 pattern_tag 的正常 attempt 处理。

## 禁区

- 不实现 LLM 溯因命名（归巩固票或后续镜像报告票）。
- 不让 G_u 候选（未过门）影响调度或评分。
- 不引入临床标签——G_u pattern 是行为模式标签，非个人特质诊断。
- 不修改冻结参考仓库。

## 交付记录

### 2026-06-12 开工记录

- 当前状态：P03G 已提交（`bffbfff feat(P03G): 实现交错调度`）；`docs/tickets/QUEUE.md` 中遗留的 P03G 待确认状态已在本票认领时修正为 P03H In Progress。
- 工作区检查：认领前 `git status --short` 无输出；认领后仅修改 QUEUE 与本票状态。
- 本票范围：从 graded attempt 的 `grader_json.pattern_tags` 聚合 G_u 候选；候选经 holdout/Beta 门升级为 validated 并接入超图；实现 `candidate → validated → active → resolved`、`candidate → expired`、`validated/active → retired` 生命周期；扩展 `misconception_active(c)` 和 grader prompt 注入 active G_u 风险。
- 预计修改面：`crates/polaris-core/src/db.rs`（迁移）、`config.rs`（G_u 参数登记）、新增/修改 G_u 归纳模块、`grader.rs`（pattern_tags 解析与 prompt 注入）、`engine.rs`（归纳入口与消费端）、`graph.rs/pack.rs`（`misconception_induced` kind）、P03H 专测。
- 禁区：不做 LLM 溯因命名；不让 candidate 影响调度或评分；不引入临床标签；不修改冻结参考仓库。
- 验收命令：`cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings`；`cargo test --workspace`；`cargo test -p polaris-core --test p03h_gu_induction`；`git diff --check`。

### 2026-06-12 交付记录

变更清单：

- `crates/polaris-core/src/db.rs`：新增 `gu_rules` 表，承载 G_u 候选、Beta 后验、生命周期状态与消费时间；状态变更仍写 `behavior_events(type='gu_lifecycle')`。
- `crates/polaris-core/src/config.rs`：补充 `gu.min_failures`、`gu.validate_thresh`、`gu.resolve_n` 参数登记，复用既有 `gu.retire_p`、`gu.retire_thresh`、`gu.window_days`。
- `crates/polaris-core/src/graph.rs`：允许 `concepts.kind='misconception_induced'`，供 validated G_u 规则接入超图。
- `crates/polaris-core/src/gu.rs`：新增 G_u 自动归纳模块；实现 pattern_tags 聚合、候选去重、holdout/Beta 验证、validated 写 misconception 节点与 confusion 边、active/resolved/retired/expired 状态机。
- `crates/polaris-core/src/grader.rs`：解析并存储 `pattern_tags`；grader prompt 在 active G_u 存在时追加行为模式核查提示，且不引入个人特质诊断。
- `crates/polaris-core/src/engine.rs`：新增 `run_gu_induction()`、`active_gu_rules_for_concept()`；graded attempt 成功回填后触发归纳扫描；`misconception_active(c)` 仅纳入 active G_u，candidate/validated 不影响调度。
- `crates/polaris-core/tests/p03h_gu_induction.rs`：新增 P03H 专测，覆盖候选生成、不过门停留 candidate、过门写超图、首次消费变 active、连续正确 resolved、低 precision retired、30 天 stale expired、无 pattern_tags 不干扰、scheduler 查询不激活 validated。
- `docs/tickets/QUEUE.md`：P03H 标记为已实现并通过验收。

技术选择说明：

- `gu_rules` 是本票新增的持久生命周期表；candidate 不进入 `concepts/edges`，只有 validated 才写 `misconception_induced` 节点和 `confusion` 边，避免未过门假设影响产品行为。
- 首次消费使用显式 `active_gu_rules_for_concept()` / grader prompt 路径；调度内部的 `misconception_active()` 只读 active 规则，不会把 validated 规则误激活。
- 本轮曾启动子 agent 做只读审查，但该 agent 超时未返回并已关闭；未把它计为审查通过。随后本地补充了 scheduler 不激活 validated 的边界测试。

验收输出：

```powershell
> cargo fmt --check
# exit 0, no output
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.46s
```

```powershell
> cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core --test p03h_gu_induction
running 8 tests
test attempts_without_pattern_tags_do_not_create_gu_candidates ... ok
test three_cross_concept_failed_pattern_tags_generate_candidate ... ok
test candidate_without_holdout_gate_remains_candidate ... ok
test stale_candidate_expires_after_window_without_new_evidence ... ok
test candidate_passing_holdout_gate_creates_misconception_node_and_confusion_edges ... ok
test low_precision_active_rule_is_retired ... ok
test first_consumption_marks_validated_rule_active_and_correct_streak_resolves_it ... ok
test scheduler_query_does_not_activate_validated_rule ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

```powershell
> git diff --check
warning: in the working copy of 'crates/polaris-core/src/config.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/db.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/engine.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/grader.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/graph.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/lib.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/TICKET_P03H_GU_AUTO_INDUCTION.md', LF will be replaced by CRLF the next time Git touches it
# exit 0
```

备注：`cargo clippy --workspace --all-targets -- -D warnings` 在普通沙箱内仍会因默认 `target/debug/deps/*.rmeta` 写入被 Windows 拒绝而失败；按同一票面命令在提升权限下通过，未发现 clippy 代码告警。

回滚方式：

```powershell
git restore crates/polaris-core/src/config.rs crates/polaris-core/src/db.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/grader.rs crates/polaris-core/src/graph.rs crates/polaris-core/src/lib.rs docs/tickets/QUEUE.md docs/tickets/TICKET_P03H_GU_AUTO_INDUCTION.md
Remove-Item crates/polaris-core/src/gu.rs crates/polaris-core/tests/p03h_gu_induction.rs
```
