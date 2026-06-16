# P11B MCP/HTTP API 稳定性合约

状态：已通过验收（2026-06-17）
服务主命题环节：全环节（外部接口可演进底座）

## 背景

P11A 已经把 SQLite schema 版本、迁移账本和 doctor 可见性补齐。当前项目继续走向 1.0 release 前，还缺一层同等清晰的外部接口承诺：HTTP API 服务本地 UI 和本地客户端，MCP 是 Tier 2 主入口。`PRODUCT_ROADMAP.md` 已把「MCP/HTTP API 稳定性合约」列为 1.0 release 时强制补齐的版本化与 deprecation 政策。

本票不新增业务能力，也不重新设计接口；目标是把现有公开面写成契约，并用结构化 contract tests 防止后续改动无意破坏客户端。

## 范围

1. 建立当前公开面契约文档：
   - 新增 `docs/API_CONTRACT.md`。
   - 记录 HTTP route、method、成功状态码、稳定顶层 JSON 字段与错误形状。
   - 记录 MCP `tools/list`、`resources/list`、`resources/read` 的稳定名称、URI 与顶层响应形状。
   - 写明兼容性规则：稳定字段只能 additive 扩展；字段删除、重命名、语义变化必须走 deprecation；实验性字段不得被误写成稳定承诺。
2. HTTP contract tests：
   - 覆盖 `/health`、`/status`、`/learner-mirror`、`/trust`、`/next`、`/evidence`、`/feedback`。
   - 使用解析后的 JSON 断言字段和类型，不做整串 snapshot。
   - 覆盖稳定错误形状：404、405、400 至少各有一条结构化断言。
3. MCP contract tests：
   - 覆盖 `tools/list` 的稳定工具名和基础 schema。
   - 覆盖 `resources/list` 的稳定资源 URI。
   - 覆盖 `resources/read` 读取 `polaris://status` 与 `polaris://trust` 的顶层形状。
   - 覆盖未知 method / 未知 resource 的稳定错误响应形状。
4. 如确有必要，可增加很薄的测试辅助函数或常量，但不得把本票做成 OpenAPI/MCP 生成器。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p polaris-cli http
cargo test -p polaris-cli mcp
cargo test -p polaris-cli contract
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

专项验收要求：

- `docs/API_CONTRACT.md` 能直接回答当前 HTTP/MCP 哪些字段稳定、哪些变更必须走 deprecation。
- Contract tests 不依赖 JSON 字段顺序。
- 本票不修改数据库 schema，不提升 `CURRENT_SCHEMA_VERSION`。
- 本票不改变掌握度、调度、报告、MRT、breeding、G_u、FSRS、MIRT 公式或参数。

## 禁区

- 不新增 HTTP/MCP 业务能力。
- 不引入 axum、OpenAPI 生成器、JSON schema 生成器或新的服务框架。
- 不改数据库 schema、迁移账本或 `CURRENT_SCHEMA_VERSION`。
- 不改掌握度、调度、评分、报告、MRT、breeding、G_u、FSRS、MIRT 行为。
- 不把尚未明确承诺的深层字段写成稳定字段；深层结构可描述为「当前暴露，但稳定承诺限于顶层字段」。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 本轮范围（2026-06-17）

- 当前状态：P11A 已提交，QUEUE 无 In Progress；本票按 `PRODUCT_ROADMAP.md` 中 1.0 release 前 API 稳定性候选转正式票并认领。
- 已有非本票改动：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`、`target_codex_reviewNndQmJ/`。本票不得回退或混入。
- 子 agent 研究结论：P11B 是 P11A 后最合适的工程硬化小票；数学深化候选后续继续按 shadow gate 切票，不混入本票。
- 预计修改面：`docs/API_CONTRACT.md`、`crates/polaris-cli/src/http.rs`、`crates/polaris-cli/src/mcp.rs`、`docs/tickets/QUEUE.md` 和本票文件。

## 交付记录（2026-06-17）

### 变更清单

- 新增 `docs/API_CONTRACT.md`
  - 定义 HTTP v1 和 MCP v1 当前稳定公开面。
  - 写明兼容性规则、废弃策略、稳定顶层字段和错误响应形状。
  - 明确 MCP `submit_evidence` 与 HTTP `/evidence` 只在请求字段和外层返回字段上对齐，保留当前 MCP submit 路径语义，不把外部评分当掌握度权威。
  - 明确 `polaris://concept/{id}/diagnosis` 本票只稳定模板发现能力，读取 payload 暂不纳入 v1 稳定承诺。
- `crates/polaris-cli/src/http.rs`
  - 新增 HTTP contract tests，覆盖 `/health`、`/status`、`/learner-mirror`、`/trust`、`/next`、`/evidence`、`/feedback` 的稳定顶层字段。
  - 新增 404、405、400 稳定错误形状断言。
  - 通过 `include_str!` 绑定 `docs/API_CONTRACT.md`，避免契约文档和测试脱节。
- `crates/polaris-cli/src/mcp.rs`
  - 新增 MCP contract tests，覆盖 `initialize`、`tools/list`、`resources/list`、`resources/templates/list`、`resources/read` 和错误形状。
  - 工具/资源/模板断言改为稳定项存在，不依赖列表顺序或完整全集，允许后续 additive 扩展。
  - 保留结构化 JSON 断言，不做整串 snapshot。
- `docs/tickets/QUEUE.md`
  - P11B 从 backlog 候选转正式票，并在验收后标记通过。

### TDD 红灯

先写 contract tests 后运行：

```text
> cargo test -p polaris-cli contract
error: couldn't read `crates\polaris-cli\src\../../../docs/API_CONTRACT.md`: 系统找不到指定的文件。 (os error 2)
```

随后补 `docs/API_CONTRACT.md`，contract tests 转绿。

### 审查与修复

只读审查 agent 发现 4 项问题，已处理：

- 文档过度承诺 MCP `submit_evidence` 与 HTTP `/evidence` 等价：已改为只承诺请求字段和外层返回字段对齐，并说明 MCP 保留当前 submit 路径语义。
- MCP contract tests 对列表顺序和全集过敏：已改为按 `name` / `uri` / `uriTemplate` 查找稳定项。
- MCP `initialize` 已公开但文档未说明：已补文档小节和 contract test。
- `polaris://concept/{id}/diagnosis` 模板已公开但 payload 未测试：已明确本票只稳定模板发现能力，payload 稳定化留给后续正式票。

### 验收输出

```text
> cargo fmt --check
exit 0
```

```text
> cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.11s
```

说明：普通 sandbox 内 clippy 两次命中 Windows `target` rmeta 写入权限问题：

```text
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris-57ed2e745a270c60.rmeta: 拒绝访问。 (os error 5)
```

已按权限规则提升后重跑同一命令并通过。

```text
> cargo test -p polaris-cli http
running 20 tests
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 50 filtered out
```

```text
> cargo test -p polaris-cli mcp
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 53 filtered out
```

```text
> cargo test -p polaris-cli contract
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 62 filtered out
```

```text
> cargo test --workspace
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

```text
> git diff --check
exit 0
```

`git diff --check` 仅输出 Windows LF/CRLF 提示，无空白错误。

### 回滚方式

- 代码与文档回滚：`git revert <P11B 提交>`。
- 本票未修改数据库 schema、`CURRENT_SCHEMA_VERSION`、掌握度/调度/评分/报告/MRT/breeding/G_u/FSRS/MIRT 行为；无需数据迁移回滚。
