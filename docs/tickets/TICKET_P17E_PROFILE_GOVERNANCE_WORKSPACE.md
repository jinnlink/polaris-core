# P17E 画像与治理工作区

状态：In Progress；依赖 P17C、P17D。

## 本轮范围（2026-08-12）

- 复用 P16D/P16E Global Profile、P16F Goals 与既有 Mirror/Reports/Trust/Privacy Core 契约，先补桌面只读快照和显式治理命令，不重写估计算法或门规则。
- Profile 只显示行为事实、带区间的慢特质后验、证据量、门状态、用途和不会影响项；未过门维度始终以假设/未验证呈现。
- 全部清除仅允许 Tauri 桌面显式调用，必须先返回范围预览，再以匹配确认词执行，并支持可选备份；普通 Web/API 不增加清除入口。
- 前端预计新增 Profile、Goals、Reports、Trust、Settings 工作区及共享治理反馈组件；先用契约测试锁定正常、跳过、暂停、关闭、反馈和删除恢复路径。

## 当前状态（2026-08-12）

- 已完成 Profile 第一条纵切：行为事实与慢特质后验分层，展示 95% 区间、证据数、门状态、用途和“不会影响什么”，不生成人格类型卡片。
- Desktop foundation 新增 shadow 非权威专项；前端新增 Profile 工作区与组件测试。当前 typecheck、lint、Vitest 19/19、foundation 13/13 和合同漂移检查全绿。
- 已完成 Goals 纵切：CRUD、目标范围、唯一维度、可选维度里程碑、无副作用派生进度、2–3 个合法行动、暂停/归档历史只读和确认删除；更新目标会保留既有进度与里程碑达成状态。
- Goals 纵切阶段 typecheck、lint、Vitest 21/21 和 Desktop 真实专项全绿；随后继续完成其余治理工作区。
- 已完成 Mirror/Reports/Trust：校准图、知识相、strict-citation、top signal、断言/假设/建议、反馈、风险门、F1–F5、MRT/育种和最近后台运行均来自权威 Core。
- 已完成 Settings 与删除治理：画像全生命周期、AI 互动偏好、量表/EMA admin_mode、Tier 0-only/隐私清单、导出、仅画像重置、全部清除范围预览/确认/备份/空库重开均为真实命令。
- 浏览器 1280px 与 720×512 等效 200% 实机检查无横向溢出，控制台 0 error / 0 warn；当前进入 SPEC §6 最终基线，P17E 继续保持 In Progress。

## 交付记录（2026-08-12）

### 变更清单

- 新增 Profile、Goals、Reports、Trust、Settings 五个完整工作区与开发预览数据；所有路由由懒加载真实页面替代占位页。
- 新增 Desktop DTO/command/state 组合层：画像事实与慢后验、目标 CRUD/进度/行动、镜像报告与反馈、五框架门、画像与 AI 设置、量表 EMA、导出/重置和桌面全量清除。
- 全量清除先返回 SQLite 文件与学习/证据/目标/画像/报告/行为事件范围；精确确认后可选备份，释放 Engine 连接并调用 Core 一致性恢复算法，最后重开空库。
- 增加 5 个页面组件测试、3 页 axe 检查和 Desktop 画像/目标/报告/信任/设置/删除专项；修复目标编辑清零存量进度与达成里程碑、暂停目标误调度、历史目标进度为零的问题。

### 验收实跑

- `cargo fmt --check`：exit 0。
- `cargo clippy --workspace --all-targets -- -D warnings`：exit 0，`Finished dev profile`。
- `cargo test --workspace`：exit 0；CLI 120/120、Core 83/83、Desktop foundation 17/17 与全部集成测试、doc-tests 全绿。
- `cargo test -p polaris-desktop --test foundation -- --nocapture`：17 passed / 0 failed；冷启动 p95=1.2323504s、Today p95=1.9513ms，票内性能断言通过。
- `pnpm --dir apps/desktop test`：12 files / 27 tests passed，含 Reports/Trust/Settings axe。
- `pnpm --dir apps/desktop typecheck`、`lint`、`contracts:check`：全部 exit 0。
- `pnpm --dir apps/desktop build`：exit 0，4641 modules，1.97s；Reports 29.11 kB、Settings 29.38 kB、Trust 13.63 kB，均独立懒加载。
- `git diff --check`：exit 0；仅报告现有 Windows CRLF 转换提示，无空白错误。
- 浏览器实机：1280px 全页核对 Reports/Trust/Settings；720×512 等效 200% 三页 `scrollWidth=705 < innerWidth=720`；控制台 0 error / 0 warn。

### 回滚方式

- 回滚本票 Desktop command/DTO/state、五个页面与样式、预览数据和 foundation 测试；Core 数据模型与既有 CLI/HTTP/MCP 不需回滚。
- 页面回滚不删除画像、目标、报告或学习数据；用户仍可经现有 Core/CLI 管理。全量清除是用户主动、精确确认的独立运行时动作，不会在代码回滚时自动执行。

当前状态：实现与全部验收已完成，等待用户确认后提交；尚未 commit、未 push。

服务主命题：全环节（学习者知情与控制）。

## 范围

- Profile 展示行为事实、慢特质后验、区间、证据数、门状态、用途和“不会影响什么”；禁止人格类型卡片。
- 会话结束按 P16E 限频呈现可跳过 EMA；提供完整量表入口并明确 admin_mode。
- Goals 完成 CRUD、维度/里程碑/进度和目标范围内 2–3 行动。
- Mirror/Reports/Trust 展示 strict-citation、top signal、F1–F5、MRT/breeding、画像门和最近后台运行；可反馈准确/不准。
- Settings 覆盖 Profile 开关/暂停/导出/仅画像重置/全部清除，AI Interaction Profile、Tier 0-only、隐私清单和本地集成分享。

## 禁区

- 未过门维度不得显示确定性解释或驱动行为。
- 清除操作不得经普通网页调用；全部清除必须范围预览、二次确认和可选备份。
- 不做社会比较、排行榜、暗模式或强迫量表。

## 验收

- profile 默认启用/首次说明/跳过/暂停/关闭/导出/重置/清除、shadow 文案、Goal 全流程、报告反馈和 Trust 门测试。
- 删除范围与 Core 测试对拍；原始回答不出现在 HTTP/MCP/日志。
- axe/键盘/视觉回归、前端/桌面测试、SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

回滚工作区页面与命令绑定；画像/目标数据保留，用户可继续经 CLI/Core 管理。
