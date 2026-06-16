# P08C Pack 作者上手指南 + 模板 Pack

状态：已完成
服务主命题环节：生态可用性（验证真懂 → 定位模糊 → 针对性补缺的 pack 入口）

## 背景

P05A0 已经定义 Course Integration Protocol，P08A 已经让多 pack 切换和 θ 共享/隔离可用。但外部课程作者现在仍缺一条可复制的最短路径：从空目录创建一个 pack、通过 validator、初始化到本地数据库、切换为 active pack，并确认调度只从该 pack 选题。

本票补“能照着做”的作者入口：一份 30 分钟上手指南和一个最小但不玩具化的模板 pack。目标不是再设计协议，而是把已有协议、CLI 和 validator 串成可靠闭环。

## 范围

1. 新增模板 pack：
   - 路径：`packs/template/`。
   - 必须包含 `pack.toml`、`concepts.toml`、`misconceptions.toml`、`rubric.md`、`moves.toml`。
   - 可选包含 `ingest.toml`，但必须明确它当前只是协议预留，validator 不强制、core 不直接消费。
   - 模板为 5 个概念左右，覆盖：
     - 至少 1 个 `schema` 概念。
     - 至少 4 个普通 `concept`。
     - 至少 3 条 `prerequisite` 或 `component_of` 边。
     - 至少 2 条常见误解，并使用 DATA_MODEL §9 的 pattern 枚举。
     - 7 个 move 模板：`recall`、`explain`、`apply`、`analyze`、`evaluate`、`create`、`transfer`。
   - 内容必须是领域中性的作者模板，不引入 Rust、英语、考试等领域逻辑进 core。
2. 新增作者指南：
   - 路径：`docs/PACK_AUTHOR_GUIDE.md`。
   - 面向“有一门课/一组知识点，想接入 Polaris Core”的作者。
   - 必须覆盖 30 分钟路径：
     1. 复制模板 pack。
     2. 修改 `pack.toml` 的 `id/title/version/lang`。
     3. 改 5 个 concept 和 seed_order。
     4. 填 prerequisite/confusion/component_of。
     5. 写 misconceptions、rubric、moves。
     6. 运行 `cargo run -p polaris-cli -- pack validate packs/template`。
     7. 用临时库运行 `init`、`pack list`、`pack switch`、`next`，确认 active pack 生效。
   - 必须包含常见 validator 报错和修复方式，且与当前 `pack.rs` 行为一致。
   - 必须说明模板 pack 与 `docs/COURSE_INTEGRATION_PROTOCOL.md` 的关系：指南负责上手，协议负责完整契约。
3. 自动化约束：
   - 增加测试，保证 `packs/template` 能被当前 validator 接受。
   - 增加 CLI 或 core 层测试，至少覆盖 `pack validate packs/template` 的有效性。
   - 不新增运行时行为，不改调度、评分、MIRT、MCP/HTTP。

## 验收

必须通过：
```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

专项验收：
```powershell
cargo run -p polaris-cli -- pack validate packs/template
cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite init --pack packs/template
cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite pack list
cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite pack switch template --theta-mode isolated
cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite next
```

专项验收要求：
- `pack validate packs/template` 输出 `pack ok`，且 concept 数约为 5。
- `init --pack packs/template` 成功初始化，不与现有 pack 概念 ID 冲突。
- `pack list` 能看到 `template`，并显示 active pack。
- `pack switch template --theta-mode isolated` 成功，输出 theta mode 为 `isolated`。
- `next` 返回的任务 concept 必须来自 `template` pack。
- `docs/PACK_AUTHOR_GUIDE.md` 中的命令可以按顺序执行；若命令需要临时 DB，必须显式使用 `--db target/...sqlite`，避免污染用户默认库。
- `git diff --check` 无 whitespace error。

## 禁区

- 不修改冻结参考仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不把模板 pack 写成某个真实领域的教学逻辑；它只能是作者可替换的中性样板。
- 不放宽当前 validator，也不修改 pack schema 语义。
- 不新增 CLI 子命令；只复用现有 `pack validate`、`init`、`pack list`、`pack switch`、`next`。
- 不修改调度、评分、MIRT、θ、MCP/HTTP 行为。
- 不混入 `.gitignore`、`.cursor/`、`docs/visuals/` 等当前已有无关改动。

## 本轮范围（2026-06-17）

- 当前状态：P08A 已提交，QUEUE 无 In Progress。
- 本票认领 P08C，范围限定为模板 pack、作者指南、模板 pack 验证测试与票据/队列记录。
- 已知非本票改动：`.gitignore`、`docs/polaris-core-comic-system-brief.md`、`.cursor/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`、`docs/visuals/`、`docs/visuals/polaris-core-architecture.html`、`docs/visuals/polaris-core-architecture.svg`、`target_codex_reviewNndQmJ/`。本票不得回退或混入这些改动。

## 回滚方式

未提交前：
```powershell
git restore docs/tickets/QUEUE.md
Remove-Item -Recurse -Force packs/template, docs/PACK_AUTHOR_GUIDE.md, docs/tickets/TICKET_P08C_PACK_AUTHOR_GUIDE_TEMPLATE.md
```

提交后：
```powershell
git revert <P08C-commit-sha>
```

## 交付记录（2026-06-17）

### 变更清单

- 新增 `packs/template/` 模板 pack：
  - `pack.toml`
  - `concepts.toml`
  - `misconceptions.toml`
  - `rubric.md`
  - `moves.toml`
  - `ingest.toml`
- 新增 `docs/PACK_AUTHOR_GUIDE.md`：
  - 30 分钟上手路径。
  - 明确 `init --pack` 真实 CLI 语法。
  - 明确复制模板后必须替换所有 `template_*` concept ID，并同步 edge 与 misconception 引用。
  - 记录 validator 当前检查项、未检查项和常见报错。
- `crates/polaris-core/src/pack.rs` 新增模板 pack validator 测试。
- `docs/tickets/QUEUE.md` 认领并收口 P08C。

### TDD 红灯

```text
cargo test -p polaris-core pack::tests::validates_template_pack_shape
test pack::tests::validates_template_pack_shape ... FAILED
called `Result::unwrap()` on an `Err` value: MissingFile("pack.toml")
test result: FAILED. 0 passed; 1 failed
exit 1
```

### 审查记录

- 建设向调研 agent Harvey：确认 loader/validator 文件形状、`init --pack` 语法、`packs/template/` 放置位置和验收命令。
- 审查向 agent Noether：
  - Important：作者指南缺少 `template_*` concept ID 替换与引用同步步骤。已修复。
  - Important：票尾缺交付记录。已补。
  - Minor：模板测试未锁住至少 3 条结构边。已把 prerequisite 断言提高到 `>= 3`。

### 验收输出

`cargo fmt --check`

```text
exit 0
```

`cargo clippy --workspace --all-targets -- -D warnings`

```text
默认 target 首次仍遇到 Windows 文件锁：
error: failed to write ... libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: could not compile `polaris-core` (lib) due to 1 previous error
```

同一条命令按权限规则提权复跑：

```text
Checking polaris-core v0.1.0 (C:\MyProject\polaris-core\crates\polaris-core)
Checking polaris-cli v0.1.0 (C:\MyProject\polaris-core\crates\polaris-cli)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.68s
exit 0
```

`cargo test --workspace`

```text
polaris-cli unit: test result: ok. 56 passed; 0 failed
polaris-core unit: test result: ok. 69 passed; 0 failed
pack::tests::validates_template_pack_shape ... ok
all integration suites and doc-tests passed
exit 0
```

`cargo test -p polaris-core pack::tests::validates_template_pack_shape`

```text
test pack::tests::validates_template_pack_shape ... ok
test result: ok. 1 passed; 0 failed
exit 0
```

`cargo test -p polaris-cli pack_`

```text
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 52 filtered out
exit 0
```

`cargo run -p polaris-cli -- pack validate packs/template`

```text
pack ok: concepts=5 prerequisites=4 misconceptions=3
exit 0
```

`cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite init --pack packs/template`

```text
initialized
exit 0
```

`cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite pack list`

```text
* template	Template Course Pack	concepts=5	theta_mode=shared
exit 0
```

`cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite pack switch template --theta-mode isolated`

```text
active_pack=template
theta_mode=isolated
exit 0
```

`cargo run -p polaris-cli -- --db target/p08c-template-clean.sqlite next`

```text
concept: template_course_goal
task_type: recall
prompt: Recall Course goal map. State the shortest useful definition and one cue that tells you it applies.
exit 0
```

`git diff --check`

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'crates/polaris-core/src/pack.rs', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/polaris-core-comic-system-brief.md', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
exit 0
```

本票 scoped diff 检查同样 exit 0，仅有 LF/CRLF warning；无 whitespace error。

### 技术选择说明

- 模板 pack 放在 `packs/template/`，与产品路线图点名位置一致，便于作者直接复制。
- 模板内容保持领域中性，只表达 pack 结构，不把 Rust、英语或考试逻辑写入 core。
- `ingest.toml` 作为可选预留文件存在，但文档明确当前 validator 不强制、core 不消费。
- 测试落在 core pack validator 层，因为 P08C 不新增 CLI 行为；CLI 通过专项真实命令验收。

### 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md crates/polaris-core/src/pack.rs
Remove-Item -Recurse -Force packs/template, docs/PACK_AUTHOR_GUIDE.md, docs/tickets/TICKET_P08C_PACK_AUTHOR_GUIDE_TEMPLATE.md
```

提交后：

```powershell
git revert <P08C-commit-sha>
```
