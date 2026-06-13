# P05A 英语示例 Domain Pack

状态：Completed（已提交；默认 target clippy 受 Windows 文件锁阻塞，隔离 target 同参数通过）

服务主命题环节：验证真懂 → 定位模糊 → 针对性补缺（跨域桥首测）

## 背景

MASTER_PLAN Phase 5 指定：英语 pack 从 Polaris CEFR 表导出，用作插拔验收、冷启动迁移评估和跨域桥首测。旧库 Polaris 的 CEFR 形状来自 `C:\MyProject\Polaris\apps\web\scripts\import-cefr-data.mjs`，其数据源为 CEFR-J vocabulary profile、Octanove C1/C2 vocabulary profile 和 CEFR-J grammar profile。

当前 core 已有 `packs/rust/` 与 `packs/algorithms/` 作为内核开发期 pack。英语不作为 core 内建线路，而是放入 `examples/packs/english/`，作为 P05A0 课程接入协议的示例 pack：证明外部课程可以按协议接入，但不把 CEFR、词汇或语法专用逻辑写入内核。

## 范围

1. 创建 `examples/packs/english/` 目录结构：

   ```text
   examples/packs/english/
     pack.toml
     concepts.toml
     misconceptions.toml
     rubric.md
     moves.toml
   ```

2. `pack.toml` 元信息：
   - `id = "english"`
   - `title = "English CEFR Example Pack"`
   - 记录来源为 Polaris CEFR 表形状和三份上游 CEFR 数据源 URL。

3. `concepts.toml` 概念种子：
   - 覆盖 CEFR A1/A2/B1/B2/C1/C2 六级。
   - 每级至少包含 vocabulary、grammar、expression 三类概念。
   - prerequisite 边形成从 A1 到 C2 的语言能力阶梯。
   - 用 `component_of` 边把 vocabulary/grammar/expression 连接到对应 CEFR level schema。
   - 用少量 `confusion` 边表达高频语言误解邻接。

4. `misconceptions.toml` 常见英语学习误解（≥8 条）：
   - 词义直译、时态边界、冠词泛化、介词搭配、从句结构、情态语气、学术词汇语域、流利错觉等。
   - 每条都挂到现有 concept id，并标注 DATA_MODEL §9 的 G_u pattern。

5. `moves.toml` 使用 7-move schema：
   - recall、explain、apply、analyze、evaluate、create、transfer。
   - 模板面向语言学习：词义检索、语法解释、句子改写、语域比较、错误评估、产出、跨语境迁移。

6. `rubric.md` 写明英语域评分标准：
   - 正确性、语境适配、语法边界、表达自然度、迁移深度。
   - strict-citation 仍必须引用 attempt 证据原文，外部 AI 不得直接改掌握度。

7. 新增 `crates/polaris-core/tests/p05a_english.rs`：
   - `pack validate examples/packs/english` 通过。
   - `init_pack("examples/packs/english")` 成功，概念数、边、误解数符合预期。
   - `next_task` 返回示例英语 pack 概念。
   - prerequisite 门控：高阶 CEFR 概念在前置未达标时不被调度。
   - 误解关联：低分 attempt 携带 misconception_id 后提高对应概念修复优先级。
   - 英语示例 pack 与 rust pack 共享相同 submit/grade/mastery 形状。
   - 冷启动评估夹具：用 seed mastery 模拟从已掌握 A1/A2 到 B1 的预测地图，确保调度优先进入 B1 而不是直接跳 C1/C2。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p polaris-cli -- pack validate examples/packs/english
cargo test -p polaris-core --test p05a_english
```

额外人工检查：

```powershell
git diff --check
```

验收要求：
- `examples/packs/english/` 通过 `pack validate`。
- 票内测试证明英语示例 pack 可被引擎加载、调度、提交、评分回填。
- 不修改冻结参考仓库。
- 不在 core 写任何英语、CEFR、词汇或语法专用逻辑。

## 禁区

- 不下载或提交完整 CEFR 词库。
- 不实现英语专用 ingest 适配器。
- 不把 CEFR level、vocabulary、grammar 规则写入 Rust 内核。
- 不修改 `packs/rust/`、`packs/algorithms/`。
- 不修改 `C:\MyProject\Polaris` 或 `C:\MyProject\Learned`。
- 不把冷启动迁移评估扩展成新的 q 向量导入机制；当前票只做可审计夹具。

## 交付记录

### 开工记录（2026-06-13）

- 当前范围：创建 `examples/packs/english/` 的 5 个声明式示例 pack 文件，并新增 `crates/polaris-core/tests/p05a_english.rs` 覆盖 pack validate、init_pack、next_task、prerequisite 门控、misconception 关联、submit/grade/mastery 一致性与冷启动评估夹具。
- 禁区：不下载完整 CEFR 词库；不实现英语 ingest；不修改 core 的域无关逻辑；不修改冻结参考库；不触碰其它 pack。
- 验收命令：
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo run -p polaris-cli -- pack validate examples/packs/english`
  - `cargo test -p polaris-core --test p05a_english`
  - `git diff --check`
- 预计修改面：`docs/tickets/QUEUE.md`、本票、`examples/packs/english/*`、`crates/polaris-core/tests/p05a_english.rs`。

### 交付记录（2026-06-13）

#### 用户裁决

- 用户指出 core 不应存在英语内建线路；本票调整为示例 pack 交付，路径从 `packs/english/` 改为 `examples/packs/english/`。
- 用户回复“可以的”，按示例 pack 定位提交。

#### 变更清单

- 新增 `examples/packs/english/`：`pack.toml`、`concepts.toml`、`misconceptions.toml`、`moves.toml`、`rubric.md`。
- `concepts.toml` 声明 24 个英语/CEFR 概念：6 个 CEFR level schema + vocabulary/grammar/expression 三轴；包含 32 条 prerequisite 边、18 条 component_of 边、3 条 confusion 边。
- `misconceptions.toml` 声明 9 条英语学习常见误解，并对齐 DATA_MODEL §9 的 G_u pattern。
- `moves.toml` 使用 7-move schema，覆盖语言学习的 recall/explain/apply/analyze/evaluate/create/transfer。
- 新增 `crates/polaris-core/tests/p05a_english.rs`，覆盖 pack validate、init_pack、next_task、CEFR prerequisite 门控、misconception 修复优先级、英语/rust submit-grade-mastery 一致性和冷启动评估夹具。
- 更新 `docs/tickets/QUEUE.md` 与本票状态；未修改 core 域无关逻辑，未修改冻结参考库。

#### TDD 红灯

`cargo test -p polaris-core --test p05a_english`

```text
running 5 tests
test english_pack_validates_expected_cefr_shape ... FAILED
test english_and_rust_packs_share_submit_grade_mastery_shape ... FAILED
test failed_english_attempt_with_misconception_raises_repair_priority ... FAILED
test english_pack_initializes_and_schedules_domain_concepts ... FAILED
test cefr_prerequisite_gate_keeps_c1_c2_out_until_intermediate_ready ... FAILED

called `Result::unwrap()` on an `Err` value: MissingFile("pack.toml")
called `Result::unwrap()` on an `Err` value: Pack(MissingFile("pack.toml"))

test result: FAILED. 0 passed; 5 failed
```

#### 验收输出

`cargo test -p polaris-core --test p05a_english`

```text
running 5 tests
test english_pack_validates_expected_cefr_shape ... ok
test english_pack_initializes_and_schedules_domain_concepts ... ok
test failed_english_attempt_with_misconception_raises_repair_priority ... ok
test cefr_prerequisite_gate_keeps_c1_c2_out_until_intermediate_ready ... ok
test english_and_rust_packs_share_submit_grade_mastery_shape ... ok

test result: ok. 5 passed; 0 failed; finished in 0.04s
```

`cargo fmt --check`

```text
exit 0，无输出。
```

`git diff --check`

```text
warning: in the working copy of '.gitignore', LF will be replaced by CRLF the next time Git touches it
warning: in the working copy of 'docs/tickets/QUEUE.md', LF will be replaced by CRLF the next time Git touches it
```

`cargo run -p polaris-cli -- pack validate examples/packs/english`

```text
pack ok: concepts=24 prerequisites=32 misconceptions=9
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s
```

`cargo test --workspace`

```text
首跑在既有 P03C geometry 用例上偶发失败：
test geometry_candidates_use_hnsw_and_combined_scores ... FAILED
thread 'geometry_candidates_use_hnsw_and_combined_scores' panicked at crates\polaris-core\tests\p03c_geometry.rs:111:10:
schema:raii candidate
后续同文件用例因 ENV_LOCK PoisonError 连锁失败。

单跑复核：
cargo test -p polaris-core --test p03c_geometry
test result: ok. 7 passed; 0 failed; finished in 0.05s

重跑全量后，再按示例 pack 路径调整复跑：
P04E: test result: ok. 3 passed; 0 failed; finished in 7.07s
P05A1: test result: ok. 5 passed; 0 failed; finished in 0.03s
P05A: test result: ok. 5 passed; 0 failed; finished in 0.03s
Doc-tests polaris_core: test result: ok. 0 passed; 0 failed
```

`cargo clippy --workspace --all-targets -- -D warnings`

```text
默认 target 失败于既有 Windows 文件锁：
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
```

同参数隔离 target 复核：

```text
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polaris-core-target-p05a-clippy'; cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.93s
```

#### 阻塞点与建议

- 阻塞点 1：默认 target 的 clippy 原命令仍受 Windows `target/debug` 文件锁影响；隔离 target 同参数已通过，符合 P05A0/P05A1 的本机裁决方式。
- 阻塞点 2：全量测试首次暴露 P03C geometry 既有偶发抖动；单跑 P03C 与重跑 workspace 均通过。已将该观察写入 QUEUE Backlog，不在本票顺手修。
- 建议：接受本票隔离 target clippy 证据；P03C HNSW 候选稳定性后续单独开票处理。

#### 回滚方式

- 删除 `examples/packs/english/`。
- 删除 `crates/polaris-core/tests/p05a_english.rs`。
- 将 `docs/tickets/QUEUE.md` 中 P05A 状态与 Backlog 观察恢复到本票开工前。
- 删除本票 `docs/tickets/TICKET_P05A_ENGLISH_PACK.md`。
