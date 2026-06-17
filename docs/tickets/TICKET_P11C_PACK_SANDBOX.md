# P11C Pack 作者沙箱模拟

状态：Done（2026-06-17，通过验收）

服务主命题环节：全环节（Pack 作者在发布前验证“能教、能跑、无污染”）

## 背景

P08C 已交付 Pack 作者上手指南和 `packs/template/`，但作者目前需要手动串起 `pack validate`、临时库 `init`、`pack switch`、`next` 才能确认一个 pack 可跑。P04E 已有端到端虚拟学习者 simulation，可验证调度、提交、final score、相图、HMM、theta 跟踪等闭环属性。

本票把这两者合成一个只写沙箱的作者入口：用临时/内存数据库验证指定 pack 的结构和最小教学闭环，不修改用户默认数据库，不引入外部 LLM，不改变 core 学习公式。

## 范围

1. 新增 CLI：
   - `polaris pack sandbox <dir> [--profile strong|weak|mixed|all] [--days N] [--json]`。
   - 默认 `profile=mixed`，默认 `days=7`，用于作者快速检查；P04E 的 30 天断言仍保留在测试中。
   - 命令不读取或写入 `--db` 指向的用户数据库；即使用户传了 `--db`，也必须明确忽略或拒绝，避免“沙箱”名不副实。

2. 新增 core/CLI 封装：
   - 复用 `pack::validate_pack_path`。
   - 在 `Connection::open_in_memory()` 上 `migrate`、`Engine::init_pack(pack)`。
   - 将 active pack 切到该 pack，theta mode 使用 `isolated`。
   - 调用 `simulation::simulate_learning_quiet`，避免 JSON/stdout 混入每日 summary。
   - 输出结构化 `SandboxReport`，至少包含：pack id/title、profile、days、status、validate counts、deadlock days、initial/final mean p_known、mean slope、final phase counts、early transfer violations、hmm lock 标记。

3. 通过标准：
   - `status=pass`：无 deadlock、无 HMM lock、最终 `mean_p_known` 高于初始值、无 early transfer violations。
   - `status=warn`：可跑完但存在弱信号，例如提升不明显或 weak learner 的校准未收敛；不得伪装为 pass。
   - `status=fail`：pack 无法 validate/init/simulate，或发生 deadlock/HMM lock/明显违反 simulation 基线。

4. 文档：
   - 更新 `docs/PACK_AUTHOR_GUIDE.md`，把当前多命令手工闭环替换或补充为 `polaris pack sandbox packs/my_course`。
   - 更新 `docs/PRODUCT_ROADMAP.md` §7，把“沙箱模式”标记为已转正式票 P11C。
   - 更新 `docs/tickets/QUEUE.md`，确保本票是唯一 In Progress。

## 预计修改面

- `crates/polaris-core/src/sandbox.rs`：新增沙箱报告结构、learner 选择、pass/warn/fail 判定和内存库运行入口。
- `crates/polaris-core/src/lib.rs`：导出 `sandbox` 模块。
- `crates/polaris-cli/src/main.rs`：新增 `pack sandbox` 命令、文本/JSON 输出和解析测试。
- `crates/polaris-core/tests/p11c_pack_sandbox.rs`：覆盖内存沙箱运行、无真实 DB 污染、invalid pack fail、strong/mixed learner 输出。
- `docs/PACK_AUTHOR_GUIDE.md`、`docs/PRODUCT_ROADMAP.md`、`docs/tickets/QUEUE.md`、本票。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p11c_pack_sandbox
cargo test -p polaris-cli sandbox
cargo run -p polaris-cli -- pack sandbox packs/template --days 7
cargo run -p polaris-cli -- pack sandbox packs/template --profile strong --days 7 --json
git diff --check
```

专项验收要求：

- `pack sandbox` 不创建、读取或修改默认 `polaris.sqlite`。
- `--db <path> pack sandbox ...` 不得写入该 path；若实现选择拒绝 `--db`，错误信息必须明确说明 sandbox 只使用内存库。
- `POLARIS_LLM_*`、`POLARIS_EMBED_*` 存在时也不得发生外部调用；沙箱运行期间强制 `POLARIS_TIER0_ONLY=1` 并复用 simulation 的环境隔离。
- invalid pack 返回 fail/错误，不留下持久文件。
- JSON 输出可由 CI 消费，字段稳定且不是二次编码字符串。

## 禁区

- 不新增数据库 schema、不提升 `CURRENT_SCHEMA_VERSION`。
- 不修改 P04E simulation 的核心学习公式或已有断言阈值，除非为沙箱新增只读封装字段。
- 不在沙箱中调用 LLM、embedding、HTTP、MCP 或外部命令。
- 不把沙箱结果写入 `mastery_states`、`param_tuning_runs`、`consolidation_runs` 或用户真实库。
- 不新增 pack 协议字段，不修改 `packs/template/` 内容。
- 不实现跨设备、多用户、数据导出或 UI。
- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 开工前复述（2026-06-17）

- 当前状态：P06J 已提交（`5dceff7`），票据状态修正已提交（`1d04fe0`），QUEUE 无正式未完成票；本票按 `PRODUCT_ROADMAP.md` §7 的“沙箱模式”候选转正式票。
- 现有脏文件不属于本票：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/jieshou.txt`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`、`target_codex_reviewNndQmJ/`。不得回退或混入。
- 预计实现策略：先写 core 沙箱测试和 CLI 解析/输出测试，确认失败；再实现 `sandbox` 模块和 `pack sandbox` CLI；最后跑专项命令和 SPEC §6 基线。

## 回滚方式

未提交前：

```powershell
git restore crates/polaris-core/src/lib.rs crates/polaris-cli/src/main.rs docs/PACK_AUTHOR_GUIDE.md docs/PRODUCT_ROADMAP.md docs/tickets/QUEUE.md
Remove-Item crates/polaris-core/src/sandbox.rs
Remove-Item crates/polaris-core/tests/p11c_pack_sandbox.rs
Remove-Item docs/tickets/TICKET_P11C_PACK_SANDBOX.md
```

提交后：

```powershell
git revert <P11C-commit-sha>
```

## 交付记录（2026-06-17）

### 变更清单

- 新增 `polaris_core::sandbox`：用内存 SQLite 执行 `validate -> init_pack -> switch_pack(isolated) -> virtual learner simulation`，输出 `SandboxReport`。
- 新增 `polaris pack sandbox <dir> [--profile strong|weak|mixed|all] [--days N] [--json]`；`--db` 会被明确拒绝，不创建用户指定数据库。
- 为 sandbox 与 simulation 合并外部模型环境变量锁；sandbox 期间强制 `POLARIS_TIER0_ONLY=1`，清除 LLM/embedding env，并在退出后恢复原值。
- 新增 P11C core/CLI 测试：沙箱闭环、strong profile、invalid pack、0 day 参数、环境恢复、`--db` 拒绝、JSON `profile` 契约。
- 更新 Pack 作者指南，把默认发布前闭环改为 `pack sandbox`，保留临时库手动检查作为可选路径。
- 更新 `docs/PRODUCT_ROADMAP.md` 与 `docs/tickets/QUEUE.md` 状态。

### 子 agent 审查

- 规格审查（Confucius）：初审指出 JSON 字段应为 `profile`、票外脏文件不得混入、票尾需补验收输出；已修复 JSON 契约，票外文件通过 staging discipline 排除，验收输出见下。
- 代码质量审查（Herschel）：初审指出 sandbox 与 simulation 使用两把 env lock 有并发恢复风险；已改为单一 `MODEL_ENV_LOCK`，复核通过。

### 验收输出

```powershell
> cargo fmt --check
# exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.22s

> cargo test --workspace
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 80 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

> cargo test -p polaris-core --test p11c_pack_sandbox
running 5 tests
test invalid_pack_fails_before_simulation ... ok
test strong_learner_sandbox_passes_template_pack ... ok
test sandbox_restores_external_model_environment ... ok
test sandbox_rejects_zero_day_runs ... ok
test template_pack_sandbox_runs_without_deadlock ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

> cargo test -p polaris-cli sandbox
running 3 tests
test tests::pack_sandbox_flags_parse ... ok
test tests::pack_sandbox_rejects_user_database_path ... ok
test tests::pack_sandbox_json_uses_profile_contract_field ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 72 filtered out; finished in 0.14s

> cargo run -p polaris-cli -- pack sandbox packs/template --days 7
sandbox status=pass
pack=template title=Template Course Pack
profile=mixed days=7 theta_mode=isolated
mode=sandbox writes_user_db=false tier0_only=true llm_used=false score_source=virtual_learner
validation: concepts=5 prerequisites=4 misconceptions=3
mean_p_known: 0.184 -> 0.562 slope=0.018
calibration_gap: 0.093 -> 0.071 theta_cosine=0.000
deadlock_days=[] hmm_state_lock=false early_transfer_violations=0
final_phase_counts={"counts":{"transfer":2,"undetermined":3}}
note=virtual learner simulation; not a real learner mastery estimate
summary=sandbox closed loop improved without deadlock

> cargo run -p polaris-cli -- pack sandbox packs/template --profile strong --days 7 --json
{
  "mode": "sandbox",
  "writes_user_db": false,
  "tier0_only": true,
  "llm_used": false,
  "score_source": "virtual_learner",
  "pack_id": "template",
  "pack_title": "Template Course Pack",
  "profile": "strong",
  "days": 7,
  "status": "pass",
  "theta_mode": "isolated",
  "validation": {
    "concept_count": 5,
    "prerequisite_count": 4,
    "misconception_count": 3
  },
  "deadlock_days": [],
  "early_transfer_violations": [],
  "hmm_state_lock": false,
  "summary": "sandbox closed loop improved without deadlock"
}

> git diff --check
# exit 0；仅有 Git 的 LF/CRLF 工作区提示，无 whitespace error。
```

### 票外改动处理

工作区仍存在本票开工前已有的无关改动：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/jieshou.txt`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`、`target_codex_reviewNndQmJ/`。本票提交时只 stage P11C 相关文件，不回退、不混入。
