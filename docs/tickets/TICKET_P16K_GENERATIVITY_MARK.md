# P16K 生成性概念标记（Generative vs Item Knowledge）

状态：Queued；依赖 P05A0、P03F。P16D 提交前不得认领。

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
