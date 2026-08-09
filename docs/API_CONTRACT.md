# API 稳定性合约

状态：v1（2026-08-08）

本文定义 Polaris Core 当前对外公开面的稳定承诺。HTTP API 服务本地 UI 和本地客户端；MCP API 服务 Tier 2 外部导师。除非另有说明，本合约只承诺顶层字段、工具名、资源 URI、错误形状和只读/写入语义；深层对象允许 additive 扩展。

## 兼容性规则

- 稳定 route、method、tool name、resource URI 不得删除或重命名。
- 稳定顶层字段只能 additive 扩展；删除、重命名、类型变化或语义反转都属于 breaking change。
- 稳定错误响应必须继续提供机器可读的错误文本字段。
- 新增字段默认是实验性，至少经过一张正式票写入本文后才成为稳定字段。
- 深层对象的当前内容可被客户端读取，但稳定承诺限于本文列出的顶层字段和明确列出的子字段。
- HTTP 与 MCP 只是外部门面；不得绕过 core 的 engine-owned scoring、schedule-first、evidence-bound 规则。

## 废弃策略

- 废弃必须先新增替代字段、route、tool 或 resource，再保留旧入口至少一个 minor release 周期。
- 废弃入口必须在本文标记为 deprecated，并说明替代入口。
- deprecated alias 只能继续映射到同一语义，不得改变旧客户端的行为。
- breaking change 必须开正式票，写清迁移方式、验收命令和回滚方式。

## HTTP v1

HTTP 响应体均为 JSON。成功响应使用 HTTP 200；客户端错误使用 400、403、404、405；服务器内部错误使用 500。稳定错误形状：

```json
{"error": "message"}
```

### `GET /health`

用途：服务探活。

成功状态码：`200`

稳定顶层字段：

- `service`: string，当前为 `polaris-core`。
- `version`: string，当前 crate 版本。

### `GET /status`

用途：读取 Tier 0 状态镜子。

成功状态码：`200`

稳定顶层字段：

- `current_pack`
- `theta_mode`
- `packs`
- `due_today`
- `phase_counts`
- `concepts`

兼容性说明：`packs`、`phase_counts`、`concepts` 内部字段允许 additive 扩展；客户端不得依赖数组顺序表达语义，除非字段本身声明排序。

### `GET /knowledge-map`

用途：读取由 Core 权威状态实时推导的当前知识地图。该入口只读，不创建第二份掌握度状态，也不接受手工“已掌握”写入。

成功状态码：`200`

查询参数：

- `scope`: `pack | global`，可选，缺省为 `pack`。`global` 只返回 Pack/潜变量维度聚合，不返回节点与边。
- `pack`: string，可选；Pack 视图缺省使用当前激活 Pack。
- `root`: string，可选；限定根概念或图式。
- `depth`: integer，可选，范围 `0..=8`，必须与 `root` 同时使用。
- `phase`: string，可选；稳定枚举为 `undetermined | phantom | fluctuation | settling | solidification | transfer | generation | regression`。
- `due`: string，可选；稳定枚举为 `new | due | scheduled | unscheduled`。
- `min_confidence`: number，可选，范围 `0..=1`。
- `limit`: integer，可选，范围 `1..=500`，缺省为 `100`。
- `cursor`: string，可选；只能原样使用上一页的 `next_cursor`，客户端不得解析。

未知或重复查询参数、非法编码、越界值及不兼容组合返回 `400`。

稳定顶层字段：

- `generated_at`
- `model_version`
- `query`
- `summary`
- `nodes`
- `edges`
- `next_cursor`

节点稳定语义包括 concept/schema 类型、Pack、retrieval、`p_known`、校准、相、到期状态、尝试/证据计数、`uncertainty` 与 `provenance`。状态来源只使用 `observed | latent_prediction | inherited_prior`；P16B 当前只会产生 `observed` 或 `inherited_prior`。没有 provenance 的边不会返回，并计入 `summary.omitted_edges_missing_provenance`。

### `GET /prediction-map`

用途：读取跨域预测地图、结构教学锚点与初始学习路径。该入口纯只读，不创建 attempt、mastery 或 MRT 预注册。

成功状态码：`200`

查询参数与 `GET /knowledge-map` 完全相同，并复用同一 `KnowledgeMapQuery` 序列化契约。

稳定顶层字段：

- `generated_at`
- `model_version`
- `query`
- `summary`
- `nodes`
- `anchors`
- `initial_paths`
- `next_cursor`

每个节点稳定提供 `observed`、`latent_prediction`、`inherited_prior` 三个独立可空字段，客户端不得把 `latent_prediction` 或 `inherited_prior` 展示为已掌握。每个 estimate 携带值、95% 区间、来源、门状态、模型版本、θ 作用域与 provenance。P18A 真实纵向验证前 latent 保持 `shadow`；不确定度不可用时为 `unfit` 且区间为 `[0,1]`。isolated Pack 仅返回 Pack 本地 latent 结果，`cross_domain=false`，不读取 shared θ。`anchors` 只包含已持久化、通过当前结构门且有 provenance 的跨 Pack `maps_to` 边；`initial_paths` 是本地调度器给出的最多 3 个可选行动。`scope=global` 时不伪造节点级预测，`nodes/anchors/initial_paths` 为空，Pack 和潜变量维度聚合保留在 `summary.packs` 与 `summary.dimensions`。

### `GET /profile`

用途：向本地 HTTP 集成读取 Global Learner Profile 摘要。该入口默认关闭；只有用户在本地设置中显式开启 `summary_sharing_enabled` 后返回 `200`，否则返回 `403` 和稳定 `error` 文本。

成功状态码：`200`

稳定顶层字段：

- `generated_at`
- `dimensions`
- `notice`

`dimensions[]` 稳定包含 scope、均值、方差、证据数、模型版本、门状态、provenance 和 evidence ids。该入口永不返回 `behavior_events(type='profile_measurement')`、题目回答或完整导出；不存在 POST/重置/删除 HTTP 入口。

### `GET /session`

用途：按 session id 读取已经显式收口的确定性小结。该入口只读，不会关闭 session，也不修改掌握度或调度。

查询参数：`session`，string，必填且不可重复。未找到已收口小结返回 `404`；不存在 POST 入口。

稳定顶层字段：

- `session_id`
- `started_at`
- `ended_at`
- `closed_at`
- `concepts_touched`
- `attempts_count`
- `top_stuck_concept_id`
- `next_entry_concept_id`
- `assertions`
- `generated_at`

`assertions` 最多 3 条，每条稳定包含 `concept_id`、`kind`、`text` 与非空 `evidence_ids`。

### `GET /learner-mirror`

用途：读取学习者静态镜像面板。

成功状态码：`200`

稳定顶层字段：

- `generated_at`
- `confidence_curve`
- `phase_distribution`
- `recent_assertions`

### `GET /trust`

用途：读取五框架门状态、活跃实验、近期活动与治理阈值。

成功状态码：`200`

稳定顶层字段：

- `gates`
- `active_breeding_experiments`
- `active_mrt_experiments`
- `recent_activity`
- `governance`

### `GET /ai-profile`

用途：读取本地 AI 交互偏好，供 CLI、HTTP 客户端和 AI IDE 调整语气、解释深度与介入频率。该 profile 不参与 mastery、调度或评分。

成功状态码：`200`

稳定顶层字段：

- `version`: integer。
- `persona`: string，稳定枚举为 `balanced_mentor | socratic_tutor | strict_coach | friendly_companion | direct_operator`。
- `verbosity`: string，稳定枚举为 `brief | normal | detailed`。
- `explanation_depth`: string，稳定枚举为 `answer_only | key_steps | deep | examples_first`。
- `proactivity`: string，稳定枚举为 `on_request | stuck_only | proactive`。
- `intervention_frequency`: string，稳定枚举为 `low | normal | high`。
- `correction_style`: string，稳定枚举为 `direct | guided | supportive`。
- `custom_notes`: string 或 null；最长 2000 字符。
- `guidance`: string，面向 AI IDE 的自然语言执行提示。

### `POST /ai-profile`

用途：更新本地 AI 交互偏好。请求可以只传需要改变的字段；非法枚举返回 400 且不写入。该入口只改变交互偏好，不改变学习事实。

成功状态码：`200`

请求字段：

- `persona`: string，可选。
- `verbosity`: string，可选。
- `explanation_depth`: string，可选。
- `proactivity`: string，可选。
- `intervention_frequency`: string，可选。
- `correction_style`: string，可选。
- `custom_notes`: string，可选；最长 2000 字符；空字符串清除已有补充说明。

稳定顶层字段：同 `GET /ai-profile`。

### `POST /capture`

用途：把外部学习资料先保存为待处理 capture，不生成 attempt，不改变掌握度、调度或 grade queue。请求中的外部评分字段不被信任。

成功状态码：`200`

请求字段：

- `text`: string，必填。
- `session`: string，可选；缺省为无 session。
- `source`: string，可选；缺省为 `paste`。
- `content_type`: string，可选；缺省为 `text/plain`。
- `learner_kind`: string，可选；稳定枚举为 `reference | own_answer | error_log | code_change | chat_excerpt | unknown`，缺省为 `reference`。
- `candidate_concept_ids`: string array，可选；仅作为候选，不触发概念新增或 mastery fold。
- `note`: string，可选。

稳定顶层字段：

- `capture_id`: string。
- `evidence_id`: string。
- `status`: string，P12C 成功写入时为 `pending`。
- `learner_kind`: string。
- `recorded_only`: boolean，P12C 成功写入时为 `true`。
- `message`: string，学生可读提示。

### `GET /inbox`

用途：读取学习收件箱中默认打开状态的 capture 条目（`pending | mapped | practice_ready`）。返回学生可读文案和最多 3 个动作，不展示掌握度参数、θ 或底层 `candidate_concept_ids_json`。

成功状态码：`200`

稳定顶层字段：

- `items`: array。

`items[]` 稳定子字段：

- `capture_id`: string。
- `evidence_id`: string。
- `status`: string，稳定枚举为 `pending | mapped | practice_ready | practiced | ignored | archived`。
- `learner_kind`: string。
- `source`: string。
- `content_type`: string。
- `text_preview`: string。
- `concept_hint`: string 或 null，面向学生的人类可读候选提示；不得要求学生填写 concept id。
- `note`: string 或 null。
- `created_at`: string。
- `updated_at`: string。
- `message`: string，学生可读状态文案。
- `actions`: array，最多 3 个。

`actions[]` 稳定子字段：

- `action`: string，稳定枚举为 `accept | defer | ignore | archive`。
- `label`: string，学生可读动作文案。

### `POST /inbox/action`

用途：对学习收件箱条目执行轻动作。`accept` 只把条目标记为 `practice_ready`，供后续练习桥接使用；不得生成 prompt、attempt、mastery 或 grade queue。

成功状态码：`200`

请求字段：

- `capture_id`: string，必填。
- `action`: string，必填；稳定枚举为 `accept | defer | ignore | archive`。
- `note`: string，可选。

稳定顶层字段：

- `capture_id`: string。
- `status`: string。
- `effect`: string，当前为 `recorded_only`。
- `message`: string，学生可读提示。

### `POST /inbox/practice`

用途：从 `practice_ready` 的学习收件箱条目生成一条确定性小题草稿。该入口只读学习资料和已有候选概念，不生成 attempt、不改变掌握度、不暴露 `p_known`、`theta` 或内部候选 JSON。

成功状态码：`200`

请求字段：

- `capture_id`: string，必填。

稳定顶层字段：

- `capture_id`: string。
- `evidence_id`: string。
- `status`: string，成功时为 `practice_ready`。
- `concept_hint`: string 或 null，学生可读概念提示。
- `task_type`: string，当前为 `explain`。
- `prompt`: string，给学生回答的小题。
- `source_excerpt`: string，资料摘要。
- `message`: string，学生可读提示。

### `POST /inbox/practice/submit`

用途：提交学生对学习收件箱小题的回答。该入口必须采集学生自评 `confidence`，然后复用 Polaris 引擎自有提交/评分路径；请求中的外部评分字段不被信任。成功后 capture 标记为 `practiced`。

成功状态码：`200`

请求字段：

- `capture_id`: string，必填。
- `session`: string，可选；缺省为 `http`。
- `response`: string，必填，学生自己的回答。
- `confidence`: integer，必填，范围 `1..=5`。

稳定顶层字段：

- `capture_id`: string。
- `attempt_id`: string。
- `status`: string，成功时为 `practiced`。
- `effect`: string，当前为 `submitted`。
- `message`: string，学生可读提示。
- `provisional_score`: number。
- `degraded`: boolean。

### `POST /next`

用途：返回本地调度的下一题并记录 `next` 行为事件。

成功状态码：`200`

请求字段：

- `session`: string，可选；缺省为 `http`。

稳定顶层字段：

- `task`: object 或 null。
- `teaching_turn_id`: string，当 `task` 非 null 时存在；供外部导师登记本次实际讲解。
- `teaching_instruction`: object，当 `task` 非 null 时存在。

`task` 稳定子字段：

- `concept_id`
- `task_type`
- `prompt`
- `reason`
- `context`: object 或 null；与 `teaching_instruction.context` 使用同一只读历史 DTO。

`teaching_instruction.context` 非空时稳定子字段：

- `recent_attempts`: array，最多 `teaching.context_attempt_limit` 条，默认 3，按时间倒序。
- `latest_failed_response`: string 或 null，只含最近失败的学习者原文摘要。
- `active_gu_rules`: array，只含当前 `status='active'` 的规则。
- `previous_anchor`: string 或 null，上一次实际交付的教学 anchor。

### `POST /teaching-turn/explanation`

用途：外部导师把刚才实际交付的讲解原文登记为 evidence，并关联到 `POST /next` 返回的教学回合。只记录事实，不判断讲解有效性，不进入学习者作答的 strict-citation 集合。

请求字段：

- `teaching_turn_id`: string，必填。
- `text`: string，必填且去空白后非空。

稳定顶层字段：

- `teaching_turn_id`: string。
- `evidence_id`: string。

### `POST /evidence`

用途：提交学习者证据，由引擎自有评分路径做乐观更新；外部评分字段不被信任。

成功状态码：`200`

请求字段：

- `session`: string，必填。
- `concept_id`: string，必填；`concept` 是兼容 alias。
- `response`: string，必填。
- `confidence`: integer，必填，范围 `1..=5`。
- `task_type`: string，可选；缺省为 `recall`。
- `prompt`: string，可选。
- `material_id`: string，可选；必须是已初始化 Pack 声明的材料 ID。未知 ID 返回 400 且不产生部分写入。
- `no_attempt_reason`: string，可选；稳定枚举为 `not_understood_prompt | no_recall | out_of_time | skipped`。必须由调用方根据学习者明确选择传入，不得推断。

稳定顶层字段：

- `attempt_id`: string。
- `provisional_score`: number。
- `degraded`: boolean。
- `no_attempt_reason`: string 或 null。非 null 时 `provisional_score` 为 null，且该 attempt 不进入 mastery、θ、校准、FSRS 或调度尝试计数。

### `POST /materials/performance`

用途：按材料及 Pack 声明的 level 顺序读取表现聚合。该入口只读，不改变 mastery、预测或调度。

请求字段：`pack` string，可选。

稳定顶层字段：

- `by_material`: array；元素包含 `material_id`、`pack`、`kind`、`level`、`title`、`attempt_count`、`average_final_score`、`first_success_rate`。
- `by_level`: array；元素包含 `pack`、`level`、`level_order` 和同样三个表现字段；数组按 Pack 声明顺序输出。

### `POST /feedback`

用途：记录学习者状态或暂停反馈。反馈只进入审计事件，不直接改掌握度、HMM 状态或调度。

成功状态码：`200`

请求字段：

- `session`: string，可选；缺省为 `http`。
- `kind`: string，必填；稳定枚举为 `state | pause`。
- `concept_id`: string，可选；`concept` 是兼容 alias。
- `state`: string，`kind=state` 时必填。
- `reason`: string，`kind=pause` 时必填。
- `note`: string，可选。

稳定顶层字段：

- `session_id`
- `kind`
- `effect`

## MCP v1

MCP 使用 JSON-RPC 2.0。通知方法 `notifications/*` 返回空响应。未知 method 使用 JSON-RPC error：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {"code": -32601, "message": "method not found"}
}
```

### stdio 传输

`polaris mcp` 接受两种本地 stdio 帧：

- `Content-Length: N\r\n\r\n<body>`，用于保留既有 AI IDE 客户端兼容。
- 单行 UTF-8 JSON + 换行（JSON Lines），用于当前 MCP SDK/Host。

每条有响应的请求使用该请求的输入帧格式回复；通知仍不产生响应。标准输出只写协议帧，不写日志或诊断文本。该兼容只属于传输层，不改变工具、资源、JSON-RPC 错误或业务 payload 契约。

### `initialize`

用途：MCP 标准握手。

稳定顶层字段：

- `protocolVersion`
- `capabilities`
- `serverInfo`

`serverInfo` 稳定子字段：

- `name`: string，当前为 `polaris-core`。
- `version`: string，当前 crate 版本。

### `tools/list`

稳定工具名：

- `get_next_task`
- `get_interleaved_batch`
- `get_phase_snapshot`
- `get_session_summary`
- `get_knowledge_map`
- `get_prediction_map`
- `get_global_profile`
- `get_active_gu_rules`
- `run_mirror_report`
- `get_latest_mirror_report`
- `mark_report_assertion_inaccurate`
- `mark_report_assertion_accurate`
- `record_learner_feedback`
- `get_trust_panel`
- `detect_project_manifest`
- `discover_learning_projects`
- `capture_evidence`
- `list_learner_inbox`
- `act_on_learner_inbox_item`
- `draft_inbox_practice`
- `submit_inbox_practice`
- `get_ai_interaction_profile`
- `update_ai_interaction_profile`
- `get_learner_mirror`
- `submit_evidence`
- `submit_task_response`
- `get_material_performance`
- `get_teaching_instruction`
- `record_teaching_explanation`

每个工具定义必须保留：

- `name`: string。
- `description`: string。
- `inputSchema`: object，顶层 `type` 为 `object`。

### `tools/call`

稳定工具调用结果形状：

- 成功：`result.content[0].type == "text"`，`result.content[0].text` 为 JSON 字符串。
- 工具级错误：`result.isError == true`，`result.content[0].text` 为错误文本。

关键工具语义：

- `get_next_task`: 与 HTTP `POST /next` 等价，缺省 session 为 `mcp`。MCP 返回额外的 `task_event_id` 与 `teaching_turn_id`；前者可交给 `submit_task_response` 建立严格作答回合，后者可交给 `record_teaching_explanation` 登记实际讲解。
- `get_knowledge_map`: 与 HTTP `GET /knowledge-map` 使用同一个 `KnowledgeMapQuery` 和 `KnowledgeMapSnapshot` 序列化契约；参数作为工具 arguments 传入，缺省为空对象。它只读取 Core 当前状态，不修改 mastery、调度或证据。
- `get_prediction_map`: 与 HTTP `GET /prediction-map` 使用同一个查询和 `PredictionMapSnapshot` 序列化契约；缺省 arguments 为空对象。它只读取 Core 预测、锚点和调度预览，不修改 mastery、MRT 或证据。
- `get_global_profile`: 与 HTTP `GET /profile` 使用同一无原始回答摘要；本地分享未开启时返回工具级 `isError=true`。该工具没有参数，不暴露回答、导出、重置或全部删除能力。
- `get_session_summary`: 与 HTTP `GET /session` 使用同一小结序列化契约；必填 `session`。只读取已显式关闭的小结，未找到时返回 JSON `null`，不得关闭 session 或改变学习状态。
- `detect_project_manifest`: 从 `path`（可选，缺省为 MCP server 当前目录）向上发现 `p-os.toml`。找到时返回 `found=true`、`project_root`、`manifest_path`、`manifest`；未找到时返回 `found=false`，不视为工具错误。
- `discover_learning_projects`: 从 `root`（可选，缺省为 MCP server 当前目录；`path` 可作为 alias）向下只读扫描 `p-os.toml` 学习项目声明，返回 `root` 与 `projects`。扫描会跳过 `_worktrees`、`.git`、`target` 等非课程目录，找到一个项目后不继续深入该项目内部；该工具只发现课程项目，不修改课程仓库、不生成掌握度。
- `capture_evidence`: 与 HTTP `POST /capture` 语义对齐，把外部学习资料保存为 pending raw capture；不得生成 attempt、不得改变 mastery、不得写 grade queue，外部评分字段不被信任。
- `list_learner_inbox`: 与 HTTP `GET /inbox` 语义对齐；可选 `statuses` 与 `limit` 参数，返回 `items`。输出必须保持学生可读，不展示内部参数。
- `act_on_learner_inbox_item`: 与 HTTP `POST /inbox/action` 语义对齐；`accept` 只标记 `practice_ready`，`defer` 保留稍后处理，`ignore` 隐藏，`archive` 归档。不得生成 attempt、mastery 或 grade queue。
- `draft_inbox_practice`: 与 HTTP `POST /inbox/practice` 语义对齐，从 `practice_ready` capture 生成学生可答的小题草稿；不得生成 attempt、mastery 或 grade queue，不暴露内部掌握度参数。
- `submit_inbox_practice`: 与 HTTP `POST /inbox/practice/submit` 语义对齐，提交学生回答和 `confidence`，复用引擎自有评分路径并把 capture 标为 `practiced`；外部评分字段仍不得作为掌握度权威。
- `record_teaching_explanation`: 与 HTTP `POST /teaching-turn/explanation` 等价；只接受 `get_next_task` 返回的教学回合并保存讲解原文 evidence，不自动归因、不改变 mastery，也不扩大 grader 可引用集合。
- `get_ai_interaction_profile`: 与 HTTP `GET /ai-profile` 语义对齐，返回本地 AI 交互偏好和 `guidance`；只读，不影响学习事实。
- `update_ai_interaction_profile`: 与 HTTP `POST /ai-profile` 语义对齐；仅在用户要求改变 AI 性格、话量、解释深度、主动程度、介入频率、纠错风格或补充说明时调用，不影响 mastery。
- `get_learner_mirror`: 与 HTTP `GET /learner-mirror` 对齐，返回 `generated_at`、`confidence_curve`、`phase_distribution`、`recent_assertions`。
- `submit_evidence`: 请求字段和外层返回字段与 HTTP `POST /evidence` 对齐，但保留当前 MCP submit 路径语义：可使用引擎评分和后续提交流水线；外部评分字段仍不得作为掌握度权威。可选 `no_attempt_reason` 使用同一枚举和不入 mastery 语义；可选 `material_id` 只关联已声明材料。
- `submit_task_response`: Tier 2 严格回合入口。必填 `session`、`task_event_id`、`response`、`confidence`（1..=5），可选 `no_attempt_reason` 与 `material_id`。`task_event_id` 必须是同一 session 的未提交 `get_next_task` 回执；内核从该事件复原 concept、task type 与 prompt，宿主传入的同名字段不参与提交。正常作答仍走 engine-owned scoring；显式未作答只记录 evidence/event/attempt，不评分、不折叠。两条路径均消费回执并写 `behavior_events(type='tier2_submission')`。不存在、跨 session、非 next、已提交回执、未知材料或非法原因必须拒绝且不得创建 attempt。旧 `submit_evidence` 不要求回执，保持兼容。
- `get_material_performance`: 与 HTTP `POST /materials/performance` 使用同一只读聚合契约；可选 `pack` 过滤，不改变学习事实。
- `record_learner_feedback`: 与 HTTP `POST /feedback` 等价，缺省 session 为 `mcp`。
- `get_trust_panel`: 与 `polaris://trust` 资源读取返回同一顶层形状。

### `resources/list`

稳定资源 URI：

- `polaris://status`
- `polaris://trust`

每个资源定义必须保留：

- `uri`: string。
- `name`: string。
- `description`: string。
- `mimeType`: `application/json`。

### `resources/templates/list`

稳定资源模板：

- `polaris://concept/{id}/diagnosis`

模板定义必须保留 `uriTemplate`、`name`、`description`、`mimeType`。

兼容性说明：本票只稳定模板发现能力；`polaris://concept/{id}/diagnosis` 的读取 payload 仍按当前诊断结构暴露，但顶层字段暂不纳入 v1 稳定承诺。后续若要稳定该 payload，必须补正式票和读取 contract tests。

### `resources/read`

成功结果形状：

- `result.contents`: array。
- `result.contents[0].uri`: string。
- `result.contents[0].mimeType`: `application/json`。
- `result.contents[0].text`: JSON 字符串。

稳定资源顶层字段：

- `polaris://status`: 同 HTTP `GET /status`。
- `polaris://trust`: 同 HTTP `GET /trust`。

未知资源使用工具级资源错误，而不是 JSON-RPC method error：

```json
{
  "result": {
    "contents": [
      {"uri": "polaris://error", "mimeType": "text/plain", "text": "unknown resource: ..."}
    ],
    "isError": true
  }
}
```
