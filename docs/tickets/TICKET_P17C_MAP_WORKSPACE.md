# P17C 知识地图工作区

状态：In Progress（实现与本地验收已完成，用户已确认保留第二版视觉，等待精准提交）；依赖 P17B、P16C。

服务主命题：定位模糊。

## 范围

- 实现 active Pack 当前地图、跨域预测地图和全局 Pack/维度聚合三种视图。
- 使用 Cytoscape.js 渲染分页子图；schema 作为节点，typed edge 保持方向与类型。大图按 viewport/root/depth 增量加载，不一次拉取全部概念。
- 节点视觉明确区分会/模糊/未学、观测/预测/先验、置信区间、相和到期；低置信预测使用虚线/淡化且有文字标签。
- 提供搜索、相/到期/置信度筛选、键盘导航、节点 Inspector、证据/provenance 抽屉和“开始练习/加入目标”动作。
- 地图只读；所有状态变化只能经正式练习/证据闭环发生。

## 禁区

- 不允许拖拽后保存知识结构或手工标记已掌握。
- 不用前端重新计算 p_known、相、预测或边门。
- 动效必须服从 reduced-motion，不牺牲 200% 缩放和高对比。

## 验收

- 当前/预测/全局、分页、筛选、搜索、证据回跳、isolated Pack、低置信和 10k 节点虚拟化测试。
- axe/键盘/缩放/视觉回归；真实 Tauri 数据 smoke。
- 前端/桌面测试、SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

移除 Map 页面与图形依赖；P16B/P16C 只读契约保留。

## 本轮范围（2026-08-09）

- 只实现当前 Pack、跨域预测、全局聚合三视图，以及分页子图、筛选搜索、节点 Inspector、provenance 和正式动作入口。
- 图渲染只消费 P16B/P16C Core 契约，不在前端计算 `p_known`、相、预测、置信区间或边门，不提供拖拽保存与手工掌握。
- 大图只保留当前分页/根节点邻域并增量加载；专项门覆盖 isolated/低置信、10k 分页、键盘/缩放/reduced-motion/高对比和真实 Tauri 数据。

## 交付记录（2026-08-10）

### 用户确认与冻结基线

- 2026-08-10，用户确认第二版地形图谱“没什么问题，就先保留”，P17C 视觉确认门通过。
- 冻结视觉基线为矿物蓝 / 氧化铜 / 赭黄 / 羊皮纸的地形知识图谱；产品资产以 `apps/desktop/src/assets/topographic-atlas-v2.jpg` 和 Map 工作区实现为准。
- 冻结产品构成包括：当前 / 预测 / 全局三种不同信息结构、节点状态与 typed edge、Inspector 决策台、证据回跳、Today 概念交接、Practice / Goals 行动入口、分页与键盘等价路径。
- 后续票可以复用该设计语言，但不得以统一换肤替代各工作区的信息任务；P17D 仍须按学习工作台票据实现完整练习闭环。
- 根目录的浏览器截图、同屏对照和设计探索记录属于本地 QA 证据，不作为运行时依赖，也不进入 P17C 产品提交。

### 变更清单

- Tauri/Core：增加 `map_workspace` 只读命令与 current/prediction/global DTO 映射，保留 P16C anchors、paths、theta mode 和 Core 权威计算；不在前端重算学习状态。
- Map 工作区：以用户选定的地形图谱方案实现三视图、分页子图、搜索筛选、两跳聚焦、Inspector、证据回跳、正式 Practice/Goal 动作与键盘等价列表。
- 视觉系统：矿物蓝 / 氧化铜 / 赭黄 / 羊皮纸，节点状态、typed edge、低置信、Schema 与三层预测均有文字和视觉双重编码；全局视图保持低密度聚合。
- 稳健性：开发浏览器 fixture 仅在非 Tauri DEV 环境生效；Tauri runtime 继续读取真实 SQLite/Engine。补 reduced-motion、forced-colors 与 200% 缩放规则。
- 加载性能：Map 工作区按路由懒加载，Cytoscape 与地图实现不进入 Today 等主入口首包；生产构建主入口 341.81 kB，地图独立 chunk 498.22 kB。
- QA：初版视觉通过结论已撤销并重新审计；第二版 `design-qa.md`、1487×1058 同尺寸对照、决策台局部对照与产品闭环审计均已生成，最终 `final result: passed`。
- 产品闭环：Today 的 `concept` 交接会直接选中地图节点；全局命令入口覆盖 Inbox/Profile/Reports/Trust/Settings；Practice/Goals/Evidence 路由继续携带概念或证据上下文。

### 验收实跑

```text
pnpm build
✓ built in 754ms；主入口 341.81 kB，Map 独立 chunk 498.22 kB，第二版局部地形纹理产物 302.78 kB

pnpm typecheck
exit 0

pnpm lint
exit 0，0 warnings

pnpm test
Test Files  5 passed (5)
Tests       13 passed (13)

pnpm contracts:check
generate-contracts --check exit 0

cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test foundation map_workspace_preserves_current_prediction_and_global_contracts -- --exact --nocapture
test result: ok. 1 passed; 0 failed; 8 filtered out

cargo fmt --check
exit 0

cargo clippy --workspace --all-targets -- -D warnings
Finished dev profile；exit 0

cargo test --workspace --quiet
workspace_test_exit=0；全部测试组 0 failed

git diff --check
exit 0（仅 Windows LF/CRLF 提示）
```

浏览器实机：Today → Map 概念交接、当前/预测/全局、完整产品命令、筛选、键盘节点、证据、缩放/Fit 均通过；控制台 0 error / 0 warn。744×529 等效 200% 缩放下 `scrollWidth=744`、`innerWidth=744`，主动作仍可达。

### 回滚方式

- 移除 Map 路由页面、地图样式、地形纹理和 Cytoscape/Phosphor 前端依赖。
- 移除 Desktop `map_workspace` 命令及对应 DTO/状态映射；P16B/P16C Core 只读契约与数据库数据不变。
- 本票没有写入学习状态，也没有结构编辑或手工掌握入口，因此回滚不需要数据迁移。
