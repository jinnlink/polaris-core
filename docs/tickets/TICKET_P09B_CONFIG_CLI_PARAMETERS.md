# P09B polaris config 浏览 CLI + 参数文档自动生成

状态：已完成

服务主命题环节：全环节（参数治理 / 可审计性）

## 背景

P03J 已经把参数登记处作为系统“参数认识论”的核心结构：每个参数都有默认值、类别（A/B/C）、边界和调优途径。当前这些信息只在 `crates/polaris-core/src/config.rs` 的 `default_registry()` 中可见，开发者能读代码，用户、Pack 作者和运维无法通过 CLI 或文档快速浏览。

P09B 只做只读可见性：新增 `polaris config list`，并生成/校验 `docs/PARAMETERS.md`，让参数治理面可查。它不修改任何参数值、公式、调用点或调优逻辑。

## 范围

1. Core 参数展示结构：
   - 复用现有 `default_registry()` / `ParameterSpec`。
   - 为 `ParameterClass` 与 `TuningRoute` 增加稳定字符串输出（例如 `A/B/C`、`Replay/Mrt/Manual/Fit`）。
   - 如有必要，新增只读排序/过滤 helper；不得改变 registry 内容。

2. CLI：
   - 新增命令：

```powershell
polaris config list [--class A|B|C] [--tuning-route Replay|Mrt|Manual|Fit] [--json|--md]
```

   - 默认文本输出按 key 字典序列出：
     - key
     - default_value
     - class
     - bounds
     - tuning_route
   - `--json` 输出结构化数组，字段名与 `ParameterSpec` 含义一致。
   - `--md` 输出 Markdown 表格，用于生成 `docs/PARAMETERS.md`。
   - `--json` 与 `--md` 互斥。
   - 过滤条件可叠加；无匹配时输出空列表但 exit 0。

3. 参数文档：
   - 新增 `docs/PARAMETERS.md`，内容从 registry 同源生成，至少包含：
     - key
     - 默认值
     - 类别
     - 边界
     - 调优途径
   - 增加同步测试，确保 `docs/PARAMETERS.md` 中的参数 key 集合与 `default_registry()` 精确一致。
   - 产品/架构审查采纳方案 A：新增 deterministic 生成 helper + 同步测试，不让 build.rs 在普通 build/test 时改工作树。
   - 同步测试失败信息必须包含明确修复指令：

```text
参数文档与 registry 不同步。修复请跑：
  cargo run -p polaris-cli -- config list --md > docs/PARAMETERS.md
```

4. 测试：
   - core 单测覆盖 `ParameterClass` / `TuningRoute` 字符串输出与过滤 helper。
   - CLI 单测覆盖：
     - `polaris config list` 解析。
     - `--class` 过滤。
     - `--tuning-route` 过滤。
     - `--json` 输出可被解析且包含 `bkt.p_init` 等哨兵参数。
   - 文档同步测试覆盖 `docs/PARAMETERS.md` 与 registry key 集合一致。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项测试：

```powershell
cargo test -p polaris-core config
cargo test -p polaris-cli config
```

专项手工命令：

```powershell
cargo run -p polaris-cli -- config list
cargo run -p polaris-cli -- config list --class A
cargo run -p polaris-cli -- config list --tuning-route Replay --json
cargo run -p polaris-cli -- config list --md
```

专项验收要求：

- `default_registry()` 参数数量和既有 key 不因本票改变。
- `polaris config list` 默认输出按 key 字典序稳定。
- `--class` 只接受 `A/B/C`。
- `--tuning-route` 只接受 `Replay/Mrt/Manual/Fit`。
- `docs/PARAMETERS.md` 与 registry key 集合同步测试强制一致。

## 禁区

- 不修改任何参数默认值、边界、类别或调优途径。
- 不修改参数读取路径、调优逻辑、公式、DDL、MRT、breeding、report、phase 或 scheduler 行为。
- 不新增 config set / edit / mutate 能力；本票只读。
- 不修改冻结参考仓库。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/` 等预存改动。

## 本轮范围（2026-06-15）

- 当前状态：P07A 已提交（`58b82f1`）。
- 已有非本票改动：`.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`。本票不得回退或混入这些改动。
- 产品/架构审查结果：采纳 helper + 同步测试方案；新增 `--md` 输出；审查通过，进入实现。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md crates/polaris-core/src/config.rs crates/polaris-cli/src/main.rs
Remove-Item docs/tickets/TICKET_P09B_CONFIG_CLI_PARAMETERS.md
Remove-Item docs/PARAMETERS.md
```

提交后：

```powershell
git revert <P09B-commit-sha>
```

## 交付记录（2026-06-15）

### 变更清单

- `crates/polaris-core/src/config.rs`：
  - `ParameterClass` / `TuningRoute` 增加稳定字符串输出与解析。
  - `ParameterSpec` 支持序列化。
  - 新增 `parameter_specs()` 过滤 helper。
  - 新增 `parameters_markdown()`，供 CLI `--md` 与文档同步测试同源使用。
  - 新增 `params_doc_keys_match_registry`，失败信息包含修复命令。
- `crates/polaris-cli/src/main.rs`：
  - 新增 `polaris config list [--class A|B|C] [--tuning-route Replay|Mrt|Manual|Fit] [--json|--md]`。
  - 默认文本输出、JSON 输出、Markdown 输出共用同一 specs 列表。
  - `--json` 与 `--md` 互斥。
- `docs/PARAMETERS.md`：
  - 由 `cargo run -p polaris-cli -- config list --md` 从 registry 生成。
- 文档：
  - `QUEUE.md` 标记 P09B In Progress。

### TDD 红灯记录

```text
cargo test -p polaris-core config
error[E0599]: no method named `as_str` found for enum `config::ParameterClass`
error[E0425]: cannot find function `parameter_specs` in this scope
error[E0425]: cannot find function `parameters_markdown` in this scope
exit 101
```

```text
cargo test -p polaris-cli config
error[E0599]: no variant named `Config` found for enum `Commands`
error[E0425]: cannot find function `config_list_text` in this scope
error[E0425]: cannot find function `config_list_json` in this scope
error[E0425]: cannot find function `config_list_markdown` in this scope
exit 101
```

```text
cargo test -p polaris-core config
test config::tests::params_doc_keys_match_registry ... FAILED
left: {"bkt.p_init"}
right: {registry key set}
参数文档与 registry 不同步。修复请跑：
  cargo run -p polaris-cli -- config list --md > docs/PARAMETERS.md
exit 101
```

### 验收输出

```text
cargo test -p polaris-core config
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 57 filtered out
exit 0
```

```text
cargo test -p polaris-cli config
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out
exit 0
```

```text
cargo run -p polaris-cli -- config list
输出表头：key	default	class	bounds	tuning_route
包含哨兵：bkt.p_init	0.20	B	[0.05,0.50]	Replay
exit 0
```

```text
cargo run -p polaris-cli -- config list --class A
输出仅包含 class=A 参数，例如 breeding.admit_p / grade.quote_min / latent.k_max
exit 0
```

```text
cargo run -p polaris-cli -- config list --tuning-route Replay --json
输出 JSON 数组，条目包含 "key" / "default_value" / "class" / "bounds" / "tuning_route"
exit 0
```

```text
cargo run -p polaris-cli -- config list --md
输出 Markdown，以 "# Polaris Parameter Registry" 开头，并包含完整参数表
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.43s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 34 passed
polaris-core unit: 67 passed
all existing integration tests and doc-tests passed
exit 0
```

```text
git diff --check
exit 0
仅有 LF/CRLF 警告，无 whitespace 错误。
```

### 技术选择说明

- 采用产品经理审查通过的方案 A：helper + 同步测试，不用 build.rs 自动改工作树。
- `docs/PARAMETERS.md` 标题使用 ASCII，避免 Windows PowerShell 捕获 stdout 生成文档时出现终端编码乱码。
- CLI 的 text/json/md 三种输出均来自同一 `parameter_specs()`，避免输出分叉。
- 本票只读展示参数，不改 registry 中任何参数 spec。

### 待审事项

- 产品/架构审查结果：ship it。
