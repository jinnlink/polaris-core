# P16L 材料层（Material Layer）

状态：已实现、通过验收并提交（`62de660`）；依赖 P05A0、P11A。P16D 已提交（`7a478cc`）。

服务主命题：定位模糊 → 针对性补缺。

## 背景

当前 33 张表里**没有任何材料实体**。`evidence_items.source` 只是来源字符串（`cli-submit`、`paste`、`browser`），不是对象，没有级别，没有身份。

后果是难度模型缺了一维。MIRT 里有 `b_c`（概念难度）与 `d_t`（任务形式难度：自由讲解 > 改写 > 完形 > 选择），**没有材料难度维**。

三个具体损害：

1. **85% 规则实际失效。** 同一个概念在初级读物句子上首次成功率 90%，在原著句子上 20%。引擎算的是概念级预测成功率，无法把任务难度调进 0.80–0.90，因为它不知道学习者在哪份材料上练。F3 摩擦曲线因此建立在一个看不见的变量上。
2. **材料超纲被错误归因成概念不会。** 材料整体远超级别时，引擎会把材料难度记到概念头上，将一批已掌握概念打回重练。这是主动的错误诊断，比不诊断更糟。
3. **外部难度校准无处落地。** 「升材料 / 降材料 / 不动」这类结论没有对象可以升降。

**领域无关性**：每个领域都有「你在拿什么练」（Rust 的 the Book 章节、算法教材小节、语言课程的课与读物），也都有材料难度 ≠ 概念难度。级别体系由 pack 声明，内核只存储与保序，不解释语义。因此这是合法的内核缺口，不违反 SPEC §4。

## 范围（只做记录层，不接数学）

1. 新增 `materials(id, pack, kind, level, title, source_ref, created_at)`。`level` 是 pack 声明的有序标签，内核**只保序不解释语义**。
2. `attempts` 增加 `material_id TEXT NULL`。
3. 课程接入协议扩展：新增可选 `materials.toml`，含 `[[material]]` 与有序的 `[levels]` 声明。未提供即无材料层，既有 pack 不受影响。
4. `polaris pack validate` 校验材料引用的 level 在 `[levels]` 中存在。
5. 提交入口（CLI / HTTP / MCP）可选传 `material_id`，不存在的 ID 拒绝写入。
6. 只读出口：按材料与按 level 聚合的表现摘要（attempt 数、平均 `final_score`、首次成功率）。

## 明确留空

**本票不把 material level 接进 `d_t`、预测成功率或 `U(c)`。**

那一步会改变预测与调度行为，按 SPEC §3 验证门必须先证明「含材料维的模型在留出集上的预测 logloss 优于不含材料维的基线」，margin 对齐 `consol.accept_margin`。单开后续票（候选 P16M），本票不做。

这样切分的理由：记录层零行为风险，可以立刻开始积累 `material_id` 数据；而没有数据就无法验证数学接入是否值得。先记录后建模是唯一可行顺序。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p polaris-cli -- pack validate packs/template
cargo test -p polaris-core --test p16l_material_layer
cargo test --workspace
```

专项要求：

- 无 `materials.toml` 的既有 pack 行为逐字段不变。
- `material_id` 为 NULL 时，提交与评分路径与本票前完全一致（回归断言）。
- 未知 `material_id` 被拒绝且无部分写入。
- level 保序：聚合摘要按 pack 声明顺序输出，不按字典序。
- 引用了未声明 level 的材料被 validator 拒绝。
- schema 迁移中断测试（沿用 P11A/P16D 做法）。
- **反向断言**：写入 `material_id` 后，`p_known`、θ、预测成功率、`next_task` 结果全部不变。此用例必须存在。

## 禁区

- 不把材料难度接进 MIRT、预测成功率或调度效用。
- 不在内核解释 level 语义。CEFR、Bookworms、章节序号等一律由 pack 声明。
- 不存储材料正文，只存引用（`source_ref`）。
- 不自动推断材料级别。
- 不建立第二套概念-材料覆盖关系图（那是更大的设计，本票不碰）。
- 不修改冻结仓库。

## 回滚

删除 `materials` 表与 `attempts.material_id`；移除 `materials.toml` 协议与 validator 分支；恢复协议文档；删除测试。schema 回退按 P11A 策略。

## 开工记录（2026-08-09）

- 范围：只建立材料身份、Pack 自定义 level 顺序、attempt 可选关联和表现聚合。
- 禁区：不把材料接入 MIRT、预测、`U(c)`、mastery 或调度；不存正文、不推断级别、不建立覆盖图。
- 验收：fmt、workspace clippy、模板 Pack validate、P16L 专项和 workspace 全测。
- 预计修改面：schema v8、Pack DTO/validator/初始化、Core 提交与聚合、CLI/HTTP/MCP、协议文档、模板及迁移回归。

## 交付记录（2026-08-09）

### 变更清单

- schema v8 新增 `materials`、`attempts.material_id` 与查询索引，迁移账本登记 `material_layer`；迁移单元失败时版本、账本和列变更整体回滚。
- Pack 可选读取 `[levels].order` 与 `[[material]]`；未提供的既有 Pack 保持空材料层，未声明 level、重复/空 level 和材料必填字段为空均拒绝。
- 初始化 Pack 保存材料与 level 声明顺序；材料 ID 跨 Pack 冲突拒绝，正文不入库，只保存 `source_ref`。
- Core、CLI、HTTP、MCP 提交入口支持可选 `material_id`；未知 ID 在 session、evidence、attempt 或 mastery 写入前拒绝，旧入口继续走 NULL 路径。
- 新增按材料、按 level 的只读摘要，输出 attempt 数、平均 final 分数和首次成功率；CLI、HTTP、MCP 共用同一 Core DTO，level 严格按 Pack 声明顺序。
- API 合约、课程接入协议、数据模型与模板 Pack 已同步；专项覆盖旧 Pack 兼容、迁移中断、零部分写入、level 保序和数学/调度反向不变。

### 验收实跑

```text
> CARGO_TARGET_DIR=target_p16l_acceptance CARGO_BUILD_JOBS=1 cargo fmt --check
exit 0

> CARGO_TARGET_DIR=target_p16l_acceptance CARGO_BUILD_JOBS=1 cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> CARGO_TARGET_DIR=target_p16l_acceptance CARGO_BUILD_JOBS=1 cargo run -p polaris-cli -- pack validate packs/template
pack ok: concepts=5 prerequisites=4 misconceptions=3
exit 0

> CARGO_TARGET_DIR=target_p16l_acceptance CARGO_BUILD_JOBS=1 cargo test -p polaris-core --test p16l_material_layer
running 5 tests
test result: ok. 5 passed; 0 failed

> CARGO_TARGET_DIR=target_p16l_acceptance CARGO_BUILD_JOBS=1 cargo test --workspace --quiet
polaris-cli: 118 passed; polaris-core: 81 passed
p16l_material_layer: 5 passed
all discovered suites: exit 0
```

- 说明：默认 `target` 首次并行全测因 Windows 系统资源不足触发 rustc OOM/元数据损坏，不是测试断言失败；最终全部验收均在隔离 target、单编译任务下实跑通过。

### 回滚

- 执行 `git revert 62de660` 移除 schema、Pack、提交关联与聚合出口；已升级真实库不做破坏性降级，使用升级前备份恢复，旧二进制按 P11A 拒绝写入 schema v8。
