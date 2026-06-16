# P08A 多 Pack 切换 + 数据隔离开关

状态：In Progress（2026-06-17 认领）
服务主命题环节：全环节（通用性）

## 背景

P05A0/P05A1/P05A 已经证明 Domain Pack 可以让内核跨 Rust、算法、英语运行，P03M 也已经把无 q 元数据时的 fallback 维度改为 `pack:<id>`。但当前日常使用仍是隐式多 pack：

- `polaris init --pack ...` 会安装 pack，却没有“当前正在学哪个 pack”的持久状态。
- `next_task`、交错 batch、`status` 会扫全量 `concepts`，多 pack 安装后会跨域混排。
- 用户看不到已安装 pack 列表、当前 pack、每个 pack 是否共享 θ。
- `theta` 当前是全局单例；如果用户在新域冷启动期想隔离 θ，必须有真实隔离路径，而不是只显示一个配置字段。

本票让“装新域 = 放一个 pack 目录”进入日常使用层：用户能列出 pack、切换 active pack、选择 `shared|isolated` θ 模式；切换后调度与状态出口必须按 active pack 生效。

## 范围

1. Pack 运行状态：
   - 新增 `meta('active_pack')`，表示当前学习上下文；未设置时保持旧行为：全 pack 参与。
   - `Engine::init_pack` 在首次安装 pack 且无 active pack 时自动设为该 pack，降低冷启动摩擦。
   - 新增每 pack `theta_mode`：`meta('pack.<id>.theta_mode') = 'shared'|'isolated'`，默认 `shared`。
   - 安装 pack 时持久化 `meta('pack.<id>.title')`，供列表展示。
2. CLI：
   - 新增 `polaris pack list [--json]`。
   - 新增 `polaris pack switch <pack> [--theta-mode shared|isolated]`。
   - `switch` 必须拒绝未安装 pack；成功后输出 active pack 与 θ 模式。
3. Active pack 过滤：
   - `next_task` 与 `get_interleaved_batch` 只在 active pack 内选题；无 active pack 时保留全库旧行为。
   - `status_snapshot` / CLI `status` / HTTP `GET /status` / MCP `polaris://status` 显示 `current_pack`、`theta_mode`、pack 列表，并按 active pack 过滤概念、相分布和 due count。
4. θ 隔离：
   - 保留全局 `theta` 作为 shared 模式。
   - 新增 pack 级 θ 存储：`pack_theta(pack, vec, g2, version, updated_at)` 与 `pack_theta_history(pack, version, vec, at)`。
   - `isolated` 模式下，该 pack 的 `latent_prediction`、`fused_p_known`、`update_theta_for_attempt` 使用 pack 级 θ。
   - `attempts.theta_scope` 记录本次 final 更新使用的 θ 作用域：`shared` 或 `pack:<id>`。
   - 夜间巩固 residual 回放按 `theta_scope` 读取对应历史；旧 attempt 缺 `theta_scope` 时按 `shared` 兼容。
5. Pack 安全性：
   - `init_pack` 必须拒绝跨 pack concept id 冲突，避免不同 pack 覆盖同一个全局 `concepts.id`。
   - 重复初始化同一 pack 仍保持幂等，不覆盖已有 concept q。
6. 只读外部入口：
   - HTTP 不新增 pack switch 写入口；`GET /status` 展示 pack 状态即可。
   - MCP 不新增切换写工具；可通过状态资源/只读工具查看 pack 状态，避免外部导师隐式改变数据隔离边界。

## 预计修改面

- `crates/polaris-core/src/db.rs`：新增 pack θ 表、索引和 `attempts.theta_scope` 迁移。
- `crates/polaris-core/src/pack.rs`：让 `PackData` 暴露 title。
- `crates/polaris-core/src/pack_state.rs`：新增 pack 列表、active pack、switch、theta mode 与 pack θ 辅助 API。
- `crates/polaris-core/src/mirt.rs`：按 concept pack + theta mode 选择 shared/isolated θ，记录 `theta_scope`。
- `crates/polaris-core/src/consolidation.rs`：按 attempt `theta_scope` 查 shared 或 pack 历史 θ。
- `crates/polaris-core/src/engine.rs`、`engine/task_selection.rs`、`status.rs`、`lib.rs`：导出 facade，过滤 active pack。
- `crates/polaris-cli/src/main.rs`：`pack list/switch` CLI 和 status 输出字段。
- `crates/polaris-cli/src/http.rs`、`mcp.rs`：只读状态字段测试。
- `crates/polaris-core/tests/p08a_pack_switch.rs`：核心行为与隔离 θ 测试。
- `docs/DATA_MODEL.md`：登记 P08A 新 DDL 与 `theta_scope` 语义。
- `docs/tickets/QUEUE.md` 与本票交付记录。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core --test p08a_pack_switch
cargo test -p polaris-cli pack_
cargo test -p polaris-cli status_json_serializes_current_pack
cargo test -p polaris-cli http_status_includes_current_pack
cargo test -p polaris-cli mcp_reads_status_resource_includes_current_pack
```

专项验收要求：

- `pack list` 能列出已安装 pack、当前 pack、每 pack θ 模式与概念数。
- `pack switch algorithms --theta-mode isolated` 后，`next_task` / batch / status 只返回 algorithms pack 的概念。
- 切回 `rust --theta-mode shared` 后，调度只返回 Rust pack 的概念，且 shared θ 路径保持旧行为。
- isolated 模式 final score 只更新 `pack_theta(pack='algorithms')`，不更新全局 `theta`。
- shared 模式 final score 继续更新全局 `theta`。
- `attempts.theta_scope` 对 shared/isolated 更新可审计，夜间 residual 回放不会因为 isolated attempt 找不到 shared `theta_history` 而跳过。
- 重复 `init_pack` 同一 pack 幂等；跨 pack concept id 冲突被拒绝。
- 不修改 pack 文件内容，不改冻结参考仓库。

## 禁区

- 不重构全库为 `(pack_id, concept_id)` 复合主键。
- 不给 `attempts`、`mastery_states`、`behavior_events` 全量新增 `pack_id` 字段；本票通过 `concept_id -> concepts.pack` 间接归属。
- 不实现多用户、多设备同步、多数据库分库。
- 不改 Domain Pack 协议，不新增领域特定逻辑。
- 不在 HTTP/MCP 暴露会改变 active pack 的写入口。
- 不做 P08C Pack 作者指南，不做 P10A 信任面板。
- 不修改 `C:\MyProject\Polaris` 与 `C:\MyProject\Learned`。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/`、atlas 计划等预存改动。

## 开工前复述（2026-06-17）

- 当前状态：P07E 已提交（`ecee5fb` + `c6e0742`），当前无 In Progress 票，本票按产品路线图在 P07E 后认领。
- 本轮范围：把 P08A 候选转为正式票，补 pack list/switch、active pack 过滤、真实 shared/isolated θ 路径和状态展示。
- 禁区确认：不做复合主键大迁移、不改 pack 协议、不做 HTTP/MCP 写入口、不做 P08C/P10A、不修改冻结参考仓库、不混入预存脏改动。
- 用户体验约束：切换必须可见且生效；`pack switch` 不能只是写 meta，`next/status/batch` 必须尊重当前 pack。
- 验收命令：见上方“验收”。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md docs/DATA_MODEL.md crates/polaris-core/src/db.rs crates/polaris-core/src/pack.rs crates/polaris-core/src/mirt.rs crates/polaris-core/src/consolidation.rs crates/polaris-core/src/engine.rs crates/polaris-core/src/engine/task_selection.rs crates/polaris-core/src/status.rs crates/polaris-core/src/lib.rs crates/polaris-cli/src/main.rs crates/polaris-cli/src/http.rs crates/polaris-cli/src/mcp.rs
Remove-Item crates/polaris-core/src/pack_state.rs
Remove-Item crates/polaris-core/tests/p08a_pack_switch.rs
Remove-Item docs/tickets/TICKET_P08A_PACK_SWITCHING.md
```

提交后：

```powershell
git revert <P08A-commit-sha>
```

## 交付记录（2026-06-17）

### 变更清单

- 新增 `pack_state`：`ThetaMode`、pack 列表、active pack、theta mode、switch 元数据逻辑；`active_pack` 指向未安装 pack 时显式报错，避免 status/next 静默变空。
- 扩展迁移：`attempts.theta_scope`、`pack_theta`、`pack_theta_history`、`idx_concepts_pack`；保留旧 attempt 缺失 `theta_scope` 时按 `shared` 兼容。
- `init_pack` 持久化 `pack.<id>.title`、默认 `pack.<id>.theta_mode=shared`、首次安装自动设置 `active_pack`，并拒绝跨 pack concept id 冲突；同 pack 重复 init 仍保留既有 q。
- `next_task`、`get_interleaved_batch`、`status_snapshot` 按 active pack 过滤；无 active pack 时保留全库旧行为。
- MIRT `latent_prediction` / `fused_p_known` / `update_theta_for_attempt` 按 concept 所属 pack 与 `theta_mode` 选择 shared 或 isolated θ，并写入 `attempts.theta_scope`。
- nightly consolidation 按 `attempts.theta_scope + theta_version` 回放 shared 或 pack θ 历史，isolated attempt 不再依赖 shared `theta_history`。
- CLI 新增 `polaris pack list [--json]`、`polaris pack switch <pack> [--theta-mode shared|isolated]`；`status` 文本/JSON 显示 `current_pack`、`theta_mode`、pack 列表。
- HTTP/MCP 只读 status 暴露 pack 状态；未新增 HTTP/MCP active pack 写入口。
- 更新 `docs/DATA_MODEL.md`，登记 P08A DDL 与 `theta_scope` 语义。

### 审查

- Curie（入口探索，只读）：确认 CLI 为唯一 switch 写入口，HTTP/MCP 保持只读 status，status 文本需先显示当前上下文。
- Anscombe（规格复审，只读）：复审后无 Critical / Important / Minor。
- Copernicus（代码质量复审，只读）：复审后无 Critical / Important；确认 invalid active pack 与 isolated residual replay 已补测。
- 预存票外 dirty 文件未纳入本票：`.gitignore`、`.cursor/`、`docs/visuals/`、atlas 计划、`docs/polaris-core-comic-system-brief.md` 等保持未 staged。

### 验收实跑输出

```powershell
> cargo fmt --check
exit 0
```

```powershell
> cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.34s
```

```powershell
> cargo test --workspace
test result: ok. 56 passed; 0 failed
test result: ok. 68 passed; 0 failed
...
test result: ok. 6 passed; 0 failed   # tests/p08a_pack_switch.rs
...
Doc-tests polaris_core
test result: ok. 0 passed; 0 failed
```

```powershell
> cargo test -p polaris-core --test p08a_pack_switch
running 6 tests
test init_pack_rejects_concept_id_collision_across_packs ... ok
test invalid_active_pack_meta_is_not_silent ... ok
test shared_theta_mode_preserves_global_theta_updates ... ok
test isolated_theta_updates_pack_theta_without_touching_shared_theta ... ok
test isolated_theta_attempts_replay_into_nightly_residual_stats ... ok
test pack_switch_filters_next_batch_and_status_to_active_pack ... ok
test result: ok. 6 passed; 0 failed
```

```powershell
> cargo test -p polaris-cli pack_
running 4 tests
test tests::pack_list_text_surfaces_active_pack_and_theta_mode ... ok
test tests::pack_switch_text_surfaces_resulting_context ... ok
test tests::pack_list_flags_parse ... ok
test tests::pack_switch_flags_parse ... ok
test result: ok. 4 passed; 0 failed
```

```powershell
> cargo test -p polaris-cli status_json_serializes_current_pack
running 1 test
test tests::status_json_serializes_current_pack ... ok
test result: ok. 1 passed; 0 failed
```

```powershell
> cargo test -p polaris-cli http_status_includes_current_pack
running 1 test
test http::tests::http_status_includes_current_pack ... ok
test result: ok. 1 passed; 0 failed
```

```powershell
> cargo test -p polaris-cli mcp_reads_status_resource_includes_current_pack
running 1 test
test mcp::tests::mcp_reads_status_resource_includes_current_pack ... ok
test result: ok. 1 passed; 0 failed
```

```powershell
> git diff --check -- <P08A scoped paths>
exit 0
warning: LF will be replaced by CRLF ...  # only line-ending warnings, no whitespace errors
```
