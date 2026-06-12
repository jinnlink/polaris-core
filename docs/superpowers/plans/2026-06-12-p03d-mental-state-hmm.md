# P03D 状态 HMM 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 按 `docs/DATA_MODEL.md` §7 建立 attempt 级状态 HMM 与 hazard 门控记录层。

**架构：** 新增 `mental_state` 纯 Rust 模块承载 HMM、feature、hazard 公式；`Engine::submit` 在现有乐观更新路径内生成 `behavior_events.type='mental_state'` 快照。P03D v1 只记录状态与风险，未过 AUC 门前不调度、不进报告。

**技术栈：** Rust、rusqlite、serde/serde_json、现有 `behavior_events` 事件日志。

---

## 文件结构

- 创建：`crates/polaris-core/src/mental_state.rs`
  - 定义 `MentalState`、`HmmObservation`、`StatePosterior`、`MentalStateSnapshot`、`HazardEstimate`。
  - 实现先验 HMM 发射、转移、前向滤波、hazard logistic 与门控。
- 修改：`crates/polaris-core/src/lib.rs`
  - 导出 `mental_state` 模块。
- 修改：`crates/polaris-core/src/engine.rs`
  - 在 `submit` 中记录 `mental_state` 行为事件。
  - 增加内部 feature 查询函数与公开 latest snapshot 查询函数。
- 创建：`crates/polaris-core/tests/p03d_mental_state.rs`
  - 覆盖 HMM 公式、hazard 门控、submit 行为事件、调度不变性。
- 修改：`docs/tickets/TICKET_P03D_MENTAL_STATE_HMM.md`
  - 票尾写真实验收输出。
- 修改：`docs/tickets/QUEUE.md`
  - 完工后把 P03D 标记完成。

## 任务 1：纯 HMM 与 hazard 公式

**文件：**
- 创建：`crates/polaris-core/src/mental_state.rs`
- 修改：`crates/polaris-core/src/lib.rs`
- 测试：`crates/polaris-core/tests/p03d_mental_state.rs`

- [ ] **步骤 1：编写失败的公式测试**

```rust
use polaris_core::mental_state::{
    estimate_hazard, forward_filter, HazardInputs, HmmObservation, MentalState,
};

#[test]
fn hmm_prior_emission_distinguishes_flow_and_frustration() {
    let flow = forward_filter(
        None,
        HmmObservation {
            z_latency: -0.5,
            hints: 0.0,
            residual: 0.10,
            consec_fail: 0.0,
            conf_delta: 0.2,
            interval_bucket: 0.0,
            session_min: 5.0,
        },
    );
    assert_eq!(flow.dominant_state(), MentalState::Flow);
    assert!((flow.probabilities.iter().sum::<f64>() - 1.0).abs() < 1e-9);

    let frustrated = forward_filter(
        None,
        HmmObservation {
            z_latency: 1.0,
            hints: 1.5,
            residual: -0.30,
            consec_fail: 2.5,
            conf_delta: -0.5,
            interval_bucket: 1.0,
            session_min: 25.0,
        },
    );
    assert_eq!(frustrated.dominant_state(), MentalState::Frustrated);
}

#[test]
fn hazard_requires_auc_gate_before_participating() {
    let posterior = forward_filter(
        None,
        HmmObservation {
            z_latency: 1.0,
            hints: 1.5,
            residual: -0.30,
            consec_fail: 2.5,
            conf_delta: -0.5,
            interval_bucket: 1.0,
            session_min: 25.0,
        },
    );
    let beta = [2.0; 12];
    let low_auc = estimate_hazard(
        HazardInputs::new(&posterior, 0.2, 2.0, 0.6, 0.0, 1.0, 25.0),
        &beta,
        Some(0.69),
        0.70,
    );
    assert!(!low_auc.participates);

    let passing_auc = estimate_hazard(
        HazardInputs::new(&posterior, 0.2, 2.0, 0.6, 0.0, 1.0, 25.0),
        &beta,
        Some(0.70),
        0.70,
    );
    assert!(passing_auc.participates);
    assert!(passing_auc.probability > 0.5);
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -p polaris-core --test p03d_mental_state hmm_prior_emission_distinguishes_flow_and_frustration`

预期：FAIL，原因是 `p03d_mental_state` 测试或 `mental_state` 模块尚不存在。

- [ ] **步骤 3：实现最少公式代码**

实现 `MentalState`、均值表、对角高斯 log emission、0.7/0.06 转移、归一化、hazard logistic。`interval_bucket` 和 `session_min` 本票先进入 observation/hazard payload；HMM 发射只使用 DATA_MODEL 明确给出数值均值的前 5 维，避免发明未冻结常数。

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -p polaris-core --test p03d_mental_state hmm_prior_emission_distinguishes_flow_and_frustration hazard_requires_auc_gate_before_participating`

预期：两个测试 PASS。

## 任务 2：submit 记录 mental_state 事件

**文件：**
- 修改：`crates/polaris-core/src/engine.rs`
- 测试：`crates/polaris-core/tests/p03d_mental_state.rs`

- [ ] **步骤 1：编写失败的集成测试**

```rust
#[test]
fn submit_records_mental_state_snapshot_without_enabling_strategy_or_hazard() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    polaris_core::db::migrate(&conn).unwrap();
    let mut engine = polaris_core::engine::Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let before = engine.next_task().unwrap().unwrap().concept_id;
    let receipt = engine
        .submit(polaris_core::engine::SubmitInput {
            session_id: "s1".to_owned(),
            concept_id: "ownership".to_owned(),
            task_type: "recall".to_owned(),
            prompt_text: "Explain ownership.".to_owned(),
            response_text: "Ownership controls drops.".to_owned(),
            self_confidence: 2,
            latency_ms: 2500,
            hint_count: 5,
        })
        .unwrap();

    let payload: String = engine
        .conn()
        .query_row(
            "SELECT payload_json FROM behavior_events
             WHERE type='mental_state' AND concept_id='ownership'
             ORDER BY at DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(json["attempt_id"], receipt.attempt_id);
    assert_eq!(json["features"]["hints"], 3.0);
    assert_eq!(json["strategy_enabled"], false);
    assert_eq!(json["hazard"]["participates"], false);
    assert_eq!(
        json["posterior"].as_array().unwrap().len(),
        polaris_core::mental_state::STATE_COUNT
    );

    let after = engine.next_task().unwrap().unwrap().concept_id;
    assert_eq!(after, before, "P03D record-only layer must not steer scheduling");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -p polaris-core --test p03d_mental_state submit_records_mental_state_snapshot_without_enabling_strategy_or_hazard`

预期：FAIL，原因是 submit 尚未写 `mental_state` 行为事件。

- [ ] **步骤 3：实现最少接入**

在 `submit` 中保存 attempt 后、`replay_concept` 前读取 pre-attempt `p_hat`、个人 latency/confidence baseline、连续失败、session 分钟等特征，调用 HMM/hazard，写入 `behavior_events.type='mental_state'`。读取 `hmm.gate_auc_margin` 和 `hazard.auc_gate`，但默认无验证 AUC，因此只记录不启用。

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -p polaris-core --test p03d_mental_state submit_records_mental_state_snapshot_without_enabling_strategy_or_hazard`

预期：PASS。

## 任务 3：验收与交付记录

**文件：**
- 修改：`docs/tickets/TICKET_P03D_MENTAL_STATE_HMM.md`
- 修改：`docs/tickets/QUEUE.md`

- [ ] **步骤 1：运行 P03D 专项验收**

运行：`cargo test -p polaris-core --test p03d_mental_state`

预期：全部 PASS。

- [ ] **步骤 2：运行 SPEC §6 基线**

运行：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

预期：全部 exit 0。

- [ ] **步骤 3：写票尾交付记录**

在 `docs/tickets/TICKET_P03D_MENTAL_STATE_HMM.md` 追加：

```markdown
## 交付记录（2026-06-12）

- 变更清单：
- 验收输出：
- 技术选择：
- 阻塞与裁决：
- 回滚方式：
```

- [ ] **步骤 4：更新队列**

把 `docs/tickets/QUEUE.md` 状态从 `P03D In Progress` 改为 `P03D 已完成，等待提交后认领下一票`，并把 P03D 勾选为完成。

- [ ] **步骤 5：提交**

只 stage 本票相关文件，避免把用户的 P05A0/漫画文档混入本票提交。
