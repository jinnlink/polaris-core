# P17B 常驻小窗与 Today

状态：Queued；依赖 P17A。

服务主命题：定位模糊 → 针对性补缺。

## 范围

- 实现 Windows 单实例、系统托盘、常驻小窗与展开工作区；关闭窗口默认隐藏到托盘，托盘“退出”才终止进程。
- Today 消费 Status、Mirror top signal、active Pack 和本地 scheduler，始终呈现 2–3 个合法行动；无足够任务时给“补证据/处理收件箱/休息”降级选择。
- 支持 Pack 切换与 shared/isolated 标识；切换后统一失效地图、Today、Goals 和 Inbox 查询。
- 心流 HMM 过门且当前为 flow 时抑制所有非错误通知；未过门只记录，不抑制。
- 小窗只读 Tier 0，打开路径不触发 LLM、网络或夜间任务。

## 禁区

- 不实现完整 Map/Practice/Profile 页面；不做单一强制行动。
- 不把 tray 生命周期写入 Core；不在同步打开路径跑重查询。

## 验收

- 单实例、托盘隐藏/恢复/退出、小窗/工作区切换、Pack 切换、2–3 行动、零任务和 flow 抑制测试。
- 冷启动 p95 与 Tier 0 命令预算记录；无 LLM/断网 smoke。
- 前端/桌面测试、SPEC §6 基线与 `git diff --check` 全绿。

## 回滚

回滚本票桌面壳与托盘事件；P17A 基础应用仍可启动。
