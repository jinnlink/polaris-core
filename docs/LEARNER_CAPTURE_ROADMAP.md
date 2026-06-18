# 无感学习入口与外部知识入库路线图

状态：v1，2026-06-18。

本文回答一个产品问题：一个零基础学生开始学习 Rust 和软件工程时，怎样自然使用 Polaris 这套学习底座，而不是先被 CLI、pack、SQLite、MCP、概念 ID 和参数名挡住。

结论很明确：`Learned` 是课程教室，不是学习边界。学习可以发生在任意地方。Polaris 的职责是把这些学习痕迹先收成可追溯证据，再把其中真正能证明理解的部分转成 attempt，进入掌握度、相图、调度和镜像报告。

## 1. 目标

本路线图服务主命题：

```text
验证真懂 -> 定位模糊 -> 针对性补缺
```

它补的是「学生能不能自然开始」这一层：

- 零基础学生只需要知道今天该做什么、刚学了什么、哪里卡住了。
- 仓库外学到的知识不能丢，先进入证据库。
- 读过、看过、收藏过、AI 讲过，不等于掌握。
- 只有学生自己的解释、预测、代码修改、测试修复或 teach-back，才可能影响掌握度。
- `polaris-core` 继续做本地学习引擎，`Learned` 和 Aura 做学生入口与课程产品。

## 2. 学生第一屏

学生不应该看到这些词：

- `pack`
- `concept_id`
- SQLite
- MCP
- HTTP
- `p_known`
- θ
- HMM
- strict-citation
- 相图英文枚举

第一屏只保留 3 个动作：

| 动作 | 学生理解 | 后台实际行为 |
|---|---|---|
| 继续今天 | 接着今天的课走 | 读取课程计划、due、弱项和收件箱，给 2 到 3 个可选任务 |
| 记录我刚学到的 | 把别处看到的东西存起来 | 写入 raw evidence，不直接影响掌握度 |
| 我卡住了 | 告诉系统现在需要帮助 | 写入 behavior event，进入状态镜像和后续调度 |

默认文案应该像学生助教，不像数据库控制台：

```text
今天先做一个小判断：这段代码会不会编译？
你可以选择：
1. 继续今天课程
2. 复习一个容易混的点
3. 记录刚在别处学到的东西
```

## 3. 入口边界

### Polaris Core

Polaris Core 负责：

- SQLite 本地事实源。
- `evidence_items`、`attempts`、`behavior_events`。
- 掌握度 fold、FSRS、BKT/MIRT、相图、HMM、G_u。
- 调度、评分、镜像报告、trust panel。
- HTTP/MCP/CLI 稳定入口。
- pack 权威源和 validator。

Polaris Core 不负责：

- Rust 课程文本。
- 桌面 UI 视觉壳。
- 学生日课叙事。
- 浏览器插件、截图 OCR、IDE 插件等具体传感器。
- 把外部 AI 评分当成掌握度权威。

### Learned / rust-mastery-lab

`C:\MyProject\Learned\rust-mastery-lab` 负责：

- Rust 和软件工程课程体验。
- `开工`、日课、预测题、练习、teach-back。
- Learning Aura 桌面伴随入口。
- 课程插图、教学场景、练习项目和 journal 视图。

长期目标：

- `Learned` 继续承载课程产品和人类可读视图。
- Polaris Core 成为结构化事实源。
- `labctl` 和 Aura 只是入口壳，不自行判断掌握度。

### 学习项目声明

学生不应该每次都回到 `polaris-core` 说「开工」。正确入口是：**每个学习项目自己声明已接入 P-OS**。学生打开 Rust、英语、生物等任意学习项目时，AI、Aura 或本地入口能自动发现该声明，并按当前项目启动。

项目声明不是 Domain Pack。两者职责不同：

| 层 | 文件 | 作用 | 学生是否需要理解 |
|---|---|---|---|
| 学习项目声明 | `p-os.toml` | 说明这个文件夹是一个学习现场，默认用哪个 pack、怎么开工、哪些路径可作为证据 | 不需要 |
| Domain Pack | `packs/<domain>/...` | 声明知识结构、概念、边、误解、rubric、moves | 不需要 |

推荐第一版在学习项目根目录放一个轻量 `p-os.toml`：

```toml
schema_version = 1
project_id = "rust-mastery-lab"
title = "Rust 与软件工程训练"
kind = "course"
default_pack = "rust"
default_entry = "today"

[entry]
start_label = "继续今天"
capture_label = "记录我刚学到的"
stuck_label = "我卡住了"
today_command = "cargo run -p labctl -- today --date {today}"

[evidence]
include = ["course/**", "exercises/**", "projects/**", "journal/**"]
ignore = ["target/**", ".git/**", "node_modules/**"]

[ui]
preferred_shell = "aura"
```

发现规则：

1. 从当前工作目录向上查找 `p-os.toml`。
2. 找到后，当前会话绑定到该学习项目。
3. `开工` 解析为该项目的 `entry.today_command` 或后续稳定入口。
4. `我刚学到...` 写入该项目关联的 capture queue。
5. 找不到声明时，不假装已经接入；只提示可以为当前项目创建 P-OS 声明。

这样学生学 Rust、英语、生物时，只需要打开对应项目。`开工` 是通用意图，但执行的是当前项目的入口。

## 4. 外部知识入库模型

仓库外知识必须「先进库，再判断」。

```text
外部来源
  -> capture
  -> raw evidence
  -> learner inbox
  -> 候选概念 / 候选误解 / 候选练习
  -> 学生确认或作答
  -> Engine::submit
  -> 掌握度 fold
```

### 可捕获来源

- `Learned` 的日课、journal、teach-back。
- 学生粘贴的网页链接、书摘、视频笔记。
- AI 对话片段。
- 终端报错、`cargo check`、`cargo test` 输出。
- Git diff、commit、代码片段。
- IDE 选中文本。
- 本地文件路径。
- 未来的浏览器分享、截图 OCR、课程平台导出。

第一版不需要一次做完所有来源。必须先把抽象层做对：任何来源都能变成 raw evidence，来源细节只放在 metadata 或 source 字段里。

## 5. 三层事实边界

### Raw evidence：先收下

Raw evidence 只说明「发生过或看到过」。它可以没有 `concept_id`，也可以暂时无法分类。

例子：

- 学生看了 Rust Book 所有权章节。
- 学生粘贴了一段 AI 对 borrow checker 的解释。
- 学生保存了一个编译错误。
- 学生贴了一段 Git diff。
- 学生说「这段我感觉有用」。

Raw evidence 不改掌握度，不改相图，不改调度权重。

### Suggestion：系统提出候选

Suggestion 是后台整理结果。

它可以提出：

- 这条证据可能属于已有概念。
- 这条证据可能暴露某个误解。
- 这条证据可以转成一道验证题。
- 这条证据可能需要新增个人 overlay pack 概念。

Suggestion 必须带 evidence id 和原文引用。它不过门时，只留在收件箱，不进入正式 pack，不影响掌握度。

### Mastery attempt：真正证明理解

Mastery attempt 必须满足：

- 有明确任务或题目。
- 有学生自己的产出。
- 在反馈前采集 `self_confidence`。
- 可归到已有概念或一个已确认的候选概念。
- 通过 `Engine::submit` 进入引擎自有评分与 fold。

例子：

- 「不看书，用自己的话解释 move 和 borrow 的区别。」
- 「预测这段代码是否编译，并说明原因。」
- 「根据失败测试修改代码，并解释为什么这样修。」
- 「把刚看的文章转成一个反例或最小代码实验。」

## 6. 绝不能自动写入掌握度的东西

以下内容最多作为 evidence 或 suggestion：

- 外部 AI 给出的 `score`、`final_score`、`mastery`、`confidence`。
- 看完视频、读完文章、收藏网页。
- 「我懂了」「这条断言对了」等自报。
- 没有 pre-feedback `self_confidence` 的历史 journal。
- AI 生成的摘要、类比、概念图。
- concept / edge / misconception suggestion。
- `pack validate` 或 `pack sandbox` 结果。
- `Learned` 的红黄绿、弱点标签、课程阶段完成状态。

掌握度只能由 attempt 经 engine-owned、evidence-bound scoring 更新。

## 7. 学习收件箱

学习收件箱是学生可见的缓冲层。它不展示内部表结构。

### 状态

| 状态 | 含义 | 学生可见文案 |
|---|---|---|
| `pending` | 已保存，未整理 | 已保存，稍后帮你整理 |
| `mapped` | 已匹配到候选概念 | 这可能和「所有权」有关 |
| `practice_ready` | 可以转成练习 | 要不要用它做一道小题？ |
| `practiced` | 已经生成并完成 attempt | 已练过 |
| `ignored` | 学生选择忽略 | 已忽略 |
| `archived` | 只归档，不再提醒 | 已归档 |

### 学生动作

每条收件箱条目最多给 3 个动作：

- 转成一道小题
- 稍后再看
- 忽略

不要让学生手动选择 pack、编辑 TOML、填概念 ID。维护者和 pack 作者才需要这些入口。

## 8. 关键体验流

### 流程 A：学生开工

```text
学生说「开工」
  -> 入口读取今天课程、due、弱项和 learner inbox
  -> 给 2 到 3 个任务选择
  -> 学生先预测或解释
  -> 采集 self_confidence
  -> 提交 attempt
  -> Polaris 更新状态
```

如果收件箱里有外部知识，入口只轻提示：

```text
昨天你保存了 2 条关于所有权的材料。
今天可以选：
1. 继续日课
2. 把其中一条变成小练习
3. 先复习一个到期点
```

### 流程 B：学生在别处学到知识

```text
学生粘贴链接、笔记、代码或报错
  -> 系统保存 raw evidence
  -> 只问一个轻问题：这是资料，还是你自己的回答？
  -> 不回答也不阻塞，先放 pending
  -> 后台生成候选映射
```

学生看到的是：

```text
已保存。
我会先把它当成资料，不会直接算作掌握。
之后可以把它变成一道小题。
```

### 流程 C：把外部资料转成练习

```text
学生点「转成一道小题」
  -> 系统基于 evidence 生成 prompt
  -> 学生作答
  -> 采集 self_confidence
  -> Engine::submit
  -> evidence id 绑定到 attempt
```

### 流程 D：候选新概念

如果材料无法映射到已有 pack：

```text
raw evidence
  -> candidate concept
  -> 个人 overlay pack 草案
  -> pack validate
  -> pack sandbox
  -> 用户或作者接受
```

这里的「个人 overlay pack」「pack validate」「pack sandbox」是后台或维护者术语，不出现在学生界面。学生只看到「这可能是一个新知识点，要不要先存着，之后再练」。

第一版可以只保留候选，不必自动写 pack。不要让 LLM 直接改正式 pack。

## 9. 视觉化承接

当前已有视觉资产分工如下：

| 资产 | 位置 | 面向对象 | 结论 |
|---|---|---|---|
| Learning Aura | `C:\MyProject\Learned\rust-mastery-lab\aura` | 学生 | 优先复用为学生入口 |
| stack-heap-ownership 教学场景 | `C:\MyProject\Learned\rust-mastery-lab\aura\public\teaching-scenes\stack-heap-ownership` | Rust 入门学生 | 适合做所有权第一批可视化 |
| learner mirror 静态面板 | `docs/visuals/learner-mirror` | 学生 / 维护者 | 可作为 Polaris 状态镜子参考 |
| atlas 架构图谱 | `docs/visuals/atlas` | 开发者 | 不作为学生入口 |
| architecture SVG/HTML | `docs/visuals/polaris-core-architecture.*` | 开发者 | 不作为学生入口 |

产品判断：

- 学生入口优先接 Aura，不从 atlas 开始。
- Polaris Core 的 `/learner-mirror`、`/next`、`/evidence`、`/feedback`、`/trust` 是 Aura 的数据面。
- Atlas 只用于维护者理解架构，不参与学生开工流。

## 10. 分阶段落地

### P12A：无感学习入口与外部知识入库规划

状态：本文档票。

交付：

- 本路线图。
- P12 实现计划。
- QUEUE 登记和后续票边界。

不改代码，不改数据库。

### P12B：学习项目声明 v1

目标：让每个学习项目显式声明已接入 P-OS，避免学生必须回到 `polaris-core` 开工。

范围：

- 定义 `p-os.toml` 最小协议。
- 提供项目发现和校验入口。
- 给出 Rust、英语、生物等多项目样例。
- 明确 project manifest 与 Domain Pack 的关系。

禁区：

- 不要求学生手写复杂配置。
- 不在 P12B 写 capture queue。
- 不修改 `Learned`，只提供协议和样例。

### P12C：Capture Queue v1

目标：让外部知识可以先入库，不生成 attempt。

范围：

- 新增最小 capture queue 数据结构或复用 `evidence_items` 加队列状态。
- CLI/HTTP 增加面向入口壳的 `capture` 写入语义。
- 返回 `recorded_only`，明确不影响掌握度。
- 验证外部评分字段被忽略。

禁区：

- 不做概念自动新增。
- 不做 GUI。
- 不改 `Learned`。

### P12D：Learner Inbox v1

目标：让学生能查看和处理捕获内容。

范围：

- 只读列出 pending/mapped/practice_ready。
- 支持 accept/defer/ignore/archive。
- HTTP/MCP/CLI 输出学生可读动作。

禁区：

- 不展示内部参数。
- 不让学生直接编辑 pack。

### P12E：Inbox Practice Bridge

目标：把收件箱条目转成可验证练习。

范围：

- 基于 evidence 生成 prompt 草案。
- 学生作答后采集反馈前 `self_confidence`。
- 调用现有 `Engine::submit`。

禁区：

- 不允许 raw evidence 直接改掌握度。
- 外部 AI 只提 prompt，不给最终掌握度。

### P12F：Concept Suggestion + Pack Patch 草案

目标：处理无法映射到已有概念的外部知识。

范围：

- 生成候选概念、边、误解。
- 每条 suggestion 带 evidence id 和 quote。
- 写入个人 overlay pack 草案。
- 强制 `pack validate` 和 `pack sandbox`。

禁区：

- 不直接修改正式 pack。
- 不绕过 strict-citation。

### P12G：Learned / Aura Bridge

目标：把学生入口接到真正可用的课程壳。

范围：

- `labctl` 或 Aura 调用 Polaris 的 capture/inbox/next/evidence/feedback。
- `开工` 时合并日课、due、弱项、收件箱。
- Aura 展示 3 个学生动作。

禁区：

- 不在 `labctl` 自己判断掌握度。
- 不修改 `C:\MyProject\Learned`，除非另开该仓库的正式票并取得写权限。

## 11. 第一版成功标准

第一版不是「全自动知识图谱」。第一版成功标准很朴素：

- 学生能说「开工」，看到今天 2 到 3 个合理选择。
- 学生在 Rust、英语、生物等任意学习项目中说「开工」，系统能通过 `p-os.toml` 识别当前学习项目。
- 学生能说「我刚学到了……」，系统保存，不丢失。
- 系统明确告诉学生：这只是资料，还不算掌握。
- 学生能把一条资料转成小练习。
- 只有完成小练习并采集反馈前 `self_confidence` 后，才进入掌握度。
- 学生不需要理解 pack、SQLite、MCP、HTTP 或内部参数。

## 12. 不做的事

- 不做跨设备同步。
- 不做完整浏览器插件。
- 不做全自动课程生成。
- 不做社交比较或排行榜。
- 不让外部 AI 评分直接进入 `mastery_states`。
- 不把 Rust 领域逻辑写进 core crate。
- 不把 atlas 当学生 UI。

## 13. 与现有文档关系

| 文件 | 关系 |
|---|---|
| `SPEC.md` | 宪法，任何冲突它赢 |
| `docs/DATA_MODEL.md` | 表结构与公式权威 |
| `docs/COURSE_INTEGRATION_PROTOCOL.md` | pack 与 adapter 边界 |
| `docs/API_CONTRACT.md` | HTTP/MCP 稳定公开面 |
| `docs/PRODUCT_ROADMAP.md` | 产品形态总路线图，本文是 2026-06-18 补充轴线 |
| `docs/tickets/QUEUE.md` | 单票执行队列 |
| `C:\MyProject\Learned\rust-mastery-lab\docs\evidence-schema.md` | Learned journal 与 Polaris attempts 的桥接参考 |
| `C:\MyProject\Learned\rust-mastery-lab\aura` | 学生入口 UI 参考 |
