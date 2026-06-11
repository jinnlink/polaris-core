# polaris-core / 工程宪法（SPEC）

> 实现、讨论、代码与文档冲突时，以本文件为准。设计意图的完整版在 `docs/MASTER_PLAN.md`。
> 执行纪律：单票制——任何时刻只允许 1 张票 In Progress，见 `docs/tickets/QUEUE.md`。

## 0) 文档优先级

铁律与边界：`SPEC.md` 最高。
设计意图冲突：以 `docs/MASTER_PLAN.md` 为准。
**实现细节冲突（公式/DDL/参数/算法配方）：以 `docs/DATA_MODEL.md` 为准**（它更具体、且经审计加固）。
其后才是 `docs/tickets/*` > 代码注释。

## 1) 主命题（系统存在的唯一理由）

**验证真懂 → 定位模糊 → 针对性补缺。**

"掌握" := 在足够深度上被证明的、校准过的、可溯源的理解（六维掌握度向量 + 知识相图，见 MASTER_PLAN）。
任何特性必须服务这个闭环，否则降级为插件或砍掉。

## 2) 三支柱

1. **抽象引擎**（知识状态）：掌握度向量 {R 保持, θ 潜在技能, p_known, C 校准, D 深度} + 四空间关联 + 夜间巩固。
2. **心智动力学引擎**（学习者状态）：特质先验 → 状态 HMM → 事件层；放弃 hazard；镜像报告。
3. **教学策略引擎**：moves 库 + F1 签名 × F2 相图 × F3 摩擦选法 + MRT 因果个人化（+ F5 育种）。

## 3) 铁律（违反即返工）

- **同步路径零 LLM**："打开就要看到"的一切 <10ms，只读 Tier 0 状态。
- **三 Tier**：Tier 0 引擎确定性计算；Tier 1 内置后台 LLM（评分/解析/巩固/报告，异步）；Tier 2 外部导师（经 MCP）。Tier 1 是引擎的雇员，不是用户的对话对象。
- **乐观更新**：提交即按启发式 provisional 落账，评分回来修正。用户永不阻塞等待。
- **评分一致性**：引擎永远自己跑 evidence-bound 评分；外部 AI 的判断只能作为证据 ingest，不得直接改掌握度或调度。
- **Schedule-first**：调度权威是本地状态；LLM 只能建议。
- **Evidence-bound + strict-citation**：一切 LLM 产物必须引用证据原文并通过子串校验；该原则延伸到一切派生知识——节点/边/图式/维度/报告断言都带 provenance + evidence_ids。
- **Local-persistent**：SQLite 单库（WAL），数据主权在本地。
- **Graceful degradation**：每个 LLM 任务必有降级路径（启发式 + 排队重试）；不崩，也不假装不依赖 AI。
- **个人化铁律**：群体先验只作初始化；一切运行参数必须有按人后验的更新路径。
- **参数三类制**（DATA_MODEL §10/§12）：A 结构/治理（只能用户改，自调优禁触，含一切验证门槛）；B 经验缺省（**起点值不是真理**，由数据接管：可重放的走夜间自调优，干预类走 MRT）；C 在线拟合（登记值只是初始化）。实现上 config 模块为每参数携带（默认/边界/类型/调优途径）。
- **反伪科学红线**：禁学习风格类型（Pashler 2008 证伪）、禁 MBTI、禁一切不可证伪标签；构念双门槛 = 文献效度 + 本系统内可被行为数据证伪。
- **验证门**：每个理论对象（巩固产物、相、签名、φ\*、G_u、育种 move）必须过其留出预测门；不过门 = 假设，不得进产品话术与默认行为。
- **动机伦理**：一切动机机制必须过"用户知情后仍会认可"测试；禁暗模式；禁社会比较（只和过去的自己比）。
- **交互铁则**：UI 永远给 2-3 个任务选项，绝不下单一指令；HMM 检出心流态 → 压所有通知。

## 4) 架构边界

- 内核 = Rust workspace（`crates/polaris-core` 引擎库 + `crates/polaris-cli`）。**领域无关**。
- 三个门：MCP（主入口，Phase 2）/ HTTP API（伴随 UI，Phase 4）/ 内置 LLM（可选便利）。
- **Domain Pack 纯声明式**（pack.toml / concepts.toml / misconceptions.toml / rubric.md / ingest.toml / moves.toml），装新域 = 放目录。任何域逻辑（CEFR、Rust 语法知识等）不得写进内核代码。
- UI（Tauri 常驻小窗 + 可展开工作区；状态镜子 = 知识相图）不进内核 crate。
- LLM 经 OpenAI-compatible endpoint。环境变量：`POLARIS_LLM_FAST_*` / `POLARIS_LLM_STRONG_*`（BASE_URL / MODEL / API_KEY）。
- **冻结参考（只读，禁止修改）**：`C:\MyProject\Polaris`、`C:\MyProject\Learned`。

## 5) 性能预算

Tier 0 读/决策 <10ms；掌握度更新（含 K≤64 线性代数）微秒级；HNSW 毫秒级；重活（巩固/报告/拟合）只在夜间离线跑。

## 6) 验收基线（每票必跑，完成声明必须附实跑输出）

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

外加当前票列明的集成命令与预期输出，以及 `git diff` 审查。

## 7) 对执行 AI 的禁止事项

- 不引入新顶层概念/命名——用 MASTER_PLAN 既有词汇（相、签名、摩擦、G_u、move、pack、Tier…）。
- 不在票范围外"顺手"实现未来阶段（例：P01 期间禁 θ/MIRT/嵌入/HMM/MRT/MCP/UI）。
- 不跳过验证门声称完成；不伪造或估算测试输出。
- 不 `git push`；commit 仅在票完成且验证全绿后进行，commit message 用中文。
- 不在内核写域特定逻辑；不改冻结仓库。
