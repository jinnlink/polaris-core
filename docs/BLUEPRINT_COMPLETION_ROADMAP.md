# Polaris 原始蓝图完整收口路线图

> 本文只回答“原始蓝图还剩什么、由哪张票完成”。设计意图仍以 `MASTER_PLAN.md` 为准，公式与 DDL 仍以 `DATA_MODEL.md` 为准，当前唯一可执行票仍以 `tickets/QUEUE.md` 为准。

## 1. 完成口径

状态使用四种语义：

- **已完成**：代码、契约或文档已经通过正式票验收。
- **底座已完成，产品面未完成**：核心能力存在，但普通学习者还不能通过正式桌面产品完整使用。
- **shadow / 待真实验证**：工程管线存在或即将建立，但数据门未通过，不得进入默认行为或确定性话术。
- **未实现**：仓库中没有对应产品能力，不能因为路线图、静态图或 DTO 存在而宣称完成。

最终完成必须同时满足：Core 权威状态、公开契约、Tauri 产品面、数据治理、真实验证和 Windows 发布链；任何一层缺失都不能把整项能力标成完成。

## 2. 蓝图能力矩阵

| 蓝图能力 | 当前实现证据 | 真实状态 | 剩余工作 / 唯一后续票 |
|---|---|---|---|
| 类型化知识超图 | P02A/P02B，`graph`、`diagnosis`、`concepts/edges` | 已完成底座 | P16B 形成可分页实时地图读模型；P17C 产品化呈现 |
| 嵌入与结构映射 | P03C/P03N，`geometry`，HNSW 候选与结构门 | 已完成底座 | P16C 把候选用于新域教学锚点和预测地图，仍受验证门约束 |
| 共享 θ/Q 潜因子 | P03A/P03M/P06G/P03O，shared/isolated θ | 已完成底座 | P16C 输出清晰区分观测、预测、继承先验的跨域地图 |
| 当前掌握度/知识相图 | `StatusSnapshot`、P03E、P07A/P07D | 底座已完成，正式地图未实现 | P16B + P17C |
| 跨域冷启动预测与教学锚点 | `latent_prediction`、几何候选、三示例 Pack | 未形成完整能力 | P16C；P18A 用真实首轮表现验证 |
| Global Learner Profile | MASTER_PLAN 已冻结；现有校准、G_u、MRT、节律事件可作证据 | 未实现统一画像、测量治理和消费门 | P16D 数据治理；P16E 估计/验证；P17E 用户控制；P18A 真实验证 |
| 三时间尺度学习者动力学 | 事件层与 HMM/hazard 已由 P03D/P03K 完成 | 状态层完成；慢特质层缺失 | P16D/P16E，不把短期状态解释为人格 |
| 教学策略、MRT、教法育种 | P04C/P05B/P10A | 已完成底座和透明出口 | P17E 展示；画像上下文仍需 P16E 双门 |
| 目标引擎 | P04D goals/dimensions/milestones | Core 薄封装完成，未接公开契约/产品面 | P16F + P17E |
| Capture/Inbox/Practice | P12C–P12E，CLI/HTTP/MCP | 已完成底座 | P17D 正式桌面工作台 |
| 外部导师审计回合 | P15A/P15B，MCP 双帧与 `task_event_id` | 已完成 | P12G 供 Coursebook 后端复用 |
| Tauri 常驻小窗与工作区 | P04A 仅交付状态镜子 DTO；仓库无 Tauri/Vite/Node 应用 | **未实现** | P17A–P17G |
| 开发者 Atlas | `docs/visuals/atlas` | P16A 收口后为已完成开发者文档 | 永不替代实时学习者地图；Tauri 仅继承视觉语言 |
| Concept Suggestion / overlay pack | Capture 可保存 raw evidence，Pack validator/sandbox 已有 | 未实现 | P12F；人工确认前不得修改正式 Pack 或 mastery |
| 课程壳桥接 | Coursebook 已存在于冻结 `Learned`，本仓库已有 MCP/项目声明 | 未实现 | P12G 在 `Learned` 另开正式票；Aura 不复活 |
| 真实纵向科学验证 | 模拟、单测和 shadow gate 较完整 | 测试不等于真实数据过门 | P18A；失败结果必须保留，禁止降门槛 |
| Windows 安装、更新与公开发布 | 尚无桌面工程或发布流水线 | 未实现 | P17G 建链；P18B 干净环境发布验收 |

## 3. 执行依赖

```text
P16A 蓝图/Atlas 事实源
  ├─ P16B 当前知识地图契约
  ├─ P16C 跨域预测地图
  ├─ P16D Global Profile 数据治理
  ├─ P16E Global Profile 估计与验证
  └─ P16F 目标产品契约
       ↓
P17A Tauri 底座
  → P17B Today/托盘
  → P17C 地图工作区
  → P17D 学习工作台
  → P17E 画像/目标/报告/信任/设置
  → P17F 生命周期与本地运维
  → P17G Windows 发布硬化
       ↓
P12F overlay pack → P12G Coursebook 跨仓桥接
       ↓
P18A 真实纵向验证 → P18B 1.0 发布验收
```

P12G 不由 `polaris-core` 执行 AI 直接修改冻结仓库。到达该阶段时，必须在 `C:\MyProject\Learned\rust-mastery-lab` 建立并批准该仓库自己的正式票。

## 4. 产品边界

- 正式桌面产品是 `polaris-core` 内的新 Tauri 应用；P04A 不是桌面应用完成证明。
- Aura 是历史命名与弃用参考。课程侧正式承接面是 Coursebook，桥接在 P12G 完成。
- Atlas 面向开发者，展示架构、数据表、票据与护栏；learner-mirror 是早期静态参考；二者都不是正式学习者工作区。
- SQLite 是唯一事实源。前端不复制掌握度、调度或画像真相。
- 预测、慢特质和干预效果未过门时必须显示 `unfit/shadow`；工程票完成不能改写科学状态。

## 5. 发布完成标准

1. 用户可从签名 Windows 安装包启动、常驻、手动更新、备份、恢复和卸载。
2. Today 永远给 2–3 个可选行动；心流态抑制通知；无 LLM/断网时 Tier 0 闭环仍可用。
3. 当前地图与预测地图清楚区分证据事实、模型预测和继承先验，并能回跳 provenance。
4. Profile 默认本地启用但透明可控，支持导出、仅画像重置和全部数据清除；未过门不影响 mastery。
5. 新 Pack 能展示预测、置信度、锚点和初始路径；isolated θ 不发生跨域继承。
6. Coursebook 回合复用 Polaris 调度和评分权威，不在课程壳复制 mastery 判断。
7. 全部正式票验收、Windows 干净环境 smoke 和 GitHub Releases 签名更新链通过后，才标记 1.0。
