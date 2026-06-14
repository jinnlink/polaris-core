# P06E 性能预算回归

状态：已完成

服务主命题：全环节（Tier 0 预算铁律）

## 背景

DATA_MODEL §11 已冻结核心预算：U(c) 全表选题 <10ms @10k 概念、单条 fold <50us、概念全量重放 <1ms/百条、HMM 前向一步 <1us。P03L 已补索引和查询计划断言，但明确不做性能预算基准。本票把这些预算做成可重复运行的回归门，防止后续改动把 Tier 0 热路径拖慢。

本票只加测试/基准护栏，不改变算法、阈值、数据模型或调度语义。

## 本轮范围

1. 新增 P06E 性能预算测试：
   - 调度候选池：10k `ScheduleCandidate` 排序/选题在预算内。
   - 单条掌握度 fold：`fold_attempt` 在预算内。
   - 概念重放：100 条 `AttemptObservation` 的 `fold_all` 在预算内。
   - HMM 前向一步：`forward_filter` 在预算内。
2. 测试必须可在普通 `cargo test --workspace` 下稳定运行：
   - debug profile 使用放宽倍数，避免开发机抖动造成误报。
   - release profile 使用 DATA_MODEL §11 原预算断言。
   - 每项使用多轮测量取中位数，降低偶发调度抖动。
3. 不引入外部 benchmark crate；当前 workspace 无 criterion 依赖，本票用 `std::time::Instant` 做轻量预算断言，避免为小票新增网络下载依赖。

## 验收

必须通过：

```powershell
cargo test -p polaris-core p06e
cargo test --release -p polaris-core p06e
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

额外人工检查：

```powershell
git diff --check
```

## 禁区

- 不修改 BKT/FSRS/MIRT/HMM/调度/相图公式。
- 不调整 DATA_MODEL §11 的 release 预算阈值。
- 不把性能测试写成依赖真实墙钟网络、LLM 或外部服务的用例。
- 不处理 `.gitignore`、`.cursor/`、`docs/visuals/` 等票外改动。
- 不修改 frozen 参考仓库。

## 交付记录

### 变更清单

- 新增 `crates/polaris-core/tests/p06e_performance_budget.rs`。
- 覆盖 DATA_MODEL §11 四条 Tier 0 热路径预算：
  - `p06e_scheduler_ranks_10k_candidates_within_budget`：10k `ScheduleCandidate` 经过 `rank_candidates_with_params` 排序/选题。
  - `p06e_single_attempt_fold_stays_within_budget`：单条 `fold_attempt`。
  - `p06e_replay_100_attempts_stays_within_budget`：100 条 `AttemptObservation` 的 `fold_all` 重放。
  - `p06e_hmm_forward_step_stays_within_budget`：单步 `forward_filter`。
- 测试用 `Instant` 多轮测量取中位数；debug profile 使用 100x 放宽倍数，release profile 使用 DATA_MODEL §11 原预算。
- 更新 `docs/tickets/QUEUE.md`，将性能预算回归从 backlog 转为 P06E 正式票。

### 红灯记录

```text
cargo fmt --check
Diff in crates\polaris-core\tests\p06e_performance_budget.rs
exit 1
```

说明：首轮红灯为新测试文件格式未满足 rustfmt；执行 `cargo fmt` 后重跑通过。本票不改生产代码，新增预算测试直接复用既有 public API，因此没有生产编译红灯。

### 验收输出

```text
cargo test -p polaris-core p06e
p06e_scheduler_ranks_10k_candidates_within_budget ... ok
p06e_single_attempt_fold_stays_within_budget ... ok
p06e_replay_100_attempts_stays_within_budget ... ok
p06e_hmm_forward_step_stays_within_budget ... ok
4 passed; 0 failed
exit 0
```

```text
cargo test --release -p polaris-core p06e
p06e_scheduler_ranks_10k_candidates_within_budget ... ok
p06e_single_attempt_fold_stays_within_budget ... ok
p06e_replay_100_attempts_stays_within_budget ... ok
p06e_hmm_forward_step_stays_within_budget ... ok
4 passed; 0 failed; finished in 0.08s
Finished `release` profile [optimized] target(s) in 1.56s
exit 0
```

```text
cargo fmt --check
exit 0
```

```text
cargo clippy --workspace --all-targets -- -D warnings
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-225b025d05403e51.rmeta: 拒绝访问。 (os error 5)
error: failed to write C:\MyProject\polaris-core\target\debug\deps\libpolaris_core-25752c227aae4632.rmeta: 拒绝访问。 (os error 5)
exit 1
```

说明：默认 target 目录仍受 Windows 文件锁影响；按既有工作区处理方式改用隔离 target 目录验证代码本身。

```text
cargo clippy --target-dir $env:POLARIS_CLIPPY_TARGET --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.43s
exit 0
```

```text
cargo test --workspace
polaris-cli unit: 29 passed
polaris-core unit: 68 passed
integration suites: all passed, including p06c_property_expansion 3 passed, p06d_mirror_report_narrative 6 passed, p06e_performance_budget 4 passed
doc-tests: 0 passed
exit 0
```

```text
git diff --check
exit 0
仅 LF/CRLF warning，无 whitespace error。
```

### 技术选择

- 不新增 criterion 或其它 benchmark crate，避免小票引入外部下载依赖；用标准库 `Instant` 做轻量预算断言。
- release profile 按 DATA_MODEL §11 原始纳秒预算断言：10ms、50us、1ms/百条、1us。
- debug profile 仅作为稳定烟测，使用 100x 放宽倍数，保证 `cargo test --workspace` 不因调试构建和 Windows 调度抖动误报。
- 每项测量多轮取中位数，并用批量循环摊薄 `Instant` 读取开销；`black_box` 防止优化器删除热路径。
- 调度预算的 10k 候选池在计时前预生成多份 owned fixture，计时区内只取一份传入排序函数，不把 clone/allocator 成本计入预算。
- 只测已有 public API，不新增测试专用生产入口，不改变公式或参数。

### 子 agent 审查

Peirce（`019ec5a8-8f7d-7f91-a767-e2c7c4bbb302`）只读审查结论：

- 核心 P06E 测试未发现修改 BKT/FSRS/MIRT/HMM/调度/相图公式或阈值的问题；生产 Rust 源码没有相关 diff。
- 四个预算点均有覆盖：10k 调度、单条 `fold_attempt`、100 条 `fold_all`、HMM 一步 `forward_filter`。
- release 阈值与 DATA_MODEL §11 对齐，debug 100x 放宽作为 `cargo test --workspace` 烟测总体合理。
- 必须处理项：工作树存在票外 `.gitignore`、`.cursor/`、`docs/visuals/`、`docs/superpowers/plans/2026-06-13-polaris-porcelain-atlas.md`。处理方式：不回退用户或其他窗口改动，提交时只白名单 stage P06E 文件，确保这些票外改动不进入本票提交。
- 建议项：10k 调度测试原先在计时区内 clone fixture，偏保守但可能引入 allocator 抖动。已改为预生成多份 owned fixture，计时区内只 `pop` 一份传入 `rank_candidates_with_params`，并重跑 debug/release 定向测试、隔离 clippy 与全量测试。
- 建议项：默认 clippy 的原命令实跑失败需明确闭环。已记录默认 target 文件锁失败，并用同参数 `--target-dir` 隔离目录复验通过；该绕行只改变构建产物目录，不改变 clippy 检查范围或 lint 参数。

## 回滚方式

未提交前：

```powershell
git restore docs/tickets/QUEUE.md
git clean -f docs/tickets/TICKET_P06E_PERFORMANCE_BUDGET_REGRESSION.md crates/polaris-core/tests/p06e_performance_budget.rs
```

提交后：

```powershell
git revert <P06E-commit-sha>
```
