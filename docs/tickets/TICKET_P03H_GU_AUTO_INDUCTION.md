# P03H G_u 自动归纳 (G_u Auto-Induction)

状态：Queued

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

待填写。
