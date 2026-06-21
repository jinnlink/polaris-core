# API 稳定性合约

状态：v1（2026-06-17）

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

HTTP 响应体均为 JSON。成功响应使用 HTTP 200；客户端错误使用 400、404、405；服务器内部错误使用 500。稳定错误形状：

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

### `POST /next`

用途：返回本地调度的下一题并记录 `next` 行为事件。

成功状态码：`200`

请求字段：

- `session`: string，可选；缺省为 `http`。

稳定顶层字段：

- `task`: object 或 null。
- `teaching_instruction`: object，当 `task` 非 null 时存在。

`task` 稳定子字段：

- `concept_id`
- `task_type`
- `prompt`
- `reason`

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

稳定顶层字段：

- `attempt_id`: string。
- `provisional_score`: number。
- `degraded`: boolean。

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
- `get_active_gu_rules`
- `run_mirror_report`
- `get_latest_mirror_report`
- `mark_report_assertion_inaccurate`
- `mark_report_assertion_accurate`
- `record_learner_feedback`
- `get_trust_panel`
- `detect_project_manifest`
- `capture_evidence`
- `get_learner_mirror`
- `submit_evidence`
- `get_teaching_instruction`

每个工具定义必须保留：

- `name`: string。
- `description`: string。
- `inputSchema`: object，顶层 `type` 为 `object`。

### `tools/call`

稳定工具调用结果形状：

- 成功：`result.content[0].type == "text"`，`result.content[0].text` 为 JSON 字符串。
- 工具级错误：`result.isError == true`，`result.content[0].text` 为错误文本。

关键工具语义：

- `get_next_task`: 与 HTTP `POST /next` 等价，缺省 session 为 `mcp`。
- `detect_project_manifest`: 从 `path`（可选，缺省为 MCP server 当前目录）向上发现 `p-os.toml`。找到时返回 `found=true`、`project_root`、`manifest_path`、`manifest`；未找到时返回 `found=false`，不视为工具错误。
- `capture_evidence`: 与 HTTP `POST /capture` 语义对齐，把外部学习资料保存为 pending raw capture；不得生成 attempt、不得改变 mastery、不得写 grade queue，外部评分字段不被信任。
- `get_learner_mirror`: 与 HTTP `GET /learner-mirror` 对齐，返回 `generated_at`、`confidence_curve`、`phase_distribution`、`recent_assertions`。
- `submit_evidence`: 请求字段和外层返回字段与 HTTP `POST /evidence` 对齐，但保留当前 MCP submit 路径语义：可使用引擎评分和后续提交流水线；外部评分字段仍不得作为掌握度权威。
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
