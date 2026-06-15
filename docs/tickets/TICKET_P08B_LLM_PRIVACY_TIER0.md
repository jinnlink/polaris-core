# P08B LLM 调用隐私清单 + 纯 Tier 0 模式

状态：已完成

服务主命题环节：信任前提（Local-persistent / Graceful degradation）

## 背景

polaris-core 已经把同步路径保持在 Tier 0，但用户目前无法快速回答两个问题：

1. 哪些命令或工具会把哪些数据发给外部模型？
2. 如果用户希望完全离线/纯本地，如何一键禁用所有 Tier 1 外部模型调用？

当前外部模型调用点主要有：

- grading：`LlmConfig::from_env()` 读取 `POLARIS_LLM_FAST_*` / `POLARIS_LLM_STRONG_*`，`grade_with_config` 会把 attempt response、rubric、evidence prompt 发给 OpenAI-compatible endpoint；失败降级为 heuristic。
- mirror narrative：`report --narrative` / MCP `run_mirror_report(narrative=true)` 复用 `LlmConfig`，把 report item claims 发送给 OpenAI-compatible endpoint 生成叙事；失败降级为 raw report。
- embedding：`OpenAiEmbeddingProvider::from_env()` 读取 `POLARIS_EMBED_*`，为几何候选层刷新概念/图式 embedding；不可用时几何层降级。

P08B 做信任地基：提供可读的调用清单，并新增 `POLARIS_TIER0_ONLY=1`，使上述外部模型入口全部视为 unavailable。

## 范围

1. 纯 Tier 0 环境开关：
   - 新增统一 helper，例如 `privacy::tier0_only_enabled()`。
   - `POLARIS_TIER0_ONLY` 取值为 `1` / `true` / `yes` / `on`（大小写不敏感）时启用；其他值或未设置时按现行 env 行为。
   - 当 `POLARIS_TIER0_ONLY=1|true|yes|on` 时：
     - `LlmConfig::from_env()` 必须返回 `Unavailable`，即使 `POLARIS_LLM_FAST_*` / `POLARIS_LLM_STRONG_*` 已配置。
     - `OpenAiEmbeddingProvider::from_env()` 必须返回 `None`，即使 `POLARIS_EMBED_*` 已配置。
   - 静态测试入口不受影响：`grade_pending_with_static_response` 与 `run_mirror_report_with_static_narrative` 仍可用于 deterministic 测试，因为它们不发外部请求。

2. 隐私清单数据结构：
   - 新增 `crates/polaris-core/src/privacy.rs`。
   - 提供 `PrivacyCallInventory` 与 `PrivacyCall`，字段建议：
     - `id`
     - `tier`
     - `trigger`
     - `env_keys`
     - `data_sent`
     - `degradation`
     - `disabled_when_tier0_only`
   - 条目 id 使用 snake_case，并带分类前缀；本票固定三项：
     - `llm_grade_attempt`
     - `llm_mirror_narrative`
     - `embed_concept`

3. CLI：
   - 新增命令 `polaris privacy show [--json]`。
   - 输出顶部必须主动显示当前 Tier0-only 状态：启用 / 未启用，并提示 `POLARIS_TIER0_ONLY=1` 可全禁外发。
   - 默认文本输出应列出调用 id、触发命令、外发数据、环境变量和降级行为。
   - `--json` 输出结构化 `PrivacyCallInventory`。

4. MCP/HTTP：
   - 本票只要求 CLI；MCP/HTTP 暴露留给 P10A 信任面板或后续小票。

5. 文档：
   - 新增或更新 `docs/PRIVACY.md`，内容与 `privacy show` 同源，不手写两份不一致清单。
   - 增加机器化同步测试：解析 `docs/PRIVACY.md` 中的 `id: <snake_case>`，与 `PrivacyCallInventory::all()` 的 id 集合做精确相等断言。
   - `QUEUE.md` 在审查通过后再标 P08B In Progress。

6. 未来扩展契约：
   - 任何未来新增外发通道（ingest 适配器、外部 webhook、MCP 外部调用、外部模型/检索服务等）必须同时：
     - 在 `PrivacyCallInventory` 增加条目；
     - 在 Tier0-only 模式下提供明确抑制或降级路径；
     - 增加对应测试。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试建议：

```powershell
cargo test -p polaris-core privacy
cargo test -p polaris-cli privacy
cargo test -p polaris-core --test p03c_geometry
cargo test -p polaris-core --test p06d_mirror_report_narrative
```

专项验收要求：

- 设置 `POLARIS_TIER0_ONLY=1` 且同时设置完整 `POLARIS_LLM_FAST_*` 时，`LlmConfig::from_env()` 仍为 `Unavailable`。
- 设置 `POLARIS_TIER0_ONLY=1` 且同时设置完整 `POLARIS_EMBED_*` 时，embedding provider 仍不可用。
- `submit` / `grade_pending` 在 Tier0-only 下走 heuristic / retry queue 降级，不阻塞、不外发。
- `report --narrative` 在 Tier0-only 下输出 raw report，`narrative=None`。
- `polaris privacy show` 文本输出顶部展示当前 Tier0-only 状态。
- `polaris privacy show --json` 至少包含 `llm_grade_attempt`、`llm_mirror_narrative`、`embed_concept` 三项。
- `docs/PRIVACY.md` 与 core inventory 的条目 id 集合由自动化测试强制一致。

## 禁区

- 不改变评分公式、相图判据、调度、MRT、breeding 或镜像报告断言生成逻辑。
- 不删除现有 OpenAI-compatible 能力；只新增显式禁用开关与可见性。
- 不把 Tier0-only 做成默认值；默认仍按现有环境变量行为。
- 不新增 UI/Tauri 面板；P10A 再做信任面板聚合。
- 不修改冻结参考仓库。

## 本轮范围（2026-06-15）

- 当前状态：P09A 已提交（`2447af3`），P09A 收口文档已提交（`69f6961`）。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、产品经理的 `docs/PRODUCT_ROADMAP.md`、`docs/ENHANCEMENT_ROADMAP.md` 与 `QUEUE.md` 产品形态轴线补充。本票不得回退或混入这些改动。
- 产品/架构审查结果：通过；补充要求已写入范围与验收。

## 交付记录（2026-06-15）

### 变更清单

- 新增 `crates/polaris-core/src/privacy.rs`：
  - `PrivacyCallInventory` / `PrivacyCall`。
  - `tier0_only_enabled()`，支持 `1/true/yes/on` 且大小写不敏感。
  - inventory 覆盖 `llm_grade_attempt`、`llm_mirror_narrative`、`embed_concept`。
- `crates/polaris-core/src/grader.rs`：
  - `LlmConfig::from_env()` 在 Tier0-only 下直接返回 `Unavailable`。
- `crates/polaris-core/src/geometry.rs`：
  - `OpenAiEmbeddingProvider::from_env()` 与 embedding env availability 在 Tier0-only 下视为不可用。
- `crates/polaris-cli/src/main.rs`：
  - 新增 `polaris privacy show [--json]`。
  - 文本输出顶部显示当前 Tier0-only 状态和 `POLARIS_TIER0_ONLY=1` 提示。
- 新增 `docs/PRIVACY.md`：
  - 三个 `id:` 与 core inventory 同步。
  - 记录未来新增外发通道必须进入 inventory 并支持 Tier0-only 抑制。
- 测试：
  - 新增 `p08b_privacy.rs`，覆盖 Tier0-only 禁 LLM、禁 embedding、inventory 必备条目、PRIVACY.md 同步。
  - CLI 单测覆盖 `privacy show` 和 `--json` 解析与 Tier0 状态提示。

### TDD 红灯记录

```text
cargo test -p polaris-core --test p08b_privacy
error: couldn't read `docs/PRIVACY.md`
error[E0432]: unresolved import `polaris_core::privacy`
exit 101
```

```text
cargo test -p polaris-cli privacy_show
error[E0433]: cannot find `privacy` in `polaris_core`
error[E0599]: no variant named `Privacy` found for enum `Commands`
error[E0425]: cannot find function `privacy_show_text` in this scope
exit 101
```

### 验收输出

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.02s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 31 passed
polaris-core unit: 63 passed
engine_submit_pipeline: 5 passed
engine_task_selection: 3 passed
p08b_privacy: 4 passed
all existing integration tests and doc-tests passed
exit 0
```

```text
cargo test -p polaris-core --test p08b_privacy
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-cli privacy_show
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p03c_geometry
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
cargo test -p polaris-core --test p06d_mirror_report_narrative
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- Tier0-only 只改变外部模型入口的 env 解析，不改默认行为。
- 静态响应测试入口保留，因为它们不发网络请求，仍是 deterministic 验收工具。
- embedding 与 LLM 一起纳入禁用面，避免“纯 Tier 0”仍外发概念文本。
- `docs/PRIVACY.md` 用 `id:` 行做机器校验，避免文档与 inventory 分叉。

### 待审事项

- 产品/架构审查结果：ship it。
