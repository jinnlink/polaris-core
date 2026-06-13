# P04A Tauri 常驻小窗：Tier 0 状态镜子契约

## 状态

Done

## 服务主命题

定位模糊 → 针对性补缺。

## 背景

`SPEC.md` 要求同步路径无 LLM，打开即见的内容只读 Tier 0 状态；`MASTER_PLAN.md` 冻结了 Phase 4 的前端形态：Tauri 常驻小窗 + 可展开工作区，主角是状态镜子，且状态镜子以相图呈现。当前仓库只有 Rust core/CLI/MCP，没有 Tauri、Vite、Node 或 HTTP 脚手架；因此本票先交付桌面壳必须消费的稳定状态镜子契约，后续 P04B HTTP API 与 Tauri 壳复用同一结构。

## 范围

1. 扩展 `status_snapshot`：
   - 继续只读 SQLite 物化状态，不调用 LLM、不写业务状态。
   - 暴露 `generated_at`，作为小窗渲染时的快照时间。
   - 暴露稳定有序的 `phase_counts`，覆盖所有已登记相位；无概念的相位计数为 0。
   - 保留概念列表中的 `p_known`、`retrieval`、`calib_gap`、`phase`，供展开工作区使用。
2. 扩展 CLI：
   - `polaris status --json` 输出同一个状态镜子 JSON。
   - 现有 `polaris status` 文本输出保持兼容。
3. 复用：
   - MCP `polaris://status` 自动返回新字段。
   - P04B HTTP API 和 Tauri 壳不得另起一套状态模型。

## 禁区

- 不把 UI 代码放进 `polaris-core`。
- 不新增 Tauri/npm/前端脚手架依赖，除非能进入验收基线真实构建。
- 不新增会改变调度或掌握度的相位算法。
- 不直接读写冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不碰票外脏文件。

## 验收

```powershell
cargo test -p polaris-core --test p04a_desktop_status
cargo test -p polaris-cli status_json_serializes_desktop_mirror_fields
cargo test -p polaris-cli status_text_keeps_existing_cli_shape
cargo run -p polaris-cli -- --db target\p04a-status.db init --pack packs/rust
cargo run -p polaris-cli -- --db target\p04a-status.db status --json
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如默认 target 的 clippy 遇到 Windows 文件锁，可使用隔离 target 复跑同参数，并在交付记录写明。

## 本轮范围（2026-06-13）

- 已确认当前仓库没有 Tauri/前端脚手架。
- 本轮只做 P04A 的 Tier 0 状态镜子数据契约与 CLI JSON 出口。
- 后续 P04B HTTP API 直接暴露该契约；Tauri 壳只做薄 UI，不重新计算状态。

## 交付记录（2026-06-13 23:51 +08:00）

### 变更清单

- `status_snapshot` 新增 `generated_at` 与稳定有序 `phase_counts`；`phase_counts` 覆盖 `Phase::ALL` 的 8 个相位，未知相位归并为 `undetermined`。
- `polaris status --json` 输出同一份 Tier 0 状态镜子 JSON；原 `polaris status` 文本输出改为复用同一快照并保持旧格式。
- MCP `polaris://status` 自动继承新字段，不另起状态模型。
- 新增 P04A 集成测试，覆盖桌面状态镜子相位计数；新增 CLI JSON 与文本格式测试。
- 未新增 Tauri/npm/HTTP 依赖，未把 UI 逻辑写进 core。

### 子 agent 审查

- 审查结论：未发现阻断代码问题；确认 `status_snapshot` 只读 SQLite/FSRS/phase，不调用 LLM、不写业务状态；确认 JSON 与 MCP 复用同一出口。
- 审查要求处理：
  - `.gitignore` 为票外已有改动，本票提交时不纳入 staging。
  - 补充 `status_text_keeps_existing_cli_shape`，覆盖文本输出兼容性。

### 验收输出

```powershell
> cargo test -p polaris-core --test p04a_desktop_status
running 1 test
test status_snapshot_exposes_stable_phase_counts_for_desktop_mirror ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

> cargo test -p polaris-cli status_json_serializes_desktop_mirror_fields
running 1 test
test tests::status_json_serializes_desktop_mirror_fields ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out

> cargo test -p polaris-cli status_text_keeps_existing_cli_shape
running 1 test
test tests::status_text_keeps_existing_cli_shape ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out

> cargo run -p polaris-cli -- --db target\p04a-status-final.db init --pack packs/rust
initialized

> cargo run -p polaris-cli -- --db target\p04a-status-final.db status --json
{
  "generated_at": "2026-06-13T15:50:09Z",
  "due_today": 0,
  "phase_counts": [
    {"phase": "undetermined", "count": 24},
    {"phase": "phantom", "count": 0},
    {"phase": "fluctuation", "count": 0},
    {"phase": "settling", "count": 0},
    {"phase": "solidification", "count": 0},
    {"phase": "transfer", "count": 0},
    {"phase": "generation", "count": 0},
    {"phase": "regression", "count": 0}
  ],
  "concepts": [
    {"concept_id": "ownership", "name": "Ownership", "retrieval": null, "p_known": 0.2, "calib_gap": 0.0, "phase": "undetermined"}
  ... 23 more concepts ...
  ]
}

> cargo fmt --check
# no output

> cargo clippy --workspace --all-targets -- -D warnings
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)

> cargo clippy --workspace --all-targets --target-dir target\p04a-clippy -- -D warnings
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.83s

> cargo test --workspace
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Doc-tests polaris_core
```

### 技术选择说明

- P04A 先落状态镜子契约，不直接引入 Tauri 壳：当前仓库没有前端脚手架，强行新增未验证桌面依赖会扩大票面风险。该契约正是后续 P04B HTTP API 与 Tauri 常驻小窗的共享输入。
- `Phase::ALL` 作为唯一相位枚举顺序，避免前端、HTTP、MCP 各自维护相位列表。
- 文本状态输出复用 `StatusSnapshot`，避免 CLI 与 MCP/JSON 走两套 SQL。

### 回滚方式

- 删除 `crates/polaris-core/tests/p04a_desktop_status.rs`。
- 还原 `crates/polaris-core/src/phase.rs` 中 `Phase::ALL`、`crates/polaris-core/src/status.rs` 的 `generated_at`/`phase_counts` 扩展、`crates/polaris-cli/src/main.rs` 的 `status --json` 与文本 helper。
- 还原 `docs/tickets/QUEUE.md` 的 P04A 状态，并删除本票文件。
- 验收临时库 `target\p04a-status.db`、`target\p04a-status-final.db` 与隔离 clippy 目录 `target\p04a-clippy` 可直接删除。
