# 课程接入协议 v1

课程接入协议（Course Integration Protocol）定义外部课程如何接入 Polaris Core。外部课程不能把课件、章节、题库或领域逻辑写进内核；它只能提供一组声明式 pack 文件。内核消费这些文件，生成概念、边、误解、评分标准和教学 move。

本协议服务主命题：验证真懂 → 定位模糊 → 针对性补缺。pack 的职责是给引擎一个可验证、可追踪、可调度的知识结构，而不是替代学习证据或评分过程。

## 目录结构

一个课程 pack 放在独立目录下：

```text
packs/<domain>/
  pack.toml
  concepts.toml
  misconceptions.toml
  rubric.md
  moves.toml
  materials.toml       # 可选；材料身份与 pack 自定义 level 顺序
  ingest.toml          # 可选，v1 预留；当前 validator 不强制
```

当前 `polaris pack validate <dir>` 强制检查前 5 个文件。`materials.toml` 可选但一旦存在就会完整校验并在初始化时入库；`ingest.toml` 是协议预留文件，用于描述外部证据如何映射到 `evidence_items` 和 `attempts`，当前内核尚未消费它。

## 总体边界

- pack 只能声明数据：元信息、概念、边、误解、rubric、moves、证据映射。
- pack 不能包含可执行代码。
- 领域专用 ingest 适配器必须是独立进程、CLI、HTTP 或 MCP 工具，不能写进 core crate。
- 外部 AI 的判断只能作为 evidence 进入系统，不能直接覆盖 `mastery_states`。
- 所有评分产物仍需走 engine 的 evidence-bound 评分与 strict-citation 校验。

## `pack.toml`

`pack.toml` 定义 pack 身份。当前 validator 要求 `id` 和 `title` 非空；其他字段可作为向后兼容的元数据保留。

| 字段 | 必填 | 说明 |
|---|---:|---|
| `id` | 是 | 稳定机器 ID，写入 `concepts.pack`。建议使用小写 ASCII、数字、下划线或连字符。 |
| `title` | 是 | 人类可读名称。 |
| `version` | 否 | pack 内容版本，建议使用 SemVer，例如 `0.2.0`。当前 validator 不消费该字段。 |
| `name` | 否 | 与早期协议草案兼容的别名。若存在，应与 `id` 一致。 |
| `display_name` | 否 | 与早期协议草案兼容的展示名。若存在，应与 `title` 一致。 |
| `lang` | 否 | pack 主要语言，例如 `zh`、`en`。当前只作元数据。 |

最小示例：

```toml
id = "algorithms"
title = "Algorithms & Data Structures"
version = "0.1.0"
lang = "en"
```

## `concepts.toml`

`concepts.toml` 声明概念种子和初始图谱边。

### `[[concept]]`

| 字段 | 必填 | 说明 |
|---|---:|---|
| `id` | 是 | 概念稳定 ID。被 `attempts.concept_id`、edges 和 misconceptions 引用。 |
| `name` | 是 | 展示名称，也是 move 模板中 `{concept}` 的替换文本。 |
| `seed_order` | 是 | pack 内确定性排序。调度平手时先按 `seed_order`，再按 `id`。 |
| `kind` | 否 | 默认 `concept`。当前合法值：`concept`、`schema`、`misconception_induced`。课程作者通常只写 `concept` 或 `schema`。 |
| `p_init` | 否 | 初始 `p_known` 覆盖值。缺省使用 `meta('bkt.p_init')`。 |
| `generativity` | 否 | 默认 `unknown`。合法值：`generative`（可由规则推出未见同族实例）、`item`（逐项记忆）、`unknown`。只改变教学处方，不改变调度、难度或先验掌握度。 |

### `[[edge]]`

| 字段 | 必填 | 说明 |
|---|---:|---|
| `id` | 是 | 边稳定 ID。 |
| `src` | 是 | 起点概念 ID，必须存在于同一 pack 的 `[[concept]]`。 |
| `dst` | 是 | 终点概念 ID，必须存在于同一 pack 的 `[[concept]]`。 |
| `type` | 是 | 合法值：`prerequisite`、`confusion`、`component_of`、`instantiates`、`maps_to`。 |
| `weight` | 否 | 边权重，默认 `1.0`。 |
| `alignment_json` | 否 | 结构映射或对齐说明，必须是字符串形式 JSON。 |

`prerequisite` 边直接参与前置门控：目标概念只有在所有前置概念 `p_known >= sched.prereq_p` 后，才获得“新概念可引入”调度项。

示例：

```toml
[[concept]]
id = "arrays_lists"
name = "Arrays and linked lists"
seed_order = 1
generativity = "generative"

[[concept]]
id = "hash_tables"
name = "Hash tables"
seed_order = 2

[[edge]]
id = "arrays_lists_to_hash_tables"
src = "arrays_lists"
dst = "hash_tables"
type = "prerequisite"
```

## `misconceptions.toml`

`misconceptions.toml` 声明常见误解，供评分、诊断、调度和 G_u 误解语法对齐使用。

| 字段 | 必填 | 说明 |
|---|---:|---|
| `id` | 是 | 误解稳定 ID。 |
| `concept_id` | 是 | 关联概念，必须存在于 `concepts.toml`。 |
| `title` | 是 | 人类可读误解标题。 |
| `pattern` | 否 | 误解语法类别。建议使用 DATA_MODEL §9 的 8 类枚举。 |

推荐 `pattern`：

- `overgeneralization`
- `boundary-blindness`
- `symbol-referent-confusion`
- `causal-inversion`
- `fluency-illusion`
- `procedural-conceptual-gap`
- `granularity-mismatch`
- `interference-confusion`

示例：

```toml
[[misconception]]
id = "hash_always_o1"
concept_id = "hash_tables"
title = "Assuming hash table lookup is always O(1)"
pattern = "boundary-blindness"
```

当某次 failed attempt 带 `misconception_id`，且 14 天窗口内之后没有同概念 `final_score >= 0.75` 的 attempt，调度器会把该概念视为 active misconception，并提高修复优先级。

## `rubric.md`

`rubric.md` 是评分标准，不是 prompt 片段的随意堆叠。它至少要说明：

- 正确性如何判断。
- 深度如何判断。
- 通过标准是什么。
- 常见边界或退化场景是什么。
- 对 citation、证据和误解标注有什么要求。

当前 validator 只检查 `rubric.md` 非空；课程作者仍应把它写成可审查的评分契约。评分器必须基于 attempt 证据和 rubric 输出，不得凭外部 AI 直觉修改掌握度。

## `moves.toml`

`moves.toml` 声明教学动作模板。当前 validator 要求至少 1 条 move，且每条 move 的 `id`、`task_type`、`template` 最终非空。`task_type` 可省略，省略时按 move ID 使用默认映射。

| move ID | 默认 `task_type` | 目标深度 |
|---|---|---|
| `recall` | `recall` | recall |
| `explain` | `free_explain` | explain |
| `apply` | `apply` | apply |
| `analyze` | `analyze` | analyze |
| `evaluate` | `evaluate` | evaluate |
| `create` | `create` | create |
| `transfer` | `transfer` | transfer |

模板必须包含清楚任务目标。推荐使用 `{concept}` 占位符，由引擎替换为概念名称。

示例：

```toml
[[move]]
id = "recall"
task_type = "recall"
template = "用自己的话说明 {concept} 的核心约束。"

[[move]]
id = "apply"
task_type = "apply"
template = "给出一个使用 {concept} 的最小例子，并说明边界条件。"
```

## `materials.toml`（可选）

材料层只回答“学习者拿什么练”，不解释 level 的领域含义，也不存材料正文。`[levels].order` 是唯一顺序权威；`[[material]].level` 必须引用其中一个标签。材料 ID 在数据库内全局稳定，`source_ref` 只保存课程 URI、文件定位或外部引用。

```toml
[levels]
order = ["starter", "practice", "reference"]

[[material]]
id = "course_intro"
kind = "lesson"
level = "starter"
title = "入门讲义"
source_ref = "course://example/intro"
```

| 字段 | 必填 | 说明 |
|---|---:|---|
| `levels.order` | 是 | 非空、无重复的 level 标签数组；数组顺序即聚合展示顺序。 |
| `material.id` | 是 | 稳定材料 ID。提交时可作为 `material_id` 引用。 |
| `material.kind` | 是 | pack 自定义材料类别，内核只存储。 |
| `material.level` | 是 | 必须出现在 `levels.order`。 |
| `material.title` | 是 | 人类可读标题。 |
| `material.source_ref` | 是 | 材料引用；不得在此存正文。 |

CLI `submit --material-id <id>`、HTTP `POST /evidence` 和 MCP `submit_evidence` / `submit_task_response` 都接受可选 `material_id`；未知 ID 在任何学习事实写入前拒绝。只读聚合可通过 `polaris materials [--pack <id>] [--json]`、HTTP `POST /materials/performance` 或 MCP `get_material_performance` 获取。首次成功定义为每个“材料或 level × 概念”的首条有 final 分数记录中 `final_score >= 0.75` 的比例。

材料层在 P16L 只记录与聚合：不得改变 `p_known`、θ、预测成功率、任务选择或 `U(c)`。

## `ingest.toml`（预留）

`ingest.toml` 用于描述外部课程材料如何进入事件源。v1 不强制 pack 提供该文件；P05C 起，独立适配器可以读取它并向 stdout 输出标准 JSON Lines，再由 `polaris ingest --adapter-command <cmd>` 导入。core crate 仍不直接消费域特定 ingest 逻辑。

建议把映射分为 3 类：

| 类别 | 可写入表 | 用途 |
|---|---|---|
| 课程材料 | `evidence_items` | 作为讲义、题干、参考答案或上下文。 |
| 学习者作答 | `evidence_items` + `attempts` | 触发评分、掌握度 fold、误解检测和调度更新。 |
| 辅助上下文 | `evidence_items` | 供 strict-citation 引用，不直接生成 attempt。 |

建议形状：

```toml
[[source]]
id = "exercise_answer"
content_type = "text/plain"
maps_to = "attempt"
concept_id_field = "concept_id"
response_field = "answer"
self_confidence_field = "confidence"
```

领域适配器可以读取 `ingest.toml`，但必须通过公开入口提交 evidence 或 attempt，不得直接写 `mastery_states`。

### P05C 适配器输出（JSON Lines）

适配器是独立进程。Polaris 只读取其 stdout，每行一个 JSON 事件；stderr 仅用于诊断。导入命令：

```powershell
polaris ingest --adapter-command path\to\adapter.exe --adapter-arg --jsonl
```

支持事件：

```json
{"type":"evidence","session":"s1","source":"browser","content_type":"text/plain","text":"...","concept_ids":["ownership"]}
{"type":"attempt","session":"s1","concept_id":"ownership","task_type":"recall","prompt":"Explain ownership.","response":"Ownership moves values.","confidence":4,"latency_ms":1200,"hint_count":1}
```

约束：

- `evidence` 只写 `evidence_items`。
- `attempt` 必须走 `Engine::submit`，由内核统一 provisional/final grading、mastery fold、HMM 与 G_u 管线。
- 外部字段如 `final_score`、`external_score`、`mastery` 只能作为普通 JSON 字段被忽略；不得直接改 `attempts.final_score`、`mastery_states`、`theta` 或调度参数。
- 未知 `type` 直接拒绝整次导入，提醒适配器作者修正输出。

## 证据映射

课程接入后的事实源仍是：

- `evidence_items`：课程材料、作答文本、反馈文本等原始证据。
- `attempts`：一次可评分学习尝试，引用作答 evidence。
- `behavior_events`：延迟、提示、放弃、恢复、编辑等行为事件。

生成 attempt 的输入必须至少能确定：

- `concept_id`
- `task_type`
- `prompt_text`
- `response_text`
- `self_confidence`
- `latency_ms`
- `hint_count`

不能确定这些字段的内容，只能作为辅助 evidence，不应生成 attempt。

strict-citation 要求评分或报告引用 `{evidence_id, quote}`，其中 quote 必须是原始 evidence 文本子串，长度在配置边界内。引用失败时，产物必须拒收、重试或降级。

## validator 规则

当前 `polaris pack validate <dir>` 检查：

- 必须存在 `pack.toml`、`concepts.toml`、`misconceptions.toml`、`rubric.md`、`moves.toml`。
- `pack.toml` 能解析，且 `id`、`title` 非空。
- `concepts.toml` 能解析，所有 concept kind 与 generativity 枚举合法。
- 所有 edge type 合法。
- 所有 edge 的 `src`、`dst` 都引用已声明 concept。
- 所有 misconception 的 `concept_id` 都引用已声明 concept。
- `moves.toml` 至少包含 1 条 move，且归一化后的 `id`、`task_type`、`template` 非空。
- `rubric.md` 非空。
- 若存在 `materials.toml`，`levels.order` 不得含空值或重复值，材料必填字段非空，且每个 `material.level` 均已声明。
- 校验通过后输出概念数、prerequisite 边数和误解数。

当前 validator 不检查：

- `ingest.toml`。
- `version` 是否符合 SemVer。
- prerequisite 是否为 DAG。
- concept ID 是否符合特定字符集。
- `alignment_json` 是否为合法 JSON。
- `misconception.pattern` 是否属于推荐枚举。

这些未检查项仍属于协议建议。未来若升级为强制规则，应提高协议主版本或提供迁移窗口。

## 版本兼容

v1 的兼容原则：

- 新增可选字段是向后兼容。
- 删除或重命名必填字段是破坏性变更。
- 改变 validator 对既有合法 pack 的判定是破坏性变更，除非先提供迁移脚本或兼容窗口。
- `version` 是 pack 内容版本，不等于协议版本；协议版本由本文档管理。
- validator 当前忽略未知字段，因此 pack 作者可以保留 `name`、`display_name`、`lang` 等元数据，但不能依赖它们改变引擎行为。

## 作者迁移步骤

1. 列出现有课程的核心学习目标，合并成 10 到 50 个稳定 concept。
2. 给每个 concept 分配稳定 `id`、`name` 和 `seed_order`。
3. 只把真正的前置关系写成 `prerequisite`；容易混淆但不互为前置的关系写成 `confusion`。
4. 收集常见错误，写入 `misconceptions.toml`，并尽量标注 `pattern`。
5. 写 `rubric.md`，明确正确性、深度、边界和评分证据要求。
6. 写 `moves.toml`，优先覆盖 7 个 Bloom move。
7. 可选写 `materials.toml`，声明材料身份与 level 顺序。
8. 可选写 `ingest.toml`，描述课程平台或适配器如何生成 evidence 和 attempt。
9. 运行 `cargo run -p polaris-cli -- pack validate packs/<domain>`。
10. 用 `Engine::init_pack` 或 CLI 初始化到测试数据库，确认 `next_task` 返回该 pack 的概念。

## 常见报错

| 报错 | 含义 | 处理方式 |
|---|---|---|
| `missing required pack file` | 缺少必需文件。 | 补齐前 5 个必需文件。 |
| `failed to parse` | TOML 语法错误或字段类型不匹配。 | 检查数组表 `[[concept]]`、`[[edge]]`、`[[move]]` 写法。 |
| `edge ... references missing concept` | 边引用了不存在的 concept。 | 修正 `src` 或 `dst`，或补 concept。 |
| `misconception ... references missing concept` | 误解挂到了不存在的 concept。 | 修正 `concept_id`。 |
| `concept ... has invalid kind` | concept kind 不在合法集合内。 | 使用 `concept`、`schema` 或 `misconception_induced`。 |
| `edge ... has invalid type` | edge type 不在合法集合内。 | 使用 `prerequisite`、`confusion`、`component_of`、`instantiates` 或 `maps_to`。 |
| `pack must contain at least one move template` | `moves.toml` 为空或 move 字段为空。 | 至少声明 `recall` move。 |
| `rubric.md is empty` | rubric 无有效内容。 | 写入评分标准。 |

## 最小可用 pack

```toml
# pack.toml
id = "demo"
title = "Demo Course"
version = "0.1.0"
```

```toml
# concepts.toml
[[concept]]
id = "core"
name = "Core idea"
seed_order = 1
```

```toml
# misconceptions.toml
misconception = []
```

```markdown
# rubric.md

答案必须说清核心概念、适用边界和至少一个例子。
```

```toml
# moves.toml
[[move]]
id = "recall"
template = "用自己的话说明 {concept} 的核心约束。"
```

这个最小 pack 可以通过 validator，但它不一定是好课程。生产 pack 应补足 prerequisite、confusion、常见误解、完整 7-move 模板和可审查的 rubric。
