# Polaris Core 漫画系统事实底稿

> 本文是 Polaris Core 自用漫画项目的事实底稿。它只回答“系统到底是什么”，不承担完整分镜和视觉设定职责。
> 角色设定见 `docs/polaris-core-comic-character-bible.md`；强化脚本见 `docs/polaris-core-comic-script-v0.md`。

## 1. 文档定位

这份文档用于约束后续漫画化表达，防止画面很好看但系统被讲偏。

它的读者包括：

- 写分镜的人；
- 生成图片提示词的人；
- 审查漫画是否忠于 Polaris Core 的人；
- 未来把漫画拆成海报、长图或演示稿的人。

它不是：

- 完整漫画脚本；
- 视觉设计稿；
- 项目实现计划；
- 票据说明书。

如果本文、漫画脚本、口头设定之间有冲突，本文只在“系统事实”层面优先；工程事实仍以 `SPEC.md`、`docs/DATA_MODEL.md` 和 `docs/MASTER_PLAN.md` 为准。

## 2. 一句话理解

Polaris Core 是一个领域无关的本地学习引擎。

它的目标不是做某一门课程，也不是做聊天机器人，而是建立一个可审计、可接入、可迁移的学习闭环：

```text
验证真懂 → 定位模糊 → 针对性补缺
```

这里的“懂”不是短期记住，也不是回答流畅，而是：

- 有真实学习证据；
- 经过统一评分口径；
- 能追溯到原文；
- 能看到自信与正确之间的差距；
- 能在概念图谱上定位问题；
- 能据此安排下一步学习任务。

漫画可以把这个闭环表现得有戏剧性，但不能把它改成“AI 陪聊学习”“刷题得分系统”或“单纯背诵复习器”。

## 3. 系统边界

Polaris Core 的核心边界如下：

- 它是引擎，不是课程。
- Rust pack 是第一条验证轨道，不是系统本体。
- 课程接入必须走 Domain Pack。
- 内核不能写入领域特定逻辑。
- 外部 AI 可以讲解、追问、陪伴、提交证据，但不能直接改掌握度。
- 同步路径零 LLM，用户打开就能看到 Tier 0 状态。
- 评分、诊断、调度必须 evidence-bound。
- 数据主权在本地 SQLite。

漫画里可以出现老师、助手、法官、城市、雷达、车站等比喻，但裁决权必须始终属于 Polaris Core 的确定性引擎。

## 4. 三个核心支柱

### 4.1 抽象引擎

抽象引擎回答“你会什么”。

它维护学习者的知识状态，而不是只看“上次答对了没”。核心分量包括：

| 分量 | 含义 | 漫画表达 |
|---|---|---|
| R | 保持度，来自 FSRS | 记忆保质期 |
| p_known | 真的会的概率，来自 BKT | 掌握概率指针 |
| C | 校准，自信与正确的差 | 真实之镜 |
| D | 深度，recall/explain/apply/transfer | 理解层级阶梯 |
| theta | 学习者潜在技能向量 | Mona 的星图坐标 |
| q | 概念载荷向量 | 概念在星图里的方向 |

注意：FSRS 只负责保持度和复习时机，不等于真正掌握。

### 4.2 心智动力学引擎

心智动力学引擎回答“你此刻处于什么状态”。

它计划识别心流、生产性困惑、挫败、无聊、焦虑、疲劳等状态，并用于未来的镜像报告、hazard 放弃预测和 MRT 个性化策略。

漫画可以预告这一层，但要明确它仍在后续票中推进，不能写成当前已经完整实现。

### 4.3 教学策略引擎

教学策略引擎回答“下一步怎么教”。

当前基础策略包括：

- 从调度器中选出高优先级概念；
- 如果前置概念没掌握，优先修复前置缺口；
- 如果存在 confusion 边，生成辨析任务；
- 通过 MCP 给外部导师输出结构化教学指令。

漫画里不要表现成系统只下达一个唯一命令。更准确的画法是：系统给出 2-3 个候选任务，其中一个被标为推荐。

## 5. 数据地基：事件溯源

事实源不是 `mastery_states`，而是：

- `attempts`
- `behavior_events`
- pack 种子数据

`mastery_states` 只是这些事实折叠出来的物化视图。

这意味着：

- 掌握度可以重放；
- 崩溃后可以恢复；
- final 评分回来后可以重新计算；
- provisional 和 final 可以共存；
- 系统能解释某个掌握度数字从哪里来；
- 测试可以验证“增量 fold”和“全量重放”结果一致。

适合漫画化的核心比喻是“没有橡皮擦的城市”：新事实不是擦掉旧状态，而是把历史胶片重新播放并折叠出新状态。

## 6. 课程接入：Domain Pack

Polaris Core 不直接内置英语、Rust、金融学、心理学或其他课程逻辑。

课程要进入系统，必须先变成 Domain Pack。当前最小结构包括：

```text
packs/<domain>/
  pack.toml
  concepts.toml
  misconceptions.toml
  rubric.md
  moves.toml
```

设计目标还包括：

```text
ingest.toml
```

`ingest.toml` 用于声明外部证据如何映射成系统可处理的学习记录。当前仓库已有 `P05A0 课程接入协议 v1` 票，专门用于把这套协议文档化、稳定化，并补齐面向外部课程作者的指南。

漫画可用比喻：

- Domain Pack 是入城许可证和城市规划图；
- `concepts.toml` 是建筑清单；
- `edges` 是道路、桥梁和依赖关系；
- `rubric.md` 是评分尺；
- `moves.toml` 是教学招式库；
- validator 是城门守卫。

## 7. 评分机制：LLM 不能自由判分

Polaris Core 的评分规则很严格。

LLM 不是自由发表意见，而是 Tier 1 后台评分员。它必须返回结构化 JSON，并带上 citations。

系统会检查：

- `score` 被限制在 `[0, 1]`；
- `depth` 必须是 `recall`、`explain`、`apply`、`transfer` 之一；
- citation 不能为空；
- citation 的 `evidence_id` 必须属于当前 attempt；
- citation 的 `quote` 必须是 evidence 原文子串；
- quote 长度必须符合 `grade.quote_min` 和 `grade.quote_max`。

如果 LLM 不可用、返回格式错误、引用不合规，系统会：

- 使用启发式 provisional score；
- 把 attempt 放入 `grade_queue`；
- 后续可通过 `grade-pending` 重试。

漫画重点：法官可以判分，但证据锁链负责审判法官。

## 8. 掌握度更新与调度

每条 attempt 会影响：

- BKT 的 `p_known`；
- 校准差 `calib_gap`；
- Brier EWMA；
- FSRS 状态；
- lapse 数；
- attempt count；
- last depth；
- max depth。

调度效用函数的核心形式是：

```text
U = w_r * (1 - R)
  + w_cal * max(0, calib_gap)
  + w_mis * misconception_active
  + w_new * new_concept_open
```

调度不是“随机复习”，也不是“永远挑战最难”。它要找的是当前最有学习收益的补缺点。

## 9. 图谱与诊断

图谱层回答：

> 这个错误背后可能是哪条知识关系出了问题？

当前图谱支持：

- `prerequisite`：前置关系；
- `confusion`：容易混淆；
- `component_of`：组成关系；
- `instantiates`：概念实例化某个图式；
- `maps_to`：两个结构之间的映射。

需要注意：

- `confusion` 是边；
- `misconceptions` 是误解记录或误解模式，不要写成“误解节点”；
- 辨析任务要强调边界、反例和区分线索。

## 10. MIRT、几何候选与夜间巩固

### 10.1 MIRT

MIRT 层负责跨概念、跨领域的潜在技能建模。

核心公式是：

```text
p_hat = sigmoid(q · theta - b - d_t)
```

其中：

- `theta` 是学习者潜在技能向量；
- `q` 是概念载荷向量；
- `b` 是概念难度；
- `d_t` 是任务类型难度。

漫画可以把 `theta` 画成 Mona 的星图坐标，把 `q` 画成每个概念指向星图不同方向的箭头。

### 10.2 BKT-MIRT 融合

系统不会盲目信任潜因子预测。

证据少时，多参考 MIRT 先验；证据多时，多相信该概念自己的 BKT 记录。

漫画里可以用天平表现：

- 左托盘：MIRT 预测；
- 右托盘：真实 attempt 记录；
- 随着 evidence 增加，天平逐渐靠向真实记录。

### 10.3 几何候选层

几何层负责快速联想和候选发现。

它可以组合 embedding 相似度、q 相似度、结构相似度和残差相关，提出关联候选。

铁律：

> 几何只负责提议，不负责裁决。

### 10.4 夜间巩固

夜间巩固是慢速抽象，不是实时魔法。

当前 v1 已建立 theta 快照、残差统计、候选发现和 `consolidation_runs` 审计轨迹。当前漫画如果表现“候选新维度通过验证并进入主网”，必须标注为目标设计或未来阶段，不应暗示当前代码已经完全实现。

## 11. MCP 与外部导师

Polaris Core 通过 MCP 暴露给 Codex、Claude Code、IDE AI、Cursor 等外部导师。

当前 MCP 工具包括：

- `get_next_task`
- `submit_evidence`
- `get_teaching_instruction`

资源包括：

- `polaris://status`
- `polaris://concept/{id}/diagnosis`

漫画里外部导师可以非常热闹，但它们必须围着同一个事实源工作。它们可以讲课、提问、解释、提交证据，不能直接修改掌握度。

## 12. 当前实现与规划边界

截至当前队列状态，已完成：

- P01 最小闭环；
- P02A 类型化超图；
- P02B 图谱感知诊断；
- P02C MCP server；
- P03A MIRT 潜因子层；
- P03B 夜间巩固 v1；
- P03C 几何候选层 v1。

仍在规划或后续票中：

- 正式课程接入协议文档；
- `ingest.toml` 的实现和 validator；
- 第二个 pack；
- 多 pack 的 latent dimension 映射；
- HMM 学习者状态层；
- hazard 放弃预测；
- 知识相图 UI；
- 误解语法 G_u；
- 镜像报告；
- Bloom 深度正式评分；
- 参数自调优；
- MRT 和教法签名；
- Tauri UI；
- HTTP API；
- 教法育种。

漫画脚本必须区分“已实现”“已有骨架”“规划中”“未来目标”。

## 13. 漫画化铁律

后续脚本、分镜和图像提示词不能改变这些事实：

- Polaris Core 是引擎，不是课程；
- Rust 是样板 pack，不是内核绑定的课程目标；
- 课程接入需要协议；
- 掌握度来自 evidence 和 attempts 的重放；
- LLM 评分必须 strict-citation；
- 外部 AI 不能直接改掌握度；
- FSRS 不等于掌握；
- 图谱诊断用于定位缺口；
- MIRT 用于跨概念和跨域预测；
- 夜间巩固是慢速抽象；
- 几何层只提议，不裁决；
- 系统给任务候选，不做粗暴单一路径命令；
- 当前仍缺正式课程接入协议文档和多 pack 映射。

## 14. 推荐产物结构

后续维护时建议保持三份文档：

1. `docs/polaris-core-comic-system-brief.md`  
   系统事实底稿，只放事实与边界。

2. `docs/polaris-core-comic-character-bible.md`  
   Mona、哆啦A梦和视觉符号设定。

3. `docs/polaris-core-comic-script-v0.md`  
   72 页漫画脚本和分镜强化稿。

这样处理后，事实、角色和剧情不会互相污染，后续继续扩写也更稳。
