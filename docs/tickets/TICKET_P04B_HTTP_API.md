# P04B HTTP API 门

## 状态

Done

## 服务主命题

验证真懂 -> 定位模糊 -> 针对性补缺。

## 背景

`SPEC.md` 冻结了三道门：MCP、HTTP API、内置 LLM。其中 HTTP API 服务常驻伴随 UI 和其他本地客户端。P04A 已交付 Tier 0 状态镜子契约，本票打开 HTTP 门，但不引入 UI，也不提前实现 MRT。

## 范围

1. 在 `polaris-cli` 增加本地 HTTP API 服务入口：
   - `polaris serve-http --host 127.0.0.1 --port 8765`
   - 默认只绑定回环地址，避免意外暴露本地学习数据。
2. 暴露最小闭环端点：
   - `GET /health`：返回服务名与版本。
   - `GET /status`：返回 P04A `StatusSnapshot` JSON。
   - `POST /next`：按本地调度返回下一题和教学指令，并记录 `behavior_events(type='next')`。
   - `POST /evidence`：提交学习者回答；外部评分字段即便出现也不得写入 `final_score`。
3. 只使用 Tier 0 / 现有 Engine 同步路径；不调用 LLM。

## 禁区

- 不实现 Tauri UI、Web UI 或前端资产。
- 不实现 P04C MRT、摩擦曲线、签名后验。
- 不改掌握度、相图、调度算法。
- 不把 HTTP API 放进 `polaris-core` 的领域逻辑里；core 只保持引擎库。
- 不监听公网地址作为默认行为。
- 不移票外脏文件。

## 验收

```powershell
cargo test -p polaris-cli http_
cargo run -p polaris-cli -- serve-http --help
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如默认 target 的 clippy 遇到 Windows 文件锁，可使用隔离 target 重跑同参数，并在交付记录写明。

## 本轮范围（2026-06-14）

- 当前仓库已有 P04A `StatusSnapshot`，HTTP `/status` 必须直接复用。
- HTTP API 先覆盖本地 UI 需要的状态、调度、证据提交最小闭环。
- 网络服务采用阻塞本地 server；不引入异步运行时或未锁定前端依赖。

## 交付记录（2026-06-14 00:56 +08:00）

### 变更清单

- 在 `polaris-cli` 增加 `serve-http --host 127.0.0.1 --port 8765` 命令，默认只绑定 loopback。
- 增加 `crates/polaris-cli/src/http.rs`，提供 `GET /health`、`GET /status`、`POST /next`、`POST /evidence`。
- `/status` 复用 P04A `StatusSnapshot`；`/next` 写入 `behavior_events(type='next')` 并返回教学指令。
- `/evidence` 不信任外部 `final_score` / `external_score`，只做 provisional 提交并进入 `grade_queue`。
- 在 `polaris-core` 增加 `Engine::submit_provisional`，复用原提交路径的 evidence、attempt、mastery replay 与降级入队语义；原 `Engine::submit` 保留同步评分语义。
- HTTP 默认不输出 CORS `Access-Control-Allow-Origin: *`；`OPTIONS` 返回 405，避免任意网页跨域读写本地学习 API。
- HTTP 请求解析增加请求大小、`Content-Length` 溢出/截断防护；坏 JSON 返回稳定 400 JSON，内部错误映射为 500 JSON。

### 子代理审查处理

- 必须修复：CORS `*` 风险。已移除默认 CORS 响应头，并用 `http_stream_serves_health_json` 断言不含 `Access-Control-Allow-Origin: *`。
- 必须修复：畸形或截断 `Content-Length` 可能 panic。已加入 checked length 与 incomplete body 400，并新增 `http_stream_rejects_truncated_body_without_panicking`。
- 必须修复：`/evidence` 可能同步触发 LLM。已改为 `submit_provisional`，新增 core 测试与 HTTP `grade_queue` 断言。
- 建议修改：handler 错误断连接。已将坏 JSON 映射 400，stream handler 内部错误映射 500。

### 验收输出

```powershell
> cargo test -p polaris-cli http_
running 9 tests
test http::tests::http_evidence_rejects_invalid_confidence ... ok
test http::tests::http_evidence_rejects_malformed_json_with_stable_error ... ok
test http::tests::http_status_reuses_p04a_status_snapshot ... ok
test http::tests::http_stream_serves_health_json ... ok
test http::tests::http_stream_rejects_truncated_body_without_panicking ... ok
test http::tests::http_evidence_queues_without_final_grading ... ok
test http::tests::http_health_returns_service_metadata ... ok
test http::tests::http_evidence_uses_engine_scoring_without_trusting_external_score ... ok
test http::tests::http_next_records_behavior_event_and_returns_instruction ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out
```

```powershell
> cargo run -p polaris-cli -- serve-http --help
Usage: polaris.exe serve-http [OPTIONS]

Options:
      --db <DB>
      --host <HOST>  [default: 127.0.0.1]
      --port <PORT>  [default: 8765]
  -h, --help         Print help
```

```powershell
> cargo fmt --check
# 无输出，exit 0
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
```

默认 target 再次遇到 Windows 文件锁。按本票验收说明，使用临时隔离 target 与 `-j 1` 重跑同参：

```powershell
> cargo clippy --workspace --all-targets --target-dir $env:TEMP\polaris-p04b-clippy-serial -j 1 -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 33s
```

```powershell
> cargo test --workspace
running 23 tests
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 66 tests
test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

...

running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

补充定向验证：

```powershell
> cargo test -p polaris-core submit_provisional_records_mastery_and_queues_retry
test engine::tests::submit_provisional_records_mastery_and_queues_retry ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 65 filtered out
```

### 回滚方式

```powershell
git revert <P04B提交SHA>
```
