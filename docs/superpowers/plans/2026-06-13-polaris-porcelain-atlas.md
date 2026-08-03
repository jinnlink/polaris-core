# Polaris Porcelain Intelligence Atlas 全栈开发计划

> **历史设计说明（P16A，2026-08-03）：** 本文保留 2026-06-13 的 Atlas 设计与实现过程，文中的阶段快照、示例状态和未勾选步骤不是当前工程事实。Atlas 已在 P16A 纳入正式交付，其票据状态改为从 `docs/tickets/QUEUE.md` 动态生成；当前能力与执行状态只以 `docs/BLUEPRINT_COMPLETION_ROADMAP.md`、QUEUE 和生成校验结果为准。Atlas 是开发者文档，不是实时学习产品或 Tauri 应用。

> **面向 AI 代理的工作者：** 执行此计划时，建议使用 subagent-driven-development 或 executing-plans。步骤使用复选框语法跟踪进度；每一步都必须保持单票制，不修改 `C:\MyProject\Polaris` 与 `C:\MyProject\Learned` 冻结参考仓库。

**目标：** 构建一个位于 `docs/visuals/atlas/` 的本地静态交互式 Atlas，用高明度瓷白视觉、鼠尾草 accent、静谧动效，把 Polaris Core 的架构、模块、数据流、票据进度、验证护栏与源码关系讲清楚。

**架构：** 只读数据抽取脚本从仓库文档和源码生成 `atlas-data.json`，前端静态页消费该 JSON 并渲染模块树、架构图谱、Inspector、时间线与专题视图。核心产品 Rust 代码不参与运行时，不引入后端服务，保证产物可以直接打开或用本地静态服务器预览。

**技术栈：** Python 标准库数据抽取与校验、原生 HTML/CSS/JavaScript、SVG 架构图谱、Canvas 2D 背景字符场、CSS View Transitions、Scroll-driven Animations、OKLCH/color-mix、backdrop-filter、prefers-reduced-motion。

---

## 已锁定设计方向

- 风格名：Polaris Porcelain Intelligence Atlas / 瓷白智能图谱。
- 色彩：Radix Air Sage，高明度瓷白、浅鼠尾草、薄石墨线。
- 动效：Quiet Motion System / 静谧运动系统。
- 可借鉴小米 MiMo：慢速空间动效、高级留白节奏、连续浮现感。
- 不借鉴小米 MiMo：品牌橙、Logo、字形、营销海报结构、官网叙事。
- 核心原则：第一屏高级、进入细节后清楚、可搜索、可点击、可追溯。

## 文件结构

- 创建：`docs/visuals/atlas/index.html`
  静态站入口，包含应用壳、语义区域、首屏总览、无脚本降级内容。

- 创建：`docs/visuals/atlas/styles.css`
  设计 token、布局、响应式、动效、玻璃层、图谱节点、Inspector、时间线样式。

- 创建：`docs/visuals/atlas/app.js`
  前端状态管理、数据加载、搜索、过滤、节点选择、图谱渲染、Inspector 更新、动效调度。

- 创建：`docs/visuals/atlas/data/atlas-data.json`
  由脚本生成的 Atlas 内容数据，包含模块、文件、流程、数据表、票据、护栏、来源引用。

- 创建：`docs/visuals/atlas/scripts/build_atlas_data.py`
  只读抽取脚本，读取 `SPEC.md`、`docs/DATA_MODEL.md`、`docs/tickets/QUEUE.md`、`docs/MASTER_PLAN.md`、`crates/polaris-core/src/lib.rs`、`crates/polaris-cli/src/main.rs`、`crates/polaris-cli/src/mcp.rs`，生成前端数据。

- 创建：`docs/visuals/atlas/scripts/validate_atlas.py`
  校验 JSON 结构、文件链接存在性、票据状态一致性、未来 Phase 不被标成 completed。

- 创建：`docs/visuals/atlas/README.md`
  说明如何生成数据、预览页面、运行校验、更新内容、回滚。

- 保留：`docs/visuals/polaris-core-architecture.html` 与 `docs/visuals/polaris-core-architecture.svg`
  作为静态海报导出版，不作为主入口。

## 数据模型

`atlas-data.json` 顶层结构固定为：

```json
{
  "meta": {
    "project": "polaris-core",
    "generated_at": "ISO-8601 timestamp",
    "design": "Polaris Porcelain Intelligence Atlas",
    "phase_note": "P01-P03L completed; P04E and P05A1 queued"
  },
  "modules": [],
  "flows": [],
  "database": [],
  "tickets": [],
  "guardrails": [],
  "views": []
}
```

每个 `modules[]` 项包含：

```json
{
  "id": "engine",
  "title": "Engine",
  "group": "Core Loop",
  "summary": "Coordinates next task, evidence submission, mastery updates, and reporting boundaries.",
  "files": ["crates/polaris-core/src/engine.rs"],
  "depends_on": ["db", "scheduler", "mastery", "grader"],
  "related_tables": ["attempts", "mastery_states", "grade_queue"],
  "related_tickets": ["P01", "P02", "P03"],
  "source_refs": ["SPEC.md", "docs/DATA_MODEL.md"]
}
```

票据状态规则：

- P01 到 P03L 使用 `completed`。
- P04E 与 P05A1 使用 `queued`。
- Phase 4 / Phase 5 未实现能力使用 `planned` 或 `future`。
- 任何 planned/future 节点必须使用虚线、低明度标签，不得视觉上等同 completed。

## 任务 1：生成 Atlas 数据层

**文件：**
- 创建：`docs/visuals/atlas/scripts/build_atlas_data.py`
- 创建：`docs/visuals/atlas/scripts/validate_atlas.py`
- 创建：`docs/visuals/atlas/data/atlas-data.json`

- [ ] **步骤 1：编写 `build_atlas_data.py` 的只读抽取骨架**

实现要求：

```python
from __future__ import annotations

import argparse
import json
import re
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / "docs" / "visuals" / "atlas" / "data" / "atlas-data.json"


@dataclass
class ModuleNode:
    id: str
    title: str
    group: str
    summary: str
    files: list[str]
    depends_on: list[str]
    related_tables: list[str]
    related_tickets: list[str]
    source_refs: list[str]


def read_text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def parse_public_modules() -> list[str]:
    lib_rs = read_text("crates/polaris-core/src/lib.rs")
    return re.findall(r"^pub mod ([a-zA-Z0-9_]+);", lib_rs, flags=re.MULTILINE)


def build_data() -> dict:
    module_names = parse_public_modules()
    modules = [
        asdict(ModuleNode(
            id=name.replace("_", "-"),
            title=name,
            group="Core Module",
            summary=f"Polaris Core module exported from crates/polaris-core/src/lib.rs: {name}.",
            files=[f"crates/polaris-core/src/{name}.rs"],
            depends_on=[],
            related_tables=[],
            related_tickets=[],
            source_refs=["crates/polaris-core/src/lib.rs"],
        ))
        for name in module_names
    ]
    return {
        "meta": {
            "project": "polaris-core",
            "generated_at": datetime.now(timezone.utc).isoformat(),
            "design": "Polaris Porcelain Intelligence Atlas",
            "phase_note": "P01-P03L completed; P04E and P05A1 queued",
        },
        "modules": modules,
        "flows": [],
        "database": [],
        "tickets": [],
        "guardrails": [],
        "views": [],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    data = build_data()
    encoded = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != encoded:
            print("atlas-data.json is out of date")
            return 1
        print("atlas-data.json is current")
        return 0
    if args.write:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(encoded, encoding="utf-8")
        print(f"wrote {OUT}")
        return 0
    print(encoded)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **步骤 2：扩展数据抽取为 curated + parsed 混合模式**

在 `build_data()` 中补入人工 curated 映射，覆盖以下组：

```python
CURATED_GROUPS = {
    "Entrypoints": ["cli", "mcp"],
    "Core Loop": ["engine", "scheduler", "mastery", "grader"],
    "Knowledge": ["graph", "phase", "pack", "moves"],
    "Evidence": ["citation", "report", "diagnosis", "teaching"],
    "Cognitive": ["fsrs", "mirt", "mental_state", "mental_fit", "consolidation"],
    "Data": ["db", "config", "status"],
}
```

同时补入核心 flows：

```python
flows = [
    {
        "id": "learning-loop",
        "title": "Learning Loop",
        "steps": ["CLI/MCP next", "Engine", "Scheduler", "Attempt", "Grade Queue", "Mastery", "Report"],
        "summary": "The main loop from selecting a task to recording evidence and updating mastery."
    },
    {
        "id": "evidence-replay",
        "title": "Evidence Replay",
        "steps": ["op_log", "evidence_items", "attempts", "mastery_states"],
        "summary": "Local-persistent facts are replayable through append-only evidence and derived state."
    }
]
```

- [ ] **步骤 3：编写 `validate_atlas.py`**

校验项：

```python
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
DATA = ROOT / "docs" / "visuals" / "atlas" / "data" / "atlas-data.json"


def fail(message: str) -> None:
    raise SystemExit(message)


def main() -> int:
    data = json.loads(DATA.read_text(encoding="utf-8"))
    for key in ["meta", "modules", "flows", "database", "tickets", "guardrails", "views"]:
        if key not in data:
            fail(f"missing key: {key}")
    for module in data["modules"]:
        for file_path in module.get("files", []):
            if not (ROOT / file_path).exists():
                fail(f"missing module file: {file_path}")
    for ticket in data.get("tickets", []):
        if ticket.get("phase") in ["P04", "P05"] and ticket.get("status") == "completed":
            fail(f"future ticket marked completed: {ticket.get('id')}")
    print("atlas validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **步骤 4：运行数据生成与校验**

运行：

```powershell
python docs\visuals\atlas\scripts\build_atlas_data.py --write
python docs\visuals\atlas\scripts\build_atlas_data.py --check
python docs\visuals\atlas\scripts\validate_atlas.py
```

预期：

```text
wrote C:\MyProject\polaris-core\docs\visuals\atlas\data\atlas-data.json
atlas-data.json is current
atlas validation passed
```

## 任务 2：实现静态站应用壳

**文件：**
- 创建：`docs/visuals/atlas/index.html`
- 创建：`docs/visuals/atlas/styles.css`
- 创建：`docs/visuals/atlas/app.js`

- [ ] **步骤 1：创建 `index.html` 语义结构**

必须包含：

```html
<header class="topbar">
  <a class="brand" href="#overview" aria-label="Polaris Atlas home">Polaris Atlas</a>
  <label class="search">
    <span>Search</span>
    <input id="atlas-search" type="search" placeholder="Search modules, tickets, tables..." />
  </label>
  <nav class="view-tabs" aria-label="Atlas views">
    <button data-view="overview">Overview</button>
    <button data-view="graph">Graph</button>
    <button data-view="timeline">Timeline</button>
    <button data-view="data">Data</button>
    <button data-view="guardrails">Guardrails</button>
  </nav>
</header>

<main class="atlas-shell">
  <aside class="module-tree" aria-label="Module tree"></aside>
  <section class="atlas-canvas" aria-label="Architecture canvas"></section>
  <aside class="inspector" aria-label="Selected node details"></aside>
</main>

<canvas class="ambient-field" aria-hidden="true"></canvas>
```

- [ ] **步骤 2：定义 JS 状态与数据加载**

`app.js` 必须提供：

```javascript
const state = {
  data: null,
  selectedNodeId: null,
  query: "",
  view: "overview",
  reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
};

async function loadAtlasData() {
  const response = await fetch("./data/atlas-data.json");
  if (!response.ok) throw new Error(`Failed to load atlas-data.json: ${response.status}`);
  state.data = await response.json();
}

function selectNode(id) {
  state.selectedNodeId = id;
  renderInspector();
  renderGraph();
}

function startViewTransition(update) {
  if (document.startViewTransition && !state.reducedMotion) {
    document.startViewTransition(update);
  } else {
    update();
  }
}
```

- [ ] **步骤 3：运行静态预览**

运行：

```powershell
python -m http.server 4173 -d docs\visuals\atlas
```

预期：

```text
Serving HTTP on :: port 4173
```

## 任务 3：落地视觉系统

**文件：**
- 修改：`docs/visuals/atlas/styles.css`

- [ ] **步骤 1：写入设计 token**

```css
:root {
  color-scheme: light;

  --bg: #fbfdfc;
  --bg-subtle: #f7f9f8;
  --panel: #ffffff;
  --panel-muted: #f3fbf9;

  --text: #1a211e;
  --text-muted: #5f6563;
  --text-faint: #868e8b;

  --line: #e6e9e8;
  --line-strong: #d7dad9;
  --hover: #eef1f0;
  --selected: #e0f8f3;

  --accent: #12a594;
  --accent-hover: #0d9b8a;
  --accent-soft: #e0f8f3;
  --accent-line: #83cdc1;

  --tier-0: #12a594;
  --tier-1: #60b3d7;
  --tier-2: #e2a336;

  --data-db: #00749e;
  --data-event: #12a594;
  --data-model: #ab6400;

  --state-done: #12a594;
  --state-queued: #ffc53d;
  --state-planned: #8b8d98;
  --state-risk: #e54d2e;

  --font-ui: "IBM Plex Sans", "Noto Sans CJK SC", "Microsoft YaHei", system-ui, sans-serif;
  --font-mono: "JetBrains Mono", "Cascadia Mono", Consolas, monospace;
}
```

- [ ] **步骤 2：实现三栏布局**

桌面布局：

```css
.atlas-shell {
  min-height: calc(100vh - 64px);
  display: grid;
  grid-template-columns: minmax(240px, 280px) minmax(520px, 1fr) minmax(340px, 420px);
  gap: 1px;
  background: var(--line);
}

.module-tree,
.atlas-canvas,
.inspector {
  background: color-mix(in oklch, var(--panel) 88%, transparent);
}
```

移动布局：

```css
@media (max-width: 900px) {
  .atlas-shell {
    grid-template-columns: 1fr;
  }
  .module-tree,
  .inspector {
    min-height: 42vh;
  }
}
```

- [ ] **步骤 3：实现瓷白背景与细网格**

```css
body {
  margin: 0;
  color: var(--text);
  font-family: var(--font-ui);
  background:
    linear-gradient(var(--line) 1px, transparent 1px),
    linear-gradient(90deg, var(--line) 1px, transparent 1px),
    radial-gradient(circle at 20% 10%, rgba(18, 165, 148, 0.06), transparent 34rem),
    var(--bg);
  background-size: 96px 96px, 96px 96px, auto, auto;
}
```

## 任务 4：实现模块树、架构图谱与 Inspector

**文件：**
- 修改：`docs/visuals/atlas/app.js`
- 修改：`docs/visuals/atlas/styles.css`

- [ ] **步骤 1：模块树渲染**

`renderModuleTree()` 按 `module.group` 分组，节点点击调用 `selectNode(module.id)`。当前节点使用浅鼠尾草底、左侧 2px accent 线。

- [ ] **步骤 2：SVG 架构图谱渲染**

`renderGraph()` 生成：

```html
<svg class="graph" viewBox="0 0 1200 760" role="img" aria-label="Polaris architecture graph">
  <g class="edges"></g>
  <g class="nodes"></g>
</svg>
```

布局规则：

- Entrypoints 放上方。
- Core Loop 放中央。
- Data 放下方。
- Cognitive / Evidence / Knowledge 分列两侧。
- planned/future 节点使用虚线边框和低明度标签。

- [ ] **步骤 3：Inspector 详情**

`renderInspector()` 显示：

- 标题与 group。
- summary。
- files 列表。
- related_tables。
- related_tickets。
- source_refs。
- 当前节点相关 flow。

文件路径使用相对路径文本，不生成 `file://` URI。

## 任务 5：实现搜索、过滤与专题视图

**文件：**
- 修改：`docs/visuals/atlas/app.js`
- 修改：`docs/visuals/atlas/styles.css`

- [ ] **步骤 1：本地搜索**

搜索范围：

- module title / summary / files。
- ticket id / title / status。
- database table name。
- guardrail title。

搜索结果显示在左侧树上方，点击结果选中对应对象。

- [ ] **步骤 2：视图切换**

实现 `overview`、`graph`、`timeline`、`data`、`guardrails` 五个 view。每个 view 共享 Inspector 和搜索，不创建额外 HTML 页面。

- [ ] **步骤 3：票据时间线**

时间线规则：

- completed：实线、低饱和绿色点。
- queued：琥珀点、半实线。
- planned/future：灰色虚线。

P04/P05 未完成能力不得使用 completed 视觉。

## 任务 6：实现 Quiet Motion System

**文件：**
- 修改：`docs/visuals/atlas/app.js`
- 修改：`docs/visuals/atlas/styles.css`

- [ ] **步骤 1：Canvas 背景字符场**

`initAmbientField()` 使用 Canvas 2D 绘制极浅文本：

```javascript
const AMBIENT_TERMS = ["POLARIS", "CORE", "FSRS", "EVIDENCE", "MASTERY", "MCP", "SPEC", "QUEUE"];
```

运动规则：

- 每秒位移不超过 6px。
- 全局透明度不超过 0.08。
- `prefers-reduced-motion: reduce` 时绘制静态帧后停止 animation loop。

- [ ] **步骤 2：SVG 路径点亮**

当前节点变化时，相关 edge 添加 `is-active` 类：

```css
.edge.is-active {
  stroke: var(--accent);
  stroke-width: 1.6;
  stroke-dasharray: 10 8;
  animation: trace-path 900ms ease-out both;
}

@keyframes trace-path {
  from { stroke-dashoffset: 36; opacity: 0.2; }
  to { stroke-dashoffset: 0; opacity: 1; }
}
```

- [ ] **步骤 3：View Transition 与降级**

所有 view 切换必须走 `startViewTransition()`。减少运动模式下直接更新 DOM，不执行路径动画和背景漂移。

## 任务 7：可访问性、响应式与性能

**文件：**
- 修改：`docs/visuals/atlas/index.html`
- 修改：`docs/visuals/atlas/app.js`
- 修改：`docs/visuals/atlas/styles.css`

- [ ] **步骤 1：键盘可用**

要求：

- 搜索框可聚焦。
- view tabs 是真实 button。
- 模块树节点是 button。
- Escape 清空搜索或关闭临时结果。
- Enter 选中搜索结果。

- [ ] **步骤 2：移动端可读**

移动端改为单列，`view-tabs` 横向滚动。图谱允许横向 pan，不压缩到文字重叠。

- [ ] **步骤 3：性能预算**

预算：

- 首屏 JS 小于 120KB 未压缩。
- `atlas-data.json` 小于 300KB。
- Canvas animation 每帧只绘制背景字符，不读取布局。
- 不引入 npm 依赖。

## 任务 8：验证与交付

**文件：**
- 创建：`docs/visuals/atlas/README.md`
- 修改：`docs/visuals/atlas/scripts/validate_atlas.py`

- [ ] **步骤 1：静态校验**

运行：

```powershell
python docs\visuals\atlas\scripts\build_atlas_data.py --check
python docs\visuals\atlas\scripts\validate_atlas.py
```

预期：

```text
atlas-data.json is current
atlas validation passed
```

- [ ] **步骤 2：浏览器预览**

运行：

```powershell
python -m http.server 4173 -d docs\visuals\atlas
```

打开：

```text
http://localhost:4173/
```

检查：

- 首屏能看到 Polaris Atlas 总览。
- 搜索 `engine` 能定位模块。
- 搜索 `P04E` 显示 queued。
- 点击 `Engine` 后路径点亮，Inspector 显示文件和表。
- 减少运动模式下背景不漂移。
- 900px 以下宽度不出现文字重叠。

- [ ] **步骤 3：产品代码非侵入检查**

运行：

```powershell
git diff --stat
git diff -- crates
```

预期：

```text
docs/visuals/atlas/... changed
```

第二条命令不应显示 `crates/` 下的变更。

- [ ] **步骤 4：回滚方式**

如需回滚 Atlas：

```powershell
Remove-Item -Recurse -Force docs\visuals\atlas
```

如果只回滚计划：

```powershell
Remove-Item -Force docs\superpowers\plans\2026-06-13-polaris-porcelain-atlas.md
```

执行删除前必须确认目标路径位于 `C:\MyProject\polaris-core`。

## 完成标准

- `docs/visuals/atlas/index.html` 可通过本地静态服务器打开。
- Atlas 能展示模块树、架构图谱、Inspector、时间线、数据模型、验证护栏。
- 色彩使用 Radix Air Sage 高明度方案。
- 动效符合 Quiet Motion System，且支持 reduced motion。
- 数据来源可追溯到仓库文档、源码和票据。
- P04/P05 未完成能力不会被误标为 completed。
- 未修改 `crates/` 产品代码。
- 校验命令真实通过，并在交付说明中粘贴输出。
