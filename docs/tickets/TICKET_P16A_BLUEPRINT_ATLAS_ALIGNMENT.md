# P16A 蓝图与 Atlas 收口

状态：已实现、通过验收并提交（`fbcad7b`）。

服务主命题：全环节（让“已实现、未实现、实验中”成为可审计工程事实，避免产品能力被错误宣称或遗漏）。

## 背景

用户已批准“Polaris 原始蓝图完整收口计划”。当前 Core 底座已经完成大量 Phase 1–15 能力，但产品文档和开发者 Atlas 仍混有早期快照：P04A 的标题容易被误读为已交付 Tauri，Atlas 生成器把已完成的 P04/P05 硬编码成 future，旧路线图仍把已弃用 Aura 写成学生主入口。

本票先统一工程事实源并登记后续单票，不实现知识地图、Global Learner Profile 或桌面端本身。

## 范围

1. 新增原始蓝图完成路线图：逐项列出蓝图能力、当前实现证据、真实状态、缺口和唯一后续票。
2. 明确产品边界：
   - P04A 只交付 Tier 0 状态镜子契约，没有交付 Tauri 应用。
   - 正式桌面端在 `polaris-core` 新建；Aura 仅为冻结、弃用参考，不复活。
   - P12G 的学生课程壳改为 Coursebook，且必须另开 `Learned` 仓库正式票取得写权限。
   - Atlas 是开发者架构图谱，不是学习者实时知识地图。
3. 把已存在但未纳入版本控制的 Porcelain Atlas 作为本票交付审计后纳入仓库；生成器从 `docs/tickets/QUEUE.md` 动态抽取阶段、票号、标题与状态，删除 P04/P05 的历史硬编码判断。
4. 更新 Atlas 状态标签和说明，支持 `in_progress`，并让生成数据与当前仓库一致。
5. 在 QUEUE 登记 P16B–P18B 的执行序和依赖；同一时刻仍只有 P16A 为 In Progress。

## 禁区

- 不新增或修改 Rust 产品行为、数据库、公开 API、数学公式或参数。
- 不创建 Tauri/Node 项目；P17A 才负责桌面底座。
- 不把 Atlas 或 learner-mirror 宣称为正式学习者 UI。
- 不修改冻结仓库 `C:\MyProject\Polaris`、`C:\MyProject\Learned`。
- 不混入现有 `.gitignore`、漫画文档、`.codex/`、`.cursor/`、`docs/jieshou.txt`、architecture HTML/SVG、临时数据库或 review target。

## 预计修改面

- `docs/BLUEPRINT_COMPLETION_ROADMAP.md` 与既有产品/采集路线图。
- `docs/tickets/QUEUE.md`、本票及后续正式票规格。
- `docs/visuals/atlas/` 和其设计计划；只收口开发者 Atlas，不改 learner-mirror。

## 验收

```powershell
python docs\visuals\atlas\scripts\build_atlas_data.py --write
python docs\visuals\atlas\scripts\build_atlas_data.py --check
python docs\visuals\atlas\scripts\validate_atlas.py
node --check docs\visuals\atlas\app.js
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

另需通过本地静态服务器做真实页面检查：首屏加载、架构图、时间线、数据模型、护栏、搜索、语言切换均可用；实施阶段 P16A 显示为进行中、P15B 显示为完成，收尾后 P16A 随 QUEUE 自动切换为完成。

## 回滚方式

回滚本票提交即可恢复。手工回滚时删除本票新增路线图与未来票文件，还原 QUEUE、产品/采集路线图，并移除本票纳入的 `docs/visuals/atlas/`；不涉及数据库或业务数据回滚。

## 本轮范围（2026-08-03）

- 仅执行上面的文档、票据与开发者 Atlas 事实收口。
- P16B 以前不新增知识地图 DTO/API；P17A 以前不新增 Tauri/npm 依赖。
- 开工前已确认 Atlas 当前 `validate_atlas.py` 通过、`build_atlas_data.py --check` 失败，原因是生成器仍保留早期硬编码状态。

## 交付记录（2026-08-03）

### 变更清单

- 新增 `docs/BLUEPRINT_COMPLETION_ROADMAP.md`，建立蓝图能力—当前证据—真实状态—唯一后续票矩阵，并明确工程完成不等于科学门通过。
- 在 QUEUE 正式登记 P16B–P18B 与 P12F，冻结依赖顺序；P12G 只作为 `Learned` 仓库外部依赖门。
- 新增 P16B–P18B、P12F 的正式票规格，逐票写明范围、禁区、验收与回滚。
- 纠正 P04A/Tauri、Aura/Coursebook、Atlas/实时学习地图的历史边界；未修改冻结仓库。
- 纳入 `docs/visuals/atlas/` 与历史设计说明；生成器改为从 QUEUE 动态提取阶段、票名和 `completed/in_progress/queued` 状态，校验器同步验证唯一进行中票。
- 修复 Atlas 在 1280px 宽度下英雄区横向溢出；保持开发者文档定位，不引入产品前端或 Node 依赖。

### 真实验收输出

```text
python docs/visuals/atlas/scripts/build_atlas_data.py --write
wrote C:\MyProject\polaris-core\docs\visuals\atlas\data\atlas-data.json

python docs/visuals/atlas/scripts/build_atlas_data.py --check
atlas-data.json is current

python docs/visuals/atlas/scripts/validate_atlas.py
atlas validation passed

node --check docs/visuals/atlas/app.js
exit 0（无输出）

cargo fmt --check
exit 0（无输出）

cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.75s

cargo test --workspace
436 passed; 0 failed

git diff --check
exit 0（无错误；仅现有 Windows CRLF 提示）
```

浏览器实测使用本地静态服务器和 1280×720 窗口：总览、图谱、时间线、数据模型、护栏、搜索和中英文切换均可用；图谱渲染 46 个节点；实施阶段时间线正确显示 P15B“已完成”和 P16A“进行中”；修复后中央画布 `scrollWidth == clientWidth == 547`，无横向溢出，控制台 0 错误。收尾将 QUEUE 中 P16A 标为完成后，重新生成数据使最终 Atlas 同步为 completed。

### 回滚

回滚本票提交即可；手工回滚按“回滚方式”一节删除新增路线图、未来票和 Atlas，恢复 QUEUE 与两份历史路线图。本票没有数据库迁移、产品行为或用户数据变更。
