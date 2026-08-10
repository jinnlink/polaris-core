# P16E Global Learner Profile 估计与验证

状态：已实现、通过验收并提交（`35fd61c`）；依赖 P16D（`7a478cc`）。

服务主命题：定位模糊 → 针对性补缺。

## 范围

- 行为层聚合全局校准倾向、G_u 模式、move 效果、会话节律和放弃前兆；状态层继续复用 HMM，不把短期状态当特质。
- 慢特质层按月更新 Big Five 相关分面、目标取向、归因倾向和自我效能后验；只使用注册量表与可审计行为证据。
- EMA 只在完成会话后触发：最多每日 1 题、每周 3 题；可跳过/暂停，心流态不触发。完整量表与分散 EMA 使用不同 admin_mode，EMA 不输出常模分数。
- 每个维度复用 `unfit/shadow/validated` 门语义。默认 A 类门：12 周、150 个相关结果、30 个有效会话、5 个时间前推留出折、logloss 改善 ≥0.01、Brier 不退化、改善后验概率 ≥0.95；跨域继承另需至少 3 Pack leave-one-pack-out。
- 画像只能初始化 C/策略/节律先验或作为 HMM/MRT 上下文；不得直接修改 mastery。任何画像驱动干预还必须过现有 MRT 门。

## 禁区

- 不降低或自动调优 A 类门；样本不足不得显示确定人格结论。
- 不把相关性说成因果，不让画像绕过评分/调度权威。
- 本票不实现 Tauri UI。

## 验收

- EMA 限频/跳过/暂停/心流抑制、月更新、partial instrument、样本不足、门失败/通过、数据漂移和决定性重放测试。
- 画像候选加入前后进行行为基线对拍；未过门任务序列与 mastery 完全不变。
- SPEC §6 基线、专项前瞻夹具、`git diff --check` 全绿。

## 回滚

关闭画像消费开关并回滚本票代码；原始测量事件保留为本地事实，派生状态可由事件重建。

## 交付记录（2026-08-09）

### 变更清单

- 新增 `profile_estimation`：聚合校准、G_u、move、会话节律与放弃前 hint；HMM 仅用于当次心流抑制，不转写为慢特质。
- 完成 session 后自动尝试 EMA：同 session 一题、每日 1 题、滚动 7 日 3 题，支持说明门、暂停、跳过、心流抑制与决定性 item 轮转。
- 月度按注册 item 反向计分并执行 fractional Beta 后验；完整量表、partial 与 EMA 分开留源，缺少注册证据的慢维度保持 `unfit` 无信息先验。
- 新增时间前推验证门：样本量、logloss、Brier、改善概率和跨域 Pack 数均读取 A 类参数；门状态支持 `unfit/shadow/active/suspended` 及漂移降级。
- 暴露 Engine 薄封装，补参数登记、数据模型公式与 7 组专项测试；未过门画像不接任务选择、评分或 mastery。

### 验收实跑

```text
> cargo fmt --all -- --check
exit 0

> cargo clippy --workspace --all-targets -- -D warnings
Checking polaris-core v0.1.0
Checking polaris-cli v0.1.0
Finished `dev` profile ...
exit 0

> cargo test -p polaris-core --test p16e_global_profile_estimation
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit 0

> cargo test --workspace
polaris-cli: 118 passed
polaris-core: 81 passed
p16e_global_profile_estimation: 7 passed
all discovered suites and doc-tests: 0 failed
exit 0

> git diff --check
exit 0
```

### 回滚

- 执行 `git revert 35fd61c` 移除估计器、EMA 触发、验证门、参数与文档；本票未新增 schema，既有 `profile_measurement` 原始事件保留，可在回滚前先关闭 `profile_settings.enabled` 停止画像采集。
