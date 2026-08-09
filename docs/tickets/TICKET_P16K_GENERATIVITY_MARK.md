# P16K 生成性概念标记（Generative vs Item Knowledge）

状态：已实现并通过验收，待提交；依赖 P05A0、P03F。P16D 已提交（`7a478cc`）。

服务主命题：针对性补缺。

## 背景

两类知识的调度经济学相反，但图谱现在无法区分。

- **生成性知识**：学会一次能推出一批未见实例，且可验证。例：英语构词的 `-cede/-ceed/-sede` 词根；Rust 的 RAII 图式。给一个没教过的同族实例让学习者推断，就是干净的 transfer 检验。
- **项目型知识**：只能逐个记，不产生推断力。例：不规则动词过去式；某个 API 的确切签名。

生成性知识值得深挖（一次投入、多次回报），项目型知识只值得间隔重复，深挖是浪费。

现有边类型 `prerequisite` / `confusion` / `component_of` / `instantiates` / `maps_to` 都不表达这个区别。理论上迁移梯度 `∂(Y 预测掌握)/∂(练 X)` 能学出来，但那需要大量数据；而在 pack 里这是**先验已知**的，直接声明比等它拟合出来便宜几个数量级。

## 范围

1. `concepts` 增加 `generativity TEXT NOT NULL DEFAULT 'unknown'`，枚举：`generative` / `item` / `unknown`。
2. 课程接入协议 v1 扩展：`[[concept]]` 增加可选 `generativity` 字段。未声明即 `unknown`，向后兼容，既有 pack 无需修改。
3. `polaris pack validate` 校验枚举合法性。
4. 消费点**只有一个**：`teaching_instruction`。
   - `generative` 概念：优先选 transfer / apply 类 move，`do_text` 要求「给一个没教过的同族实例，让学习者推断并说明依据」。
   - `item` 概念：不给深挖建议，保持既有行为。
   - `unknown`：行为与当前完全一致。
5. 更新 `docs/COURSE_INTEGRATION_PROTOCOL.md` 与 `packs/template/concepts.toml` 示例。

## 验收

必须通过：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p polaris-cli -- pack validate packs/template
cargo test -p polaris-core --test p16k_generativity
cargo test --workspace
```

专项要求：

- 未声明 `generativity` 的既有 pack（`packs/rust`、`packs/algorithms`、`examples/packs/english`）行为逐字段不变。
- 非法枚举被 validator 拒绝。
- `generative` 与 `item` 概念的 `teaching_instruction` 输出确有差异，有对照用例。
- 调度结果（`next_task` 返回的概念序列）在本票前后完全一致。

## 禁区

- 不改调度、`U(c)`、mastery、相图、迁移梯度公式。
- 不自动推断 `generativity`，只接受 pack 显式声明。
- 不把 `generativity` 当成难度或先验掌握度。
- 不新增边类型。
- 不修改冻结仓库。

## 回滚

删除 `concepts.generativity` 列与 `teaching_instruction` 分支；恢复协议文档与模板 pack；删除测试。

## 开工记录（2026-08-09）

- 范围：只接受 Pack 显式 `generativity=generative|item|unknown`，未声明保持 unknown；唯一消费点是教学指令。
- 禁区：不改调度、`U(c)`、mastery、相图、迁移梯度，不自动推断，不新增边类型。
- 验收：fmt、workspace clippy、模板 pack validate、P16K 专项和 workspace 全测。
- 预计修改面：schema v7/迁移、Pack DTO/validator/加载、教学指令分支、模板与课程接入协议、专项及迁移回归。

## 交付记录（2026-08-09）

### 变更清单

- schema v7 为 `concepts` 增加非空 CHECK 三态 `generativity`；旧行默认 `unknown`，迁移账本登记 `concept_generativity`。
- Pack DTO/validator/加载支持 `generative | item | unknown`；未声明字段向后兼容为 `unknown`，非法枚举给出概念级错误。
- 唯一消费点位于普通 `teaching_instruction`：`generative` 使用 transfer 教学锚点并要求未见同族实例推断；`item` 与 `unknown` 逐字段保持原处方。
- 模板 Pack 同时示范 generative、item 与省略字段；课程接入协议和数据模型补齐边界。
- P16K 专项锁定三个旧 Pack 默认行为、非法枚举、教学对照及三种标记下调度签名不变。

### 验收实跑

```text
> cargo fmt --check
exit 0

> CARGO_TARGET_DIR=target_p16k_acceptance cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile ...
exit 0

> cargo run -p polaris-cli -- pack validate packs/template
pack ok: concepts=5 prerequisites=4 misconceptions=3
exit 0

> cargo test -p polaris-core --test p16k_generativity
running 4 tests
test result: ok. 4 passed; 0 failed

> CARGO_TARGET_DIR=target_p16k_acceptance cargo test --workspace --quiet
polaris-cli: 115 passed; polaris-core: 81 passed
p16k_generativity: 4 passed
all discovered suites: exit 0
```

- 说明：默认 `target` 曾返回 Rust 元数据缓存损坏，而非测试断言失败；最终 Clippy 与全测均在全新独立缓存实跑通过。验收后执行 `cargo clean --target-dir target_p16k_acceptance`，删除本票创建的 7198 个缓存文件（7.4 GiB）。

### 回滚

- 执行 `git revert <P16K-commit-sha>` 移除 schema/Pack/教学分支；已升级真实库不做破坏性降级，使用升级前备份恢复，旧二进制按 P11A 拒绝写入 schema v7。
