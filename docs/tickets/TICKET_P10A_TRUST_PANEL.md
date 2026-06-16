# P10A 五框架门状态面板 + 实验透明度

状态：已提交
服务主命题环节：验证真懂（验证门可见）

## 背景

SPEC §3 要求“不过验证门 = 假设，不得进默认行为”。P03I、P04C、P05B 已分别实现镜像报告、MRT 预登记与教法育种，但用户目前无法一次性看到 F1-F5 哪些框架已经过门、哪些仍只是候选假设，也无法查看当前正在运行的 breeding/MRT 实验。

本票做一个只读信任出口：把已经分散在表和配置里的门状态、实验透明度和最近后台摘要聚合出来。它回答“系统对我做了什么、哪些判断有门、哪些还只是实验”，不改变任何调度、评分、报告或育种行为。

## 范围

1. Core 只读聚合：
   - 新增 `trust` 相关结构与只读查询入口，建议放在 `crates/polaris-core/src/trust.rs`。
   - 输出 `TrustPanel`，包含：
     - `gates`：F1-F5 五框架门状态。
     - `active_breeding_experiments`：当前 `bred_moves.status='preregistered'` 的候选实验。
     - `active_mrt_experiments`：最近窗口内 `mrt_log` 的预登记记录。
     - `recent_activity`：最近一次 `mental_dynamics_fit`、`param_tuning`、`nightly_consolidation` 摘要。
     - `governance`：至少包含 `breeding.min_n` 当前值、默认值和是否为治理门槛。
   - 所有查询只读，不写库、不触发后台 job、不调用 LLM。

2. 五框架门状态：
   - F1 教法签名：基于 `moves_effects` / `mrt_log` 是否有可审计样本，展示 fitted/unfit、样本数和说明。
   - F2 相图判据：基于镜像报告/相图已有能力展示 available，并说明这是 Tier 0 判据层；若无样本则不伪造 AUC。
   - F3 摩擦曲线：基于 `moves_effects` / `mrt_log` 是否存在摩擦上下文样本，展示 fitted/unfit。
   - F4 G_u 误解语法：基于 `gu_rules` 活跃/验证规则统计，展示 fitted/unfit、active/validated 数。
   - F5 育种：基于 `bred_moves`、`breeding.admit_p`、`breeding.retire_p`、`breeding.min_n` 展示门槛、候选数、已准入数、已退役数。
   - 允许无数据时明确输出 `unfit` / `no_data`，不得把缺数据说成通过。

3. CLI：
   - 新增：
```powershell
polaris trust show [--json]
```
   - 文本输出必须直读、稳定，包含五框架门、active breeding、active MRT、recent activity、governance。
   - `--json` 输出结构化 `TrustPanel`，字段名稳定，便于 UI/HTTP/MCP 消费。

4. HTTP/MCP 只读出口：
   - HTTP 新增最小只读 endpoint，建议 `GET /trust`。
   - MCP 新增只读工具或资源，复用同一个 core `TrustPanel`，不另写一套业务逻辑。
   - 仅做读取，不新增写入口。

5. 治理参数默认值：
   - `breeding.min_n` 默认值从 `6` 提到 `20`。
   - 同步 `docs/DATA_MODEL.md`、`docs/PARAMETERS.md` 与相关测试期望。
   - 既有测试如需小样本，必须在测试内显式写入 meta 覆盖，不能依赖默认 6。

## 验收

必须通过：
```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：
```powershell
cargo test -p polaris-core trust
cargo test -p polaris-cli trust
cargo test -p polaris-core --test p05b_breeding
```

专项命令：
```powershell
cargo run -p polaris-cli -- --db target/p10a-trust.sqlite init --pack packs/rust
cargo run -p polaris-cli -- --db target/p10a-trust.sqlite trust show
cargo run -p polaris-cli -- --db target/p10a-trust.sqlite trust show --json
```

验收要求：
- 空库或新初始化库下，`trust show` 不报错；缺数据项明确显示 `unfit` / `no_data`。
- JSON 顶层至少包含 `gates`、`active_breeding_experiments`、`active_mrt_experiments`、`recent_activity`、`governance`。
- 文本输出能看出 F1-F5 每个门的状态与原因。
- active breeding 输出候选 vs 在位者、posterior win probability、样本数、admit/retire/min_n 门槛。
- active MRT 输出 move、randomized、prereg_id、context hash 与主效应假设摘要。
- HTTP/MCP 只读出口与 CLI JSON 复用同一 core 结构。
- `breeding.min_n` 默认值在 config、参数文档、DATA_MODEL 中一致为 20。
- 本票不触发 LLM，不写入除命令必要初始化以外的实验/报告/诊断数据。

## 禁区

- 不改变 `next_task`、MRT、breeding、report、tuning、mental fit 的生成或评估逻辑。
- 不把未过门的框架接入默认行为或产品话术。
- 不新增 DDL，除非现有表无法只读表达；若必须新增需先记录阻塞点并请用户裁决。
- 不做 Tauri/UI 大面板；本票只做 CLI + HTTP/MCP 数据出口。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/` 等预存脏改动。

## 本轮范围（2026-06-17）

- 当前状态：P08C 已提交（`198121b`），QUEUE 无 In Progress，本票按产品路线图最后剩余 P10A 转正式票并认领。
- 已有非本票改动：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`target_codex_reviewNndQmJ/`。本票不得回退或混入。
- 预计修改面：`crates/polaris-core/src/trust.rs`、`lib.rs`、`config.rs`、`crates/polaris-cli/src/main.rs`、HTTP/MCP 入口、相关测试与文档。

## 回滚方式

未提交前：
```powershell
git restore docs/tickets/QUEUE.md docs/DATA_MODEL.md docs/PARAMETERS.md crates/polaris-core/src/lib.rs crates/polaris-core/src/config.rs crates/polaris-cli/src/main.rs
Remove-Item docs/tickets/TICKET_P10A_TRUST_PANEL.md
```

提交后：
```powershell
git revert <P10A-commit-sha>
```

## 交付记录（2026-06-17）

状态：实现完成，审查反馈已修复。

### 变更清单

- 新增 `crates/polaris-core/src/trust.rs`，提供只读 `TrustPanel` 聚合：F1-F5 门状态、active breeding、active MRT、recent activity、governance。
- `Engine::trust_panel()` 暴露同一 core 结构；CLI/HTTP/MCP 均复用该结构。
- 新增 `polaris trust show [--json]`。
- 新增 HTTP `GET /trust`；`POST /trust` 返回 405。
- 新增 MCP `get_trust_panel` 工具与 `polaris://trust` 资源。
- `breeding.min_n` 默认值从 `6` 提升到 `20`；同步 `docs/DATA_MODEL.md`、`docs/PARAMETERS.md` 与 P05B 测试。
- 新增 core/CLI/HTTP/MCP 覆盖测试；旧低样本 breeding 场景测试显式写入 `breeding.min_n=8`，不再依赖默认值。
- 审查修复：
  - `governance` 参数新增稳定字段 `is_governance_gate`，CLI 文本同步输出 `governance_gate=true/false`。
  - active MRT 只认教学 MRT 预登记：`context_json.kind='preregistration'`；F5 breeding 审计行不再误列入 active MRT / F1。
  - 只有教学 MRT 日志、没有 `moves_effects` 样本时，F1/F3 显示 `running`，不再显示 `fitted`。
  - `/trust` 的所有非 GET 方法统一返回 405。

### TDD 红灯

```powershell
cargo test -p polaris-core trust_panel
error[E0432]: unresolved import `polaris_core::trust`
 --> crates\polaris-core\tests\p10a_trust.rs:2:19
  |
2 | use polaris_core::trust::trust_panel;
  |                   ^^^^^ could not find `trust` in `polaris_core`
error: could not compile `polaris-core` (test "p10a_trust") due to 1 previous error
```

```powershell
cargo test -p polaris-cli trust_show_json_flag_parses
error[E0599]: no variant named `Trust` found for enum `Commands`
error[E0433]: cannot find type `TrustCommands` in this scope
error: could not compile `polaris-cli` (bin "polaris" test) due to 2 previous errors
```

### 验收输出

```powershell
cargo test -p polaris-core trust
test trust_panel_reports_empty_state_without_fake_passes ... ok
test trust_panel_excludes_breeding_preregistration_audit_from_active_mrt_and_f1_fit ... ok
test trust_panel_marks_teaching_mrt_preregistration_without_effect_samples_as_running ... ok
test trust_panel_surfaces_active_experiments_and_recent_activity ... ok
test result: ok. 4 passed; 0 failed
```

```powershell
cargo test -p polaris-cli trust
test tests::trust_show_json_flag_parses ... ok
test http::tests::http_trust_rejects_non_get_methods_as_method_not_allowed ... ok
test http::tests::http_trust_returns_stable_panel_shape ... ok
test mcp::tests::mcp_get_trust_panel_tool_and_resource_share_shape ... ok
test result: ok. 6 passed; 0 failed
```

```powershell
cargo test -p polaris-core --test p05b_breeding
running 5 tests
test breeding_parameters_are_governance_gates ... ok
test preregistration_writes_audit_and_keeps_candidate_out_of_admitted_library ... ok
test admission_uses_frozen_preregistration_gates_not_current_meta ... ok
test candidate_admits_only_after_posterior_beats_incumbent_with_minimum_n ... ok
test admitted_move_retires_when_effect_decays_below_incumbent ... ok
test result: ok. 5 passed; 0 failed
```

```powershell
cargo run -p polaris-cli -- --db target/p10a-trust.sqlite init --pack packs/rust
initialized
```

```powershell
cargo run -p polaris-cli -- --db target/p10a-trust.sqlite trust show
generated_at=2026-06-16T18:37:27Z
window_days=7
current_pack=rust

gates
F1	pedagogy_signature	status=unfit	gate=no_data	metric=-	reason=no moves_effects or MRT preregistrations yet
F2	phase_diagram	status=available	gate=deterministic_rule	metric=concepts=24	reason=Tier 0 phase classification is deterministic and visible; no validation AUC is fabricated
F3	friction_curve	status=unfit	gate=no_data	metric=-	reason=no friction-context move effects or signature MRT rows yet
F4	g_u_rules	status=unfit	gate=no_data	metric=-	reason=no candidate, validated, or active G_u rules yet
F5	breeding	status=unfit	gate=no_data	metric=-	reason=no bred move preregistrations yet

governance
breeding.admit_p	current=0.80	default=0.80	class=A	bounds=[0.5,0.99]	tuning_route=Manual	governance_gate=true
breeding.retire_p	current=0.50	default=0.50	class=A	bounds=[0.01,0.80]	tuning_route=Manual	governance_gate=true
breeding.min_n	current=20	default=20	class=A	bounds=[2,1000]	tuning_route=Manual	governance_gate=true
```

```powershell
cargo run -p polaris-cli -- --db target/p10a-trust.sqlite trust show --json
{
  "generated_at": "2026-06-16T18:37:27Z",
  "window_days": 7,
  "gates": [
    {
      "framework": "F1",
      "name": "pedagogy_signature",
      "status": "unfit",
      "gate": "no_data",
      "metric": null,
      "reason": "no moves_effects or MRT preregistrations yet"
    },
    {
      "framework": "F2",
      "name": "phase_diagram",
      "status": "available",
      "gate": "deterministic_rule",
      "metric": "concepts=24",
      "reason": "Tier 0 phase classification is deterministic and visible; no validation AUC is fabricated"
    },
    {
      "framework": "F3",
      "name": "friction_curve",
      "status": "unfit",
      "gate": "no_data",
      "metric": null,
      "reason": "no friction-context move effects or signature MRT rows yet"
    },
    {
      "framework": "F4",
      "name": "g_u_rules",
      "status": "unfit",
      "gate": "no_data",
      "metric": null,
      "reason": "no candidate, validated, or active G_u rules yet"
    },
    {
      "framework": "F5",
      "name": "breeding",
      "status": "unfit",
      "gate": "no_data",
      "metric": null,
      "reason": "no bred move preregistrations yet"
    }
  ],
  "active_breeding_experiments": [],
  "active_mrt_experiments": [],
  "recent_activity": {
    "window_days": 7,
    "param_tuning_runs": { "count_7d": 0, "last_at": null, "last_status": null },
    "breeding_evaluated_7d": { "count_7d": 0, "last_at": null, "last_status": null },
    "breeding_admitted_7d": { "count_7d": 0, "last_at": null, "last_status": null },
    "breeding_retired_7d": { "count_7d": 0, "last_at": null, "last_status": null },
    "mental_fit_hazard": { "count_7d": 0, "last_at": null, "last_status": null },
    "mental_fit_state_gate": { "count_7d": 0, "last_at": null, "last_status": null },
    "gu_inductions": { "count_7d": 0, "last_at": null, "last_status": null },
    "nightly_consolidation": { "count_7d": 0, "last_at": null, "last_status": null },
    "mirror_reports": { "count_7d": 0, "last_at": null, "last_status": null }
  },
  "governance": {
    "current_pack_id": "rust",
    "breeding_admit_p": {
      "key": "breeding.admit_p",
      "current_value": "0.80",
      "default_value": "0.80",
      "class": "A",
      "bounds": "[0.5,0.99]",
      "tuning_route": "Manual",
      "is_governance_gate": true
    },
    "breeding_retire_p": {
      "key": "breeding.retire_p",
      "current_value": "0.50",
      "default_value": "0.50",
      "class": "A",
      "bounds": "[0.01,0.80]",
      "tuning_route": "Manual",
      "is_governance_gate": true
    },
    "breeding_min_n": {
      "key": "breeding.min_n",
      "current_value": "20",
      "default_value": "20",
      "class": "A",
      "bounds": "[2,1000]",
      "tuning_route": "Manual",
      "is_governance_gate": true
    }
  }
}
```

### SPEC §6 基线

```powershell
cargo fmt --check
# 退出码 0，无输出
```

```powershell
cargo clippy --workspace --all-targets -- -D warnings
    Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
    Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.51s
```

说明：同一 clippy 命令在默认沙箱内先因 `target` 构建产物覆盖/删除权限报 `Access denied (os error 5)`；未改代码，按权限规则提权重跑同一命令后通过。

```powershell
cargo test --workspace
test result: ok. 60 passed; 0 failed
test result: ok. 69 passed; 0 failed
...
test trust_panel_excludes_breeding_preregistration_audit_from_active_mrt_and_f1_fit ... ok
test trust_panel_reports_empty_state_without_fake_passes ... ok
test trust_panel_marks_teaching_mrt_preregistration_without_effect_samples_as_running ... ok
test trust_panel_surfaces_active_experiments_and_recent_activity ... ok
test result: ok. 4 passed; 0 failed
Doc-tests polaris_core
test result: ok. 0 passed; 0 failed
```

### 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md docs/DATA_MODEL.md docs/PARAMETERS.md crates/polaris-core/src/lib.rs crates/polaris-core/src/config.rs crates/polaris-core/src/engine.rs crates/polaris-core/tests/p05b_breeding.rs crates/polaris-cli/src/main.rs crates/polaris-cli/src/http.rs crates/polaris-cli/src/mcp.rs
Remove-Item crates/polaris-core/src/trust.rs crates/polaris-core/tests/p10a_trust.rs docs/tickets/TICKET_P10A_TRUST_PANEL.md
```

提交后：

```powershell
git revert <P10A-commit-sha>
```
