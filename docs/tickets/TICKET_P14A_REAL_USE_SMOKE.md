# TICKET P14A：真实使用 smoke v1

状态：已实现、通过验收并提交

服务主命题：验证真懂 → 定位模糊 → 针对性补缺；同时服务“用户能实际用起来”。

## 背景

P12/P13 已经补齐项目声明、capture、学习收件箱、收件箱练习桥、AI IDE MCP 入口和 AI 交互偏好。现在需要一条不靠手抄 `capture_id` 的真实使用自检，让学生或执行 AI 能在本仓库里一键跑出完整闭环，并留下可审计 transcript。

本票不新增学习数学，不改变调度、掌握度或 MCP 合约；只把已经实现的能力串成实际可跑的使用验收。

## 范围

- 新增 PowerShell smoke 脚本：
  - 构建 `polaris-cli`。
  - 使用 `target\p14a-real-use.sqlite` 临时库初始化 Rust pack。
  - 设置 AI interaction profile。
  - 检测示例 `p-os.toml` 学习项目声明。
  - 记录一条学习资料到 capture queue。
  - 自动解析 `capture_id`。
  - 查看 inbox。
  - 将该条目 `accept` 为 `practice_ready`。
  - 生成 inbox practice prompt。
  - 提交学生回答和 confidence。
  - 读取 learner mirror。
  - 将完整命令输出写入 `target\p14a-real-use-transcript.txt`。
- 新增真实使用文档，告诉用户怎么运行脚本、怎么看结果、下一步怎么把同样流程迁移到 AI IDE。
- 更新 README 和 AI IDE 使用指南，指向该 smoke 脚本。
- 在票尾粘贴脚本实跑输出和 transcript 摘要。

## 禁区

- 不修改 mastery、FSRS、BKT/MIRT、相图、调度、评分或数据库 schema。
- 不新增桌面 UI 或新的 daemon。
- 不把 smoke 数据写入用户默认数据库；必须使用 `target\` 下临时库。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 验收

必须真实运行并粘贴输出：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

脚本输出必须包含：

- `P14A real-use smoke passed.`
- transcript 路径。
- capture_id。
- inbox practice 提示。
- submit 后 attempt/provisional 相关输出。

## 回滚方式

- 删除 `scripts\real_use_smoke.ps1`。
- 删除真实使用文档。
- 恢复 README、AI IDE 使用指南和 QUEUE 中 P14A 相关说明。
- 删除 `target\p14a-real-use.sqlite*` 与 `target\p14a-real-use-transcript.txt` 即可清理运行产物。

## 本轮范围（2026-06-24）

- 用户要求“今天要完成 Polaris OS 的剩下工作，然后实际开始用起来的测试”。
- 本票把该裁决收敛成最小可交付：真实使用 smoke 脚本和可复跑记录，不扩展产品架构。

## 交付记录（2026-06-24）

### 变更清单

- 新增 `scripts\real_use_smoke.ps1`：
  - 使用 `target\p14a-real-use.sqlite` 临时库。
  - 构建 `polaris-cli`。
  - 自动运行 init、AI profile、project detect、capture、inbox list、inbox accept、inbox practice、inbox submit、learner mirror。
  - 自动解析 `capture_id`，不用用户手抄。
  - 将完整输出写入 `target\p14a-real-use-transcript.txt`。
  - `DbPath` 与 `TranscriptPath` 必须位于本仓库 `target\` 下，避免误删或污染用户库。
  - 脚本自身断言关键输出字段：project、recorded_only、practice_ready、prompt、attempt、provisional score、learner mirror。
- 新增 `docs\REAL_USE_SMOKE.md`，说明怎么运行、看哪些信号、如何换成真实课程路径。
- 更新 `README.md` 和 `docs\AI_IDE_USAGE.md`，把真实使用 smoke 作为本机自检入口。

### 验收输出

```powershell
> powershell -ExecutionPolicy Bypass -File scripts\real_use_smoke.ps1
capture_id: feae3cc4-99e8-4330-89be-2ca4110f7dbf
P14A real-use smoke passed.
transcript: C:\MyProject\polaris-core\target\p14a-real-use-transcript.txt
```

transcript 关键行：

```text
project_id: rust-mastery-lab
default_pack: rust
today_command: cargo run -p labctl -- today --date {today}
capture_id: feae3cc4-99e8-4330-89be-2ca4110f7dbf
recorded_only: true
status: practice_ready
prompt: 请用自己的话回答：这条资料和「Ownership」有什么关系？请解释关键点，并给出一个例子或反例。
attempt_id: dbc79eb7-9049-4138-b68f-dbee66506505
provisional_score: 0.700
degraded: true
"confidence_curve": [
```

```powershell
> cargo fmt --check
```

无输出，退出码 0。

```powershell
> cargo clippy --workspace --all-targets -- -D warnings

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s
```

```powershell
> cargo test --workspace

running 93 tests
...
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
...
running 5 tests
test update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation ... ok
test update_ai_interaction_profile_rejects_invalid_values_without_mutation ... ok
test default_ai_interaction_profile_is_balanced_and_read_only ... ok
test update_ai_interaction_profile_trims_blank_custom_notes ... ok
test update_ai_interaction_profile_persists_student_preferences ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 回滚方式

- 删除 `scripts\real_use_smoke.ps1`。
- 删除 `docs\REAL_USE_SMOKE.md`。
- 恢复 `README.md`、`docs\AI_IDE_USAGE.md`、`docs\tickets\QUEUE.md` 和本票状态。
- 删除运行产物 `target\p14a-real-use.sqlite*` 与 `target\p14a-real-use-transcript.txt`。
