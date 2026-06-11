# P01 Walking Skeleton 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:executing-plans 或按本计划内联执行。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现 P01 Rust 内核最小闭环：ingest 证据、提交 attempt、乐观落账、异步/降级评分、确定性 fold 掌握度、调度下一任务、CLI 操作与 rust pack 校验。

**架构：** 新建 Cargo workspace，核心逻辑放在 `crates/polaris-core`，CLI 薄封装放在 `crates/polaris-cli`。SQLite 是事实源，`attempts`、`behavior_events` 和 pack 种子可折叠出 `mastery_states`；P01 只实现 DATA_MODEL 中激活的 FSRS、BKT、校准、strict-citation、U(c) 调度和降级评分。

**技术栈：** Rust、rusqlite(bundled)、serde、serde_json、thiserror、clap、reqwest blocking、toml、tempfile（测试）。

---

## 文件职责

- `Cargo.toml`：workspace 成员与共享配置。
- `crates/polaris-core/Cargo.toml`：核心库依赖。
- `crates/polaris-core/src/lib.rs`：模块出口。
- `crates/polaris-core/src/config.rs`：参数登记处，携带默认值、边界、类型 A/B/C、调优途径。
- `crates/polaris-core/src/error.rs`：统一错误类型。
- `crates/polaris-core/src/db.rs`：SQLite 打开、WAL、迁移、默认 meta 初始化。
- `crates/polaris-core/src/fsrs.rs`：从冻结 Polaris 移植 FSRS。
- `crates/polaris-core/src/mastery.rs`：attempt fold、BKT、校准、FSRS 状态更新、全量重放。
- `crates/polaris-core/src/citation.rs`：strict-citation 校验。
- `crates/polaris-core/src/pack.rs`：pack TOML 解析、安装、validate。
- `crates/polaris-core/src/grader.rs`：LLM 评分接口、strict-citation 重试、启发式降级、grade_queue。
- `crates/polaris-core/src/scheduler.rs`：U(c) 计算与决定性排序。
- `crates/polaris-core/src/engine.rs`：CLI 用例编排。
- `crates/polaris-cli/Cargo.toml`：CLI 依赖。
- `crates/polaris-cli/src/main.rs`：`polaris` 命令。
- `packs/rust/*.toml`、`packs/rust/rubric.md`：P01 rust pack。
- `README.md`：quickstart 与实跑记录入口。

## 任务 1：建立 workspace 和最小测试骨架

**文件：**
- 创建：`Cargo.toml`
- 创建：`crates/polaris-core/Cargo.toml`
- 创建：`crates/polaris-core/src/lib.rs`
- 创建：`crates/polaris-core/src/error.rs`
- 创建：`crates/polaris-cli/Cargo.toml`
- 创建：`crates/polaris-cli/src/main.rs`

- [ ] **步骤 1：编写失败测试**
  创建 `crates/polaris-core/src/lib.rs` 中的最小模块测试：

  ```rust
  #[cfg(test)]
  mod tests {
      #[test]
      fn crate_exports_version() {
          assert_eq!(crate::VERSION, "0.1.0");
      }
  }
  ```

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core crate_exports_version`
  预期：失败，原因是没有 workspace 或没有 `VERSION`。

- [ ] **步骤 3：写最少实现**
  建立 workspace、两个 crate，并在 core 暴露：

  ```rust
  pub const VERSION: &str = "0.1.0";
  ```

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core crate_exports_version`
  预期：PASS。

## 任务 2：SQLite 迁移与 meta 参数

**文件：**
- 创建：`crates/polaris-core/src/config.rs`
- 创建：`crates/polaris-core/src/db.rs`
- 修改：`crates/polaris-core/src/lib.rs`

- [ ] **步骤 1：编写失败测试**
  在 `db.rs` 写测试：内存库迁移后存在 P01 表；`journal_mode` 为 WAL；`meta` 中有 `bkt.p_init` 和 `grade.quote_min`。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core db::tests::`
  预期：FAIL，原因是 `db`/`config` 模块不存在。

- [ ] **步骤 3：写最少实现**
  按 `DATA_MODEL.md` P01 表创建迁移；在 `config.rs` 定义 `ParameterSpec`、`ParameterClass`、`TuningRoute`，默认参数覆盖 P01 需要的所有常数。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core db::tests::`
  预期：PASS。

## 任务 3：FSRS 对拍移植

**文件：**
- 创建：`crates/polaris-core/src/fsrs.rs`
- 修改：`crates/polaris-core/src/lib.rs`
- 参考只读：`C:\MyProject\Polaris\apps\web\src\lib\fsrs.ts`

- [ ] **步骤 1：读取冻结实现并写失败测试**
  从 ts 版提取 `w[0..16]` 与 5 条序列期望值，测试 `init_stability`、`next_difficulty`、`retrievability`、`calculate_next_due`。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core fsrs::tests::`
  预期：FAIL，原因是 FSRS 函数未实现。

- [ ] **步骤 3：1:1 移植最少实现**
  使用 f64 计算，score→rating 按 `DATA_MODEL.md`，时间差按 elapsed_days 定义。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core fsrs::tests::`
  预期：PASS。

## 任务 4：掌握度 fold、BKT、校准和属性测试

**文件：**
- 创建：`crates/polaris-core/src/mastery.rs`
- 修改：`crates/polaris-core/src/db.rs`
- 修改：`crates/polaris-core/src/lib.rs`

- [ ] **步骤 1：编写失败测试**
  覆盖 BKT 判对、判错、中间区、参数从 meta 读取；校准 gap、Brier、死区跳过；增量 fold 等于全量重放。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core mastery::tests::`
  预期：FAIL，原因是 fold API 不存在。

- [ ] **步骤 3：写最少实现**
  实现 `fold_attempt`、`replay_concept`、`apply_final_score`，attempt 不可变，final 回填后对概念全量重放。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core mastery::tests::`
  预期：PASS。

## 任务 5：strict-citation 和 grader 降级

**文件：**
- 创建：`crates/polaris-core/src/citation.rs`
- 创建：`crates/polaris-core/src/grader.rs`
- 修改：`crates/polaris-core/src/lib.rs`

- [ ] **步骤 1：编写失败测试**
  覆盖 citation 通过、quote 过短、quote 过长、quote 非 evidence 子串；LLM env 缺失时降级并入 `grade_queue`。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core citation::tests:: grader::tests::`
  预期：FAIL，原因是模块不存在。

- [ ] **步骤 3：写最少实现**
  实现 citation 校验；grader 在无 env 时返回启发式结果并写队列；有 env 的 HTTP 调用只在集成路径封装，不让 P01 同步阻塞。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core citation::tests:: grader::tests::`
  预期：PASS。

## 任务 6：rust pack 与 pack validate

**文件：**
- 创建：`crates/polaris-core/src/pack.rs`
- 创建：`packs/rust/pack.toml`
- 创建：`packs/rust/concepts.toml`
- 创建：`packs/rust/misconceptions.toml`
- 创建：`packs/rust/rubric.md`
- 创建：`packs/rust/moves.toml`
- 修改：`crates/polaris-core/src/lib.rs`

- [ ] **步骤 1：编写失败测试**
  覆盖合法 pack 通过、缺引用失败；概念数 ≥24、prerequisite ≥15、误解 ≥10。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core pack::tests::`
  预期：FAIL，原因是 pack 解析和文件不存在。

- [ ] **步骤 3：写最少实现**
  参考冻结 Learned 的 TOML 形状，声明 rust pack；validate 检查文件、字段、引用完整性。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core pack::tests::`
  预期：PASS。

## 任务 7：调度 U(c) 与 next

**文件：**
- 创建：`crates/polaris-core/src/scheduler.rs`
- 修改：`crates/polaris-core/src/lib.rs`

- [ ] **步骤 1：编写失败测试**
  覆盖高自信低分优先、误解窗口、14 天语义、平手排序 `seed_order` → id。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core scheduler::tests::`
  预期：FAIL，原因是 scheduler 不存在。

- [ ] **步骤 3：写最少实现**
  实现 `rank_next_concepts`，U 公式严格按 `DATA_MODEL.md`，新概念引入检查 prerequisite p_known。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core scheduler::tests::`
  预期：PASS。

## 任务 8：engine 和 CLI 全集

**文件：**
- 创建：`crates/polaris-core/src/engine.rs`
- 修改：`crates/polaris-cli/src/main.rs`

- [ ] **步骤 1：编写失败测试**
  核心层测试 init、ingest、next、submit、hint、abandon、status、grade-pending；CLI 只做参数解析烟测。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test -p polaris-core engine::tests::`
  预期：FAIL，原因是 engine 不存在。

- [ ] **步骤 3：写最少实现**
  CLI 调用 engine；DB 路径默认 `%USERPROFILE%\.polaris-core\core.db`，`POLARIS_CORE_DB` 可覆盖。

- [ ] **步骤 4：运行测试验证通过**
  运行：`cargo test -p polaris-core engine::tests::`
  预期：PASS。

## 任务 9：集成流和 README quickstart

**文件：**
- 修改：`README.md`
- 新增或修改：核心集成测试文件
- 修改：`docs/tickets/TICKET_P01_WALKING_SKELETON.md`

- [ ] **步骤 1：编写失败测试**
  自动化种子流 ≥6 概念、≥10 attempts，断言 P01 票列出的 4 项集成行为。

- [ ] **步骤 2：运行测试验证失败**
  运行：`cargo test --workspace integration`
  预期：FAIL，原因是集成路径未完整。

- [ ] **步骤 3：写最少实现和 README quickstart**
  补齐 CLI 可跑路径；README 只描述 P01 已实现能力。

- [ ] **步骤 4：运行完整验收**
  运行：

  ```powershell
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

  预期：全部 exit 0。

- [ ] **步骤 5：填写票尾交付记录**
  在当前票尾填写变更清单、验收输出、阻塞与裁决记录、技术选择说明、回滚方式。
