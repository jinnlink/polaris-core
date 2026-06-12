# P03I 镜像报告 v1 (Mirror Report)

状态：Completed

服务主命题环节：验证真懂 → 定位模糊（把引擎对学习者的模型透明化为可质疑、可证伪的断言）

## 背景

MASTER_PLAN 心智动力学引擎定义了镜像报告（周）：**只说可验证的话**，每条断言带证据 id 与置信度，"说不出证据的话不许进报告"（Phase 3 验证红线）。报告同时是开放理论层的呈现通道——夜间巩固的候选发现经镜像报告呈现（显式标注未过验证门）；DATA_MODEL §12.6 授权报告基于证据建议手动参数，但只建议、不执行。模型对用户透明可质疑：用户可标"不准"，该反馈本身成为校正数据。

科学锚点：延迟 JOL 校准（Nelson & Dunlosky 1991）、exam wrapper 考后反思（Lovett）、策略元教学（系统解释自己——透明即教学）。

本票为纯确定性实现（零 LLM）：断言由引擎从事件溯源数据直接挖掘，置信度为 Beta-Binomial 后验概率。LLM 叙事润色归后续票。

## 范围

1. 新增 `crates/polaris-core/src/report.rs`：镜像报告生成器。
   - 报告三段制：
     - `assertions`（已验证断言）：每条 `{id, kind, subject, claim, confidence, evidence_ids, stats}`。
     - `hypotheses`（未过门假设）：巩固候选维度等，显式标注未过验证门、不作为行为依据（SPEC 验证门铁律）。
     - `suggestions`（参数建议）：DATA_MODEL §12.6，只建议、不执行。
   - **红线过滤器**：任何条目 `evidence_ids` 为空 → 丢弃；assertion `confidence < report.confidence_floor` 或样本 < `report.min_evidence` → 丢弃。被丢弃候选计入 `skipped`（kind + reason），保证可审计。
   - 断言 id 确定性：`kind:subject`（稳定，可跨报告引用与抑制）。

2. 断言类型 v1（全部确定性挖掘）：
   - `calibration_phantom`：幻影掌握风险——calib_gap/p_known/attempt_count 满足幻影判据的概念；证据 = 该概念 graded attempts；置信度 = P(高估率 > 0.5 | Beta 后验)。
   - `hint_abandon_conditional`：连续 ≥2 次 hint 后 10 分钟内放弃的条件概率 vs 无该前件的基线放弃率；证据 = hint/abandon 事件 id；置信度 = P(p_cond > p_base)。
   - `abandon_time_contrast`：时段（6 小时桶）放弃频率对比，取合格桶中最高 vs 最低；证据 = 高桶 abandon 事件 id；置信度 = P(p_hi > p_lo)。
   - `gu_pattern`：active/validated G_u 规则呈现；证据 = 规则 attempt_ids；置信度 = P(precision ≥ gu.retire_p | Beta 后验)。
   - `hazard_prediction`：**仅当 hazard 模型 validation_auc ≥ hazard.auc_gate 才允许**（DATA_MODEL §7）。v1 模型未拟合 → 必须整类排除，报告记 `hazard_gate.participates=false` 与原因。

3. 假设呈现：最近一次 `consolidation_runs` 的候选潜在维度簇 → `hypotheses`，证据 = `consolidation:<run_id>`，标注 holdout 状态（未过门）。

4. 参数建议 v1 一条规则：provisional 启发式系统性偏差——90 天内 graded attempts 的 mean(provisional − final) 绝对值 ≥ `report.suggest_bias_thresh` 且 n ≥ `report.suggest_bias_n` → 建议复核 `grade.provisional_base/slope`；证据 = 相关 attempt ids；置信度 = P(偏差方向一致率 > 0.5)。**不写 meta，不执行任何参数变更。**

5. 置信度数学（`report.rs` 内，确定性）：
   - ln-gamma（Lanczos）→ 正则不完全 Beta 函数 I_x(a,b)（连分式）→ Beta CDF。
   - 两 Beta 后验方向概率 P(X > Y)（整数参数闭式求和，均匀先验 Beta(1,1)）。
   - 单测：已知值对拍（I_0.5(2,2)=0.5、对称情形 P=0.5、强分离趋近 1）。

6. 持久化与反馈闭环：
   - 新表 `mirror_reports(id, week, generated_at, report_json, assertion_count, skipped_count)`；week 用 ISO 周标签（复用巩固模块实现）。
   - 生成时写 `behavior_events(type='mirror_report')`。
   - `record_report_feedback(report_id, assertion_id, verdict='inaccurate')` → 写 `behavior_events(type='report_feedback')`；此后 `report.feedback_suppress_days` 内同 id 断言被抑制并计入 skipped（"不准"反馈成为校正数据的 v1 落地）。
   - 报告附 exam-wrapper 三问反思（静态提示词，非断言）。

7. 引擎与 CLI 接入：
   - `Engine::run_mirror_report()` / `latest_mirror_report()` / `record_report_feedback()`。
   - CLI：`polaris report`（生成 + 打印）、`polaris report-feedback --assertion <id>`（标记不准）。

8. 参数登记（config registry + DATA_MODEL 三类制）：
   - `report.window_days = 7`（B，[3,30]，手动）——挖掘窗口。
   - `report.min_evidence = 3`（**A**，验证门槛，不调）。
   - `report.confidence_floor = 0.6`（**A**，验证门槛，不调）。
   - `report.feedback_suppress_days = 90`（B，[30,365]，手动）。
   - `report.suggest_bias_thresh = 0.15`（B，[0.05,0.30]，手动）。
   - `report.suggest_bias_n = 10`（B，[5,50]，手动）。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03i_mirror_report
```

额外人工检查：

```powershell
git diff --check
```

验收要求：

- 红线：空数据 → 报告三段皆空，无任何无证据条目；每条断言的 evidence_ids 都能解析回真实行。
- 幻影校准断言：种子数据满足判据 → 断言出现，带 attempt 证据与置信度；样本不足 → 不出现且 skipped 留痕。
- hint→放弃条件断言与时段对比断言：方向、n、置信度正确；不足 min_evidence 不出现。
- hazard 整类排除：v1 无 hazard_prediction 断言，hazard_gate.participates=false 且有原因。
- 巩固候选以假设段呈现，带 consolidation run 证据 id 与未过门标注。
- G_u active 规则呈现，证据为其 attempt_ids。
- 参数建议触发时 meta 不变（只建议不执行）。
- 反馈闭环：标记"不准"后，下次报告同 id 断言被抑制且计入 skipped。
- 决定性：同一数据库两次生成，稳定字段（kind/subject/claim/confidence/evidence_ids）完全一致。
- G_u/调度/评分行为不受报告生成影响（报告是只读挖掘 + 自身表写入）。

## 禁区

- 不做 LLM 叙事/溯因/润色（零 LLM；Tier 1 润色归后续票）。
- 报告不得修改任何参数、调度状态或掌握度（suggest-only；只写 mirror_reports 与 behavior_events）。
- 不引入临床/人格标签；不做社会比较（只和过去的自己比）。
- hazard 模型未过 AUC 门不得产出预测类断言。
- 不修改冻结参考仓库。

## 交付记录

### 2026-06-12 开工记录

- 当前状态：P03H 已提交（`f8d05cb feat(P03H): 实现 G_u 自动归纳`）；本票按 QUEUE 顺序认领为唯一 In Progress。
- 工作区检查：认领前 `git status --short` 仅余未跟踪的 `.cursor/`（用户本地配置，不属于任何票，不动）。
- 本票范围：如上"范围"1-8；预计修改面：`db.rs`（mirror_reports 迁移）、`config.rs`（report.* 参数登记）、新增 `report.rs`、`engine.rs`（三个入口方法）、`lib.rs`（模块导出）、`consolidation.rs`（iso_week_label 改 pub(crate) 复用）、`polaris-cli/src/main.rs`（report 命令）、新增 `tests/p03i_mirror_report.rs`。
- 禁区：见上；尤其零 LLM、suggest-only、hazard AUC 门。
- 验收命令：见上。

### 2026-06-12 交付记录

变更清单：

- `crates/polaris-core/src/report.rs`（新增）：镜像报告生成器。断言挖掘 4 类（calibration_phantom / hint_abandon_conditional / abandon_time_contrast / gu_pattern）+ 巩固候选假设段 + provisional 偏差参数建议段；红线过滤器（无证据/样本不足/低置信/用户标不准 → 丢弃且 skipped 留痕）；Beta-Binomial 置信度数学（Lanczos ln-gamma、连分式正则不完全 Beta、两 Beta 后验方向概率闭式求和），全部确定性、零外部依赖、带已知值单测。
- `crates/polaris-core/src/db.rs`：新增 `mirror_reports` 表（id, week, generated_at, report_json, assertion_count, skipped_count）。
- `crates/polaris-core/src/config.rs`：登记 `report.window_days`(B)、`report.min_evidence`(A)、`report.confidence_floor`(A)、`report.feedback_suppress_days`(B)、`report.suggest_bias_thresh`(B)、`report.suggest_bias_n`(B)，附注册表单测。
- `crates/polaris-core/src/consolidation.rs`：`iso_week_label` 改 `pub(crate)` 供报告复用（无行为变更）。
- `crates/polaris-core/src/engine.rs`：新增 `run_mirror_report()` / `latest_mirror_report()` / `record_report_feedback()` 三个入口。
- `crates/polaris-core/src/lib.rs`：导出 `report` 模块。
- `crates/polaris-cli/src/main.rs`：新增 `polaris report`（生成 + 分段打印 + 三问反思）与 `polaris report-feedback --assertion <id> [--report <id>]`。
- `crates/polaris-core/tests/p03i_mirror_report.rs`（新增）：14 个集成测试，覆盖空库红线、幻影断言证据可解析、样本不足 skipped、hint→放弃条件断言、单事件不足、时段对比方向与置信度、hazard 整类排除、巩固假设未过门标注、G_u 规则呈现、参数建议不改 meta、"不准"反馈抑制、未知断言反馈拒绝、稳定字段决定性、latest 读取。
- `docs/tickets/QUEUE.md`：P03I 标记完成。

技术选择说明：

- 报告三段制（assertions / hypotheses / suggestions）：红线"说不出证据不许进报告"对三段都强制 evidence_ids 非空；置信度下限与最小样本门只对断言与建议生效，假设段显式标注"未过留出验证门，仅为假设"（对应 SPEC 验证门铁律与 MASTER_PLAN 开放理论层"巩固发现经镜像报告呈现"）。
- 断言 id 为确定性 `kind:subject`，跨报告稳定，使"不准"反馈可以按 id 精确抑制后续报告中的同一断言（反馈成为校正数据的 v1 落地）。
- 时段对比按存储时间（UTC）分桶并在文案中显式标注 UTC，与 hazard 特征的时段口径一致；本地化呈现归 UI 阶段。
- hazard 门按 DATA_MODEL §7 实现为整类排除：v1 hazard 模型未拟合（validation_auc=NULL），`hazard_gate.participates=false` 且 reason 可审计；任何 hazard_prediction 断言不会出现。
- 证据列表按时间倒序截断（断言上限 20 条、校准类 12 条），完整计数保留在 stats；防止 report_json 无界膨胀。
- 报告生成是只读挖掘 + 自身表写入（mirror_reports + behavior_events 两类事件），不触碰掌握度、调度、参数——测试中验证 meta 不变。

验收输出：

```powershell
> cargo fmt --check
# exit 0, no output
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.59s
```

```powershell
> cargo test --workspace
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```powershell
> cargo test -p polaris-core --test p03i_mirror_report
running 14 tests
test feedback_for_unknown_assertion_is_rejected ... ok
test hazard_prediction_assertions_are_gated_out_while_model_unfit ... ok
test calibration_phantom_assertion_carries_attempt_evidence_and_confidence ... ok
test hint_streak_followed_by_abandons_yields_conditional_assertion ... ok
test empty_database_produces_report_with_no_unevidenced_items ... ok
test active_gu_rule_appears_with_attempt_evidence ... ok
test consolidation_proposals_surface_as_gated_hypotheses ... ok
test single_hint_episode_is_skipped_for_insufficient_evidence ... ok
test inaccurate_feedback_suppresses_assertion_in_next_report ... ok
test abandon_time_contrast_reports_direction_with_confidence ... ok
test calibration_phantom_below_min_evidence_is_skipped_with_audit_trail ... ok
test latest_mirror_report_returns_most_recent ... ok
test param_suggestion_fires_without_mutating_meta ... ok
test report_generation_is_deterministic_on_stable_fields ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

```powershell
> git diff --check
# exit 0（仅 CRLF 行尾告警，无空白错误）
```

CLI 冒烟（空库红线实跑）：

```powershell
> polaris init --pack packs/rust
initialized
> polaris report
镜像报告 77701adf-47c3-4cb9-86c3-164053d53475 （周 2026-W24）
窗口=7天 断言=0 假设=0 建议=0 被过滤=0
hazard 门：participates=false reason=no_mental_state_data
--- 三问反思 ---
· 本周哪个概念的实际表现最出乎你的意料？为什么？
· 上面哪条断言和你的自我感觉不符？标记「不准」——这本身就是校正数据。
· 下周你优先补哪个缺口？打算用什么方式验证自己真的补上了？
```

回滚方式：

```powershell
git restore crates/polaris-core/src/config.rs crates/polaris-core/src/consolidation.rs crates/polaris-core/src/db.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/lib.rs crates/polaris-cli/src/main.rs docs/tickets/QUEUE.md
Remove-Item crates/polaris-core/src/report.rs, crates/polaris-core/tests/p03i_mirror_report.rs, docs/tickets/TICKET_P03I_MIRROR_REPORT.md
```
