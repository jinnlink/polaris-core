# 数据模型与公式 v1.1（实现依据，经审计加固）

> 实现细节（公式/DDL/参数/算法配方）冲突时以本文为准；设计意图冲突以 MASTER_PLAN 为准；铁律以 SPEC 为准。
> v1.1 = 针对"逐字执行"标准的加固版：所有常数有默认值、所有边界有定义、所有算法可逐步执行。

## 0. 核心架构语义：事件溯源（先读这个）

- **事实源** = `attempts`（含 final 回填）+ `behavior_events` + pack 种子。**`mastery_states` 是确定性折叠（fold）出的物化视图。**
- fold 顺序：`created_at` 升序，同刻按 `id` 字典序。每条 attempt 用 `final_score`（若有）否则 `provisional_score`。
- provisional 落账时增量 fold 一次；**final 到达时对该概念全量重放**（重新 fold 该概念全部 attempts）。重放预算：≤1ms/百条。
- 推论：任何掌握度数字都可审计、可复现；崩溃恢复 = 重放；属性测试 = 增量结果必须等于全量重放结果。

## 1. 约定

- id：UUIDv4 文本；时间：ISO-8601 UTC。
- 派生知识行带 `provenance`（'pack-seed'|'llm'|'consolidation'|'user'|'engine'）+ `evidence_ids_json`。
- 向量 = f32 小端 BLOB；计算一律 f64，存储才降 f32。K 默认 32、上限 64。
- **数值规范**：logit 一律 clamp 到 ±10；σ(x)=1/(1+e^(−x)) 防溢出实现；EWMA/概率存 REAL。
- attempts 不可变：修正写 `final_*`，不覆盖 provisional。
- **一切常数从 meta 读**（见 §9 参数登记处），代码不写死；读取经单一 config 模块。

### 1.1 Schema 版本与迁移账本

- SQLite schema 版本权威源 = `PRAGMA user_version`，当前由 `db::CURRENT_SCHEMA_VERSION` 定义。
- `schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL)` 是迁移账本；当前一次性完整 schema 作为 baseline 迁移记录。
- `migrate()` 对空库和旧的未版本化库都必须幂等：先补齐当前 schema，再写 baseline 账本并设置 `user_version`。
- 旧库已有业务行和用户手动 `meta` 参数不得被迁移覆盖；默认参数继续使用 `INSERT OR IGNORE`。
- 若数据库 `user_version` 高于当前二进制支持版本，写路径必须拒绝打开并提示版本不支持，避免旧程序误写新库。
- 只读入口（doctor / diagnose / trust / learner mirror）不得为了检查版本而创建库或执行迁移。

## 2. 表

### P01 激活

```sql
meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)

schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL)

op_log(op_id TEXT PRIMARY KEY, ts TEXT, entity TEXT, entity_id TEXT, type TEXT,
       payload_json TEXT, evidence_refs_json TEXT, actor TEXT, lamport INTEGER)

evidence_items(id TEXT PRIMARY KEY, session_id TEXT, source TEXT, content_type TEXT,
       text TEXT NOT NULL, lang TEXT, concept_ids_json TEXT, created_at TEXT)

attempts(id TEXT PRIMARY KEY, session_id TEXT, concept_id TEXT NOT NULL, task_type TEXT,
       prompt_text TEXT, response_evidence_id TEXT,
       self_confidence INTEGER,            -- 1-5，必须在看到任何反馈前采集
       latency_ms INTEGER, hint_count INTEGER DEFAULT 0,
       provisional_score REAL,
       final_score REAL, depth TEXT,       -- recall|explain|apply|transfer
       misconception_id TEXT, grader_json TEXT, rating TEXT,
       theta_version INTEGER,              -- P03 填；P01 留 NULL
       theta_scope TEXT DEFAULT 'shared',  -- P08A: shared | pack:<pack_id>；旧 NULL 视为 shared
       created_at TEXT, graded_at TEXT)

concepts(id TEXT PRIMARY KEY, pack TEXT, name TEXT, kind TEXT DEFAULT 'concept',  -- 'concept'|'schema'
       seed_order INTEGER,                 -- pack 内出现序（决定性排序用）
       p_init REAL,                        -- 可选；缺省用 meta('bkt.p_init')
       b_difficulty REAL DEFAULT 0, q BLOB, embedding BLOB,   -- P01 留 NULL
       provenance TEXT, evidence_ids_json TEXT, created_at TEXT)

edges(id TEXT PRIMARY KEY, src TEXT, dst TEXT,
       type TEXT,    -- prerequisite|confusion|component_of|instantiates|maps_to（P01 只消费 prerequisite）
       weight REAL DEFAULT 1.0, alignment_json TEXT,
       provenance TEXT, evidence_ids_json TEXT, created_at TEXT)

mastery_states(concept_id TEXT PRIMARY KEY, p_known REAL, fsrs_json TEXT, next_due_at TEXT,
       last_review_at TEXT, calib_gap REAL DEFAULT 0, brier_ewma REAL DEFAULT 0,
       last_depth TEXT, max_depth TEXT, attempt_count INTEGER DEFAULT 0,
       lapses INTEGER DEFAULT 0, updated_at TEXT)

sessions(id TEXT PRIMARY KEY, started_at TEXT, ended_at TEXT, context_json TEXT)

behavior_events(id TEXT PRIMARY KEY, session_id TEXT, at TEXT,
       type TEXT,    -- latency|hint|abandon|resume|edit
       concept_id TEXT, payload_json TEXT)

grade_queue(attempt_id TEXT PRIMARY KEY, enqueued_at TEXT, retry_count INTEGER DEFAULT 0, last_error TEXT)
```

P08A pack 状态放在 `meta`：`active_pack` 表示当前 pack；`pack.<id>.title` 保存显示名；`pack.<id>.theta_mode` 取 `shared|isolated`，默认 `shared`。

### 后续激活（建表即可，逻辑后置）

- 图式 = `concepts.kind='schema'`（P02）。
- `theta(id INTEGER PRIMARY KEY CHECK(id=1), vec BLOB, g2 BLOB, version INTEGER, updated_at TEXT)`、`theta_history(version INTEGER PRIMARY KEY, vec BLOB, at TEXT)`（P03/P06G）。
- `pack_theta(pack TEXT PRIMARY KEY, vec BLOB, g2 BLOB, version INTEGER, updated_at TEXT)`、`pack_theta_history(pack TEXT, version INTEGER, vec BLOB, at TEXT, PRIMARY KEY(pack, version))`（P08A；仅 `theta_mode=isolated` 的 pack 使用，shared pack 继续使用全局 `theta`）。
- `residual_stats(concept_id TEXT, week TEXT, mean_resid REAL, n INTEGER, PRIMARY KEY(concept_id, week))`（P03）。
- `consolidation_runs(id TEXT PRIMARY KEY, ran_at TEXT, proposals_json TEXT, holdout_delta REAL, status TEXT)`（accepted|rolled_back）。
- `moves_effects(move TEXT, context_hash TEXT, alpha REAL, beta REAL, n INTEGER, PRIMARY KEY(move, context_hash))`（P04）。
- `mrt_log(id TEXT PRIMARY KEY, at TEXT, context_json TEXT, randomized INTEGER, move TEXT, prereg_id TEXT)`（P04）。
- `bred_moves(id TEXT PRIMARY KEY, candidate_move TEXT, incumbent_move TEXT, context_hash TEXT, task_type TEXT, template TEXT, mechanisms_json TEXT, main_effect_hypothesis TEXT, prereg_json TEXT, status TEXT, posterior_win_prob REAL, candidate_alpha REAL, candidate_beta REAL, incumbent_alpha REAL, incumbent_beta REAL, n_candidate INTEGER, n_incumbent INTEGER, created_at TEXT, updated_at TEXT, admitted_at TEXT, retired_at TEXT)`（P05B，status=preregistered|admitted|retired）。
- `param_tuning_runs(id TEXT PRIMARY KEY, ran_at TEXT, param TEXT, old_value TEXT, new_value TEXT, metric TEXT, delta REAL, status TEXT)`（P03H，accepted|rejected）。

## 3. P01 公式（全部钉死）

**初始化**：首条 attempt 触发创建 mastery_states：`p_known = concepts.p_init ?? meta('bkt.p_init', 0.20)`；FSRS 走 reps==0 路径。

**FSRS**：从 `Polaris/apps/web/src/lib/fsrs.ts` 1:1 移植。
`elapsed_days = max(0, (本次时间 − last_review_at)/86400.0)`（f64，本次时间 = graded_at ?? created_at）；首次复习无 last_review_at → reps==0 分支。
`R(c) = (1 + elapsed/(9·stability))^(−1)`；**无 FSRS 状态的概念 R 项不参与 U(c)**（由新概念项接管）。

**score → rating**：`<0.5 again；[0.5,0.7) hard；[0.7,0.9) good；≥0.9 easy`。fold 中使用的 score = final ?? provisional。

**BKT**（s=0.10，g=0.20，free_explain 的 g=0.05，l=0.10）：
- 判对（score ≥ 0.75）：`p' = p(1−s)/(p(1−s)+(1−p)g)`，再 `p ← p' + (1−p')·l`
- 判错（score ≤ 0.40）：`p' = p·s/(p·s+(1−p)(1−g))`（无学习转移）
- 死区 (0.40, 0.75)：p 不动，只记录。

**校准**：`conf_norm=(self_confidence−1)/4`；`gap = conf_norm − score`；`calib_gap ← 0.7·calib_gap + 0.3·gap`。
Brier 用二值结果（≥0.75→1，≤0.40→0，死区跳过）：`brier_ewma ← 0.7·brier_ewma + 0.3·(conf_norm − 结果01)²`。
**幻影标记（P01 粗版）**：`attempt_count ≥ 2 ∧ calib_gap ≥ 0.25 ∧ p_known < 0.6`。

**misconception_active(c)**：存在近 14 天内带 misconception_id 的 attempt，且其后该概念无 final_score ≥ 0.75 的 attempt。

**U(c)**：`U = 0.40·(1−R) + 0.30·max(0, calib_gap) + 0.20·misconception_active + 0.10·新概念可引入`
新概念可引入 = 无 attempt ∧ 全部 prerequisite 的 p_known ≥ 0.6。
**决定性平手排序**：U 相同 → `seed_order` 小者 → id 字典序。（测试依赖此决定性。）

**strict-citation**：citation `{evidence_id, quote}`：evidence_id ∈ 该 attempt 关联证据；`quote.trim().len() ∈ [8,220]` 且为证据 text 子串；任一失败 → 拒收 → 重试 1 次 → 降级（启发式 + grade_queue）。

**乐观更新**：`provisional_score = 0.1 + 0.8·conf_norm`（meta 可调）；final 到达 → 该概念全量重放 + 打印差异。

## 4. P03A — MIRT（潜因子层）

- **a_c ≡ 1.0（v1 锁死，不拟合）**——n=1 可辨识性优先。
- 任务难度 d_t（logit，meta 表 `mirt.d.<task_type>`）：`recall −0.30；choose −0.15；cloze 0.00；rewrite +0.15；apply/translate +0.30；transfer/free_produce +0.50`。
- 软标签 `y = final_score`；预测 `p̂ = σ(q_c·θ − b_c − d_t)`。
- 在线更新（P06G 后）：`gradient_k = (y − p̂)·q_ck`；每维累积 `g2_k ← g2_k + gradient_k²`；`Δθ_k = eta·gradient_k / sqrt(g2_k + adagrad_epsilon)`，逐元素帽 `|Δθ_k| ≤ step_cap`；每夜收缩 `θ ← θ·(1 − shrink)`（`g2` 不收缩，保留历史步长记忆）。
- 每条 graded attempt 记 `attempts.theta_version` 与 `attempts.theta_scope`：`shared` 使用全局 `theta.version`；`pack:<id>` 使用 `pack_theta.version`。`theta_history` / `pack_theta_history` 每夜快照后各自 version+1。
- **BKT-MIRT 融合**：`p̂_known = λ·BKT + (1−λ)·σ(q·θ−b−d_t)`，`λ = n_c/(n_c + 5)`。
- **P03O shadow gate**：主 `p̂_known` 在 v1 仍使用上一条 λ 融合，不切换产品行为；`fused_p_known` 额外输出逆方差 shadow 融合、BKT 方差、MIRT 方差、shadow 权重与融合方差。BKT 方差用 Bernoulli posterior 工程近似并随 `attempt_count` 收缩；MIRT 方差只把 AdaGrad `g2` 与 `q` 当作信息量工程近似，不宣称为严格协方差；BKT 无样本 evidence、无效 `g2` 或非 finite 方差时 shadow 回退到当前 λ 融合（`g2=0` 是有效但高不确定度的冷启动估计）。
- Q 初始化：pack 安装时 LLM 按图式先验产出 q0（维度命名表 meta('latent.dims') JSON）；LLM 不可用 → q0 = onehot(track 维)。

## 5. P03B — 夜间巩固（逐步配方）

1. 残差入 `residual_stats`：90 天窗口、按 ISO 周分桶，`Z[c,w] = mean(y − p̂)`；覆盖 <4 周的概念跳过。
2. 概念两两相关：公共周 ≥4 才算；average-linkage 层次聚类，阈值 ρ ≥ 0.5，簇 ≥ 3 概念 → 候选新维。
3. LLM 溯因命名（strict-citation 引 ≥3 条 attempts），拒收则丢弃该候选。
4. 试装：K+1，簇成员新分量载荷 0.5、其余 0 → 重拟合受影响 q 行（步骤 5）→ **留出集 = 时间序最后 20% 的 graded attempts**；**接受当且仅当留出 logloss 改善 ≥ 0.01**，否则整体回滚。写 consolidation_runs。
5. q 行重拟合（n_c ≥ 8 的概念）：`min Σ BCE(y_i, σ(q·θ̂_{v_i} − b_c − d_{t_i})) + 0.5·‖q − q0‖²`，θ̂ 按 `attempts.theta_scope + attempts.theta_version` 查 `theta_history` 或 `pack_theta_history`；旧 `theta_scope` 缺失视为 `shared`。优化 = 100 步梯度下降（lr 0.1）即可。
6. 维度合并：`|corr(q_·j, q_·k)| > 0.85` → 平均合并；K ≤ 64 硬帽。

## 6. P03C — 几何与结构

- 嵌入：`POLARIS_EMBED_BASE_URL/MODEL/API_KEY`（OpenAI-compatible `/v1/embeddings`），单位化，维度入 meta；不可用 → 几何层整体停用（它只负责提议）。
- HNSW：hnsw_rs 或 instant-distance，M=16，ef_search=64。
- `struct(a,b)`：取各自 2-hop 类型化邻域；节点贪心配对（按嵌入 cos 降序）；边匹配 = 类型相同且两端已配对；`score = 匹配边数 / max(|E_a|,|E_b|)`；≥ 0.4 进 maps_to 候选（仍须 LLM 解释 + 验证门）。
- `coh(a,b)` = §5 的 Z 行相关（复用）。
- `assoc = 0.15·cos_E + 0.35·cos_Q + 0.25·struct + 0.25·coh`；`discover = (0.35·cos_Q + 0.25·struct + 0.25·coh)·(1 − cos_E)`。

## 7. P03D — 状态 HMM 与 hazard

- 观测粒度 = attempt。特征 `x = [z_latency(按 task_type 的个人 z 分), min(hints,3), resid = y − p̂, consec_fail, conf_delta(自评−个人均值), 间隔桶(0:<1m, 1:<10m, 2:≥10m), session_min]`。
- 发射 = 对角高斯，σ 全 1.0，先验均值表（行=状态，列=前 5 个特征 [z_lat, hints, resid, consec, conf_delta]）：

| 状态 | z_lat | hints | resid | consec | conf_delta |
|---|---|---|---|---|---|
| 心流 | −0.5 | 0.2 | +0.10 | 0.2 | +0.2 |
| 生产性困惑 | +0.5 | 0.8 | −0.20 | 1.0 | 0.0 |
| 挫败 | +1.0 | 1.5 | −0.30 | 2.5 | −0.5 |
| 无聊 | −0.8 | 0.1 | 0.00 | 0.3 | 0.0 |
| 焦虑 | +0.5 | 0.5 | −0.10 | 1.0 | −0.8 |
| 疲劳 | +0.8 | 0.6 | −0.15 | 1.5 | −0.2 |

（疲劳另以 session_min 高均值区分；无聊以间隔桶大区分。）
- 转移矩阵 A 初始：对角 0.7，其余 0.06；前向滤波在线跑；**EM 重估每周一次且 graded attempts ≥ 200 才启用**，否则一直用先验参数。
- **门控（降级语义）**：状态后验加入后对"下一动作（继续/求hint/放弃）"的预测 AUC 必须比无状态基线高 ≥ 0.03，否则状态层只记录、不得调策略。
- hazard：`P(本 attempt 后 10 分钟内放弃 session) = σ(β·[状态后验(6), calib_gap, consec_fail, hint速率, sin/cos(时段), session_min])`；L2 逻辑回归每周拟合；AUC ≥ 0.70 才允许其参与调度与镜像报告。

## 8. P04C — 摩擦 / 签名 / MRT / Thompson

- **摩擦（固定权重的指数定义，不与 g/h 联合拟合——防过参化）**：
  `φ = 0.4·(1−p̂) + 0.2·min(1, 僵局秒/600) + 0.2·(提示延迟档/2) + 0.2·脚手架等级`
  提示延迟档 ∈ {0:立即, 1:30s, 2:120s}；脚手架等级 = task_type 序数归一（工作样例 0 … 自由迁移 1）。
- g(φ)：φ 十分桶，结果 = 该概念下次到期的 final_score，isotonic 回归；h(φ)：该桶实测放弃率。`φ* = argmax g − 1.0·h`（0.05 网格）。
- **MRT**：决策点 = `next` 选 move 时；以 ε=0.2 概率从"U 前 3 的合法备选"均匀替换；mrt_log 落 prereg_id（预登记 JSON：窗口/ε/候选集/主效应假设/最小 n）。
- **签名收缩估计**：上下文桶 = 状态(6) × 相(7 含未判)。每分量效应 = 桶内随机化样本均值 − 同桶基线均值；收缩：`post = (n·x̄ + 10·μ_lit)/(n + 10)`，μ_lit 来自 moves 库每 move 的文献先验向量。
- **Thompson**：结果 = 7 天内同概念 final ≥ 0.75（1/0）；先验 `m_lit = clamp(0.5 + 0.1·d_lit, 0.2, 0.8)`，`Beta(10·m_lit, 10·(1−m_lit))` 起步。
- **F5 育种准入**：候选 move 必须先写 `bred_moves.prereg_json` 与 `mrt_log.prereg_id`；候选/在位者样本写入同一 `context_hash` 下的 `moves_effects`；`P(τ_candidate > τ_incumbent) > breeding.admit_p` 且双方 `n >= breeding.min_n` 才能从 `preregistered` 进入 `admitted`；准入后持续评估，若胜出概率低于 `breeding.retire_p` 则转 `retired`。

## 9. P03E — 相图判据与误解语法

**相判据（操作性；条件不足 = 未判）**：
- 幻影：n≥2 ∧ calib_gap≥0.25 ∧ p<0.6
- 脆弱：p≥0.6 ∧ max_depth ≤ explain
- 惰性：p≥0.6 ∧ 原情境成功≥2 ∧ 新情境失败≥2（任务带 context_novel 标志位）
- 僵硬：p≥0.6 ∧ max_depth≥apply ∧ transfer 尝试（≥2 次）均 <0.5
- 活跃：p≥0.7 ∧ ≥1 次 transfer 成功
- 自动化：活跃 ∧ 该 task_type 个人 latency 中位 < 个人全局 25 分位 ∧ 样本≥3
- **未判 → 调度自动派发"探针任务"**：缺哪类证据派哪类（缺 transfer 证据派迁移题，缺新情境证据派换景题）。数据不足本身触发取证。

**误解语法 G_u**：pattern 枚举 8 类：`overgeneralization | boundary-blindness | symbol-referent-confusion | causal-inversion | fluency-illusion | procedural-conceptual-gap | granularity-mismatch | interference-confusion`。
规则 = `{pattern, tag_scope, Beta 后验(precision), evidence_ids}`；预测命中窗口 30 天；Beta(1,1) 起步；`P(precision < 0.3) > 0.8` → 规则退役。

### 9.1 P06H — 相变动力学 shadow gate

- 数据源只读：`behavior_events(type='phase_transition')`，payload 使用 P03E 已记录的 `{from,to,concept_id,attempt_id}`；未知相名、缺字段或 malformed JSON 计入 ignored，不参与统计。
- 相集合使用现有 `Phase::ALL` 的 8 相：`undetermined, phantom, fluctuation, settling, solidification, transfer, generation, regression`。路线图旧稿里的“7 相”不作为实现依据。
- 输出 8x8 计数矩阵与行归一化转移概率；无观测行概率全 0，不做平滑伪造。
- 目标集合 = `transfer|generation`。对每个起点相解 Markov hitting time；目标不可达、存在非目标吸收风险或线性方程奇异时返回 `None`。
- 验证门：按时间序 holdout，把 Markov 下一相预测与“静态相不变”基线分别记录 accuracy/logloss；样本不足时 `skipped`。阈值走 `phase_dynamics.min_shadow_ready_transitions`、`phase_dynamics.min_validation_transitions`、`phase_dynamics.holdout_frac`，均为 A 类手动参数。未过门前只作为 shadow 统计，不改变相判据、调度、MRT、报告或默认产品行为。

### 9.2 P06I — G_u 层级 Beta 超先验 shadow gate

- 数据源只读：`gu_rules`、`attempts.grader_json.pattern_tags`、`concepts`、`edges`。本票不新增表，不写回 `gu_rules` 生命周期字段。
- baseline = 现行平坦 `Beta(1,1)`；hierarchical shadow = 同 pattern 既有规则概念与当前规则一跳图谱邻域在 holdout 起点前的 pattern 命中/未命中证据，折算为 bounded pseudo-count。
- pseudo-count 强度上限由 `gu_prior.max_prior_strength` 控制；无同 pattern / 邻域来源证据时退化为 `Beta(1,1)`。
- holdout = 当前规则 `last_seen` 之后、`gu.window_days` 内、当前规则概念集合上的 final attempts；标签 = grader_json 是否包含该 rule pattern。
- 验证摘要分别记录 flat 与 hierarchical 的 sequential Beta predictive logloss/Brier/accuracy；样本不足时 `skipped`。阈值走 `gu_prior.min_shadow_rules`、`gu_prior.min_holdout_attempts`、`gu_prior.max_prior_strength`，均为 A 类手动参数。未过门前只作为 shadow 统计，不改变 G_u 生命周期、调度、评分、报告或默认产品行为。

## 10. 参数登记处（参数认识论——本节是"不僵硬"的结构保证）

**三类参数制，严禁混淆**：

- **A【结构/治理】**：定义性常数与验证门槛。改 A = 改系统定义或验收标准，**只能用户手改**，自调优禁止触碰。
- **B【经验缺省】**：来自文献/工程判断的**起点值，不是真理**。预期被数据接管：能离线重放评估的由 P03H 自调优；影响"给了什么任务"的（无反事实数据）只能 MRT 在线对比或手动。
- **C【在线拟合】**：本来就是系统学的量（θ、Q、b_c、HMM 参数、hazard β、g/h 曲线、签名、Thompson α/β、FSRS 个人参数）。登记处里的只是**初始化**。

**调优途径标记**：`重放`=事件溯源反事实重放可离线评估；`MRT`=干预参数需在线随机化对比；`手动`=用户裁决。

| key | 默认 | 类 | 边界 | 调优途径 | 用途 |
|---|---|---|---|---|---|
| bkt.p_init | 0.20 | B | [0.05,0.50] | 重放(P03H) | p_known 初值；pack 可按概念覆盖 |
| bkt.slip / guess / guess_explain / learn | 0.10/0.20/0.05/0.10 | B | [0.02,0.30]/[0.05,0.40]/[0,0.20]/[0.02,0.40] | 重放(P03H，经典 BKT 个人拟合) | BKT |
| bkt.cut_hi / cut_lo | 0.75/0.40 | B | [0.60,0.90]/[0.20,0.50] | 手动（改标签语义，谨慎） | 判对/判错阈 |
| calib.ewma | 0.30 | B | [0.10,0.50] | 重放(P03H) | 校准步长 |
| calib.phantom_gap / _p / _n | 0.25/0.60/2 | B | [0.15,0.40]/[0.4,0.8]/[2,5] | 重放(P03H，目标=幻影标记的前瞻验证率) | 幻影判据 |
| sched.w_r/w_cal/w_mis/w_new | .40/.30/.20/.10 | B | 单纯形(和=1) | **MRT**（影响给什么任务，无反事实） | U(c) 权重；P04C 后渐被签名选法取代 |
| sched.mis_window_days / prereq_p | 14 / 0.60 | B | [7,30]/[0.4,0.8] | 重放 / MRT | 窗口与门槛 |
| grade.provisional_base / slope | 0.10/0.80 | B | — | **重放**（直接回归历史 (conf, final) 对） | 乐观落账 |
| grade.quote_min / quote_max | 8 / 220 | A | — | 不调 | strict-citation 校验语义 |
| fsrs.r_again / r_hard / r_good | .5/.7/.9 | B | ±0.1 | MRT/手动 | score→rating |
| fsrs.w[0..16] | Polaris 移植值 | C | — | 个人复习史拟合（P06J，显式 Fit + 留出对拍门） | 遗忘曲线 |
| fsrs_fit.min_attempts / min_holdout_predictions / holdout_frac / accept_margin | 100/20/0.20/0.005 | **A** | [1,100000]/[1,100000]/[0.05,0.50]/— | 不调（FSRS Fit 验证门） | `fsrs.w` 个人拟合的样本量、留出与接受门 |
| latent.k | 32 | B | [8,64] | 巩固过程实际管理 | 初始维数 |
| latent.k_max | 64 | A | — | 不调 | 硬帽 |
| mirt.eta / step_cap / shrink / adagrad_epsilon / fuse_n0 | .05/.05/1e−3/1e−8/5 | B | [.01,.2]/[.01,.1]/—/[1e−12,1e−3]/[2,20] | 重放(P03H，目标=留出 logloss) | θ AdaGrad 更新与融合 |
| mirt.d.* | §4 表 | B | [−1,+1] | 重放(P03H，小步) | 任务难度 |
| mirt.a_c | ≡1.0 | A | — | 解锁=未来票+门 | 判别度（v1 结构锁死） |
| consol.*（窗口/阈值/margin…） | §5 | B | 保守 | 手动（巩固自身已有门） | 巩固超参 |
| hmm.em_min_n | 200 | B | [100,500] | 手动 | EM 启用 |
| hmm.gate_auc_margin | 0.03 | **A** | — | 不调（验收标准） | 状态层门 |
| hazard.auc_gate | 0.70 | **A** | — | 不调（验收标准） | hazard 门 |
| phase_dynamics.min_shadow_ready_transitions | 3 | **A** | [1,1000] | 不调（shadow 验证门） | 相变动力学达到 shadow_ready 的最少有效迁移数 |
| phase_dynamics.min_validation_transitions | 8 | **A** | [2,1000] | 不调（shadow 验证门） | 相变动力学 holdout 验证最少有效迁移数 |
| phase_dynamics.holdout_frac | 0.20 | **A** | [0.05,0.50] | 不调（shadow 验证门） | 相变动力学时间序 holdout 比例 |
| gu_prior.min_shadow_rules | 1 | **A** | [1,1000] | 不调（shadow 验证门） | G_u 层级先验 shadow_ready 的最少可评估规则数 |
| gu_prior.min_holdout_attempts | 6 | **A** | [1,10000] | 不调（shadow 验证门） | G_u 层级先验 holdout 验证最少 attempt 数 |
| gu_prior.max_prior_strength | 20 | **A** | [0,1000] | 不调（shadow 验证门） | G_u 层级先验可折算的最大 pseudo-count 强度 |
| friction.w1..w4 | .4/.2/.2/.2 | **A** | — | 不调（φ 的指数定义） | 摩擦定义 |
| friction.lambda | 1.0 | B | [0.5,3.0] | MRT/手动（个人风险厌恶度） | φ\* 取舍 |
| mrt.epsilon | 0.20 | B | [0.05,0.30] | 手动/按计划随签名收窄而衰减 | 探索率 |
| sig.shrink_n0 / thompson.prior_n | 10/10 | B | [5,30] | 重放(P03H) | 收缩强度 |
| breeding.admit_p / retire_p / min_n | .80/.50/20 | **A** | [0.5,0.99]/[0.01,0.80]/[2,1000] | 不调（F5 验证门） | 育种准入、退役与样本量门 |
| gu.retire_p / retire_thresh / window_days | .30/.80/30 | B | — | 重放(P03H，目标=前瞻 precision) | 误解语法 |
| consol.accept_margin / holdout_frac | 0.01/0.20 | **A** | — | 不调（验收标准） | 巩固门 |

**实现要求**：config 模块为每个参数携带（默认值, 边界, 类型标签, 调优途径）四元组——后续自调优不改代码、只读登记处。

**FSRS Fit 规则（P06J）**：`fsrs.w` 是 C 类 Fit 参数，不进入 P03J 的 B 类夜间自调优白名单。显式运行个人拟合时，只读取 `final_score IS NOT NULL` 的复习史，按时间序 prequential replay：每条可预测复习先用此前 FSRS state 预测 `R`，再 fold 当前结果；最后 `fsrs_fit.holdout_frac` 的可预测复习作为 holdout。候选 `fsrs.w` 的 holdout logloss 相比当前值改善 ≥ `fsrs_fit.accept_margin` 才写入 `meta('fsrs.w')`，并写 `param_tuning_runs(param='fsrs.w', metric='fsrs_holdout_logloss')`；接受后必须重放相关概念，刷新 `mastery_states.fsrs_json` 与 `next_due_at`。样本不足或不过门不得改变默认调度。

## 11. 性能预算（"高效"的保证）

| 操作 | 复杂度 | 预算 |
|---|---|---|
| U(c) 全表选题 | O(概念数) | <10ms @ 10k 概念 |
| fold 单条 attempt | O(1) | <50µs |
| 概念全量重放 | O(该概念 attempts) | <1ms/百条 |
| θ 更新 / p̂ 预测 | O(K) | <1µs |
| HMM 前向一步 | O(S²)=36 | <1µs |
| HNSW 查询 | ~O(log n) | <5ms |
| 夜间巩固/EM/拟合 | 重 | 离线，无预算约束 |

属性测试要求：增量 fold == 全量重放（任意 attempt 序列）；同输入同输出（决定性，含平手排序）。

## 12. 参数自调优（P03H，"数据接管 B 类"的机制）

事件溯源的红利：**估计类参数可以反事实重放**——换一组 BKT/校准/融合参数，把全部历史 attempts 重新 fold 一遍，在时间序留出（最后 20%）上算目标指标，离线完成、零风险。

夜间自调优 job 规则：
1. 只碰 B 类且调优途径=重放的参数；**A 类（含一切验证门槛）禁止触碰**——系统不许给自己改及格线。
2. 每晚最多调 1–2 个参数（避免同时漂移不可归因）；候选按"当前最差预测指标→负责参数"映射表轮转。
3. 方法：边界内三点/网格搜索；评估=留出指标重放；**新值必须改善 ≥ margin 才生效**，否则保持原值。
4. 全程写 `param_tuning_runs(id, ran_at, param, old_value, new_value, metric, delta, status)`（accepted|rejected）——和巩固同等审计纪律。
5. 途径=MRT 的参数（sched.w_\*、friction.lambda、fsrs.r_\*）不许重放调（没有反事实数据），只能进 MRT 预登记对比或由用户手改。
6. 镜像报告有权基于证据建议手动参数（"你的 cut_hi=0.75 对你偏松，证据：…"），但只建议、不执行。
