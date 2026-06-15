# P06G theta AdaGrad 步长实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:test-driven-development。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将 MIRT theta 在线更新从固定步长改为每维 AdaGrad 自适应步长。

**架构：** `theta` 表随 `vec` 一起持久化 `g2` 二阶矩向量；`mirt::update_theta_for_attempt` 在 final score 到达时累积梯度平方并用 `eta / sqrt(g2 + epsilon)` 缩放。`theta_history` 仍只快照 `vec`，保持历史预测语义不变。

**技术栈：** Rust、rusqlite、现有 `encode_vector` / `decode_vector` f32 BLOB 编码、现有参数登记处。

---

## 文件结构

- 修改 `crates/polaris-core/tests/p03a_mirt.rs`：新增红灯测试，覆盖 `theta.g2` 初始化与重复更新步长衰减。
- 修改 `crates/polaris-core/src/db.rs`：`theta` 表新增 `g2` 列，并通过 `ensure_column` 支持旧库迁移。
- 修改 `crates/polaris-core/src/mirt.rs`：扩展 `MirtParams`，初始化和读取 `g2`，在更新中应用 AdaGrad。
- 修改 `crates/polaris-core/src/config.rs`：登记 `mirt.adagrad_epsilon`。
- 修改 `docs/DATA_MODEL.md`：同步 MIRT 更新公式与参数表。
- 修改 `docs/tickets/QUEUE.md`：认领 P06G。
- 修改 `docs/tickets/TICKET_P06G_THETA_ADAGRAD.md`：完工后追加交付记录。

### 任务 1：红灯测试

**文件：**
- 修改：`crates/polaris-core/tests/p03a_mirt.rs`

- [ ] **步骤 1：写失败测试**

新增测试：

```rust
#[test]
fn init_pack_initializes_theta_adagrad_accumulator() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let (theta_blob, g2_blob): (Vec<u8>, Vec<u8>) = engine
        .conn()
        .query_row("SELECT vec, g2 FROM theta WHERE id=1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let theta = decode_vector(&theta_blob).unwrap();
    let g2 = decode_vector(&g2_blob).unwrap();

    assert_eq!(g2.len(), theta.len());
    assert!(g2.iter().all(|value| *value == 0.0));
}
```

新增重复更新测试，断言第二次同维增量小于第一次。

- [ ] **步骤 2：运行红灯**

运行：

```powershell
cargo test -p polaris-core --test p03a_mirt init_pack_initializes_theta_adagrad_accumulator
```

预期：FAIL，当前 schema 没有 `theta.g2`。

### 任务 2：最小实现

**文件：**
- 修改：`crates/polaris-core/src/db.rs`
- 修改：`crates/polaris-core/src/mirt.rs`
- 修改：`crates/polaris-core/src/config.rs`

- [ ] **步骤 1：补 schema 和参数**

`theta` 建表增加 `g2 BLOB`；迁移流程调用 `ensure_column(conn, "theta", "g2", "BLOB")`；参数登记新增 `mirt.adagrad_epsilon`。

- [ ] **步骤 2：补 MIRT 状态读写**

`ensure_theta` 初始化 `vec` 与 `g2` 零向量；若旧库 `g2` 为空或长度错误，回填零向量。

- [ ] **步骤 3：实现 AdaGrad 更新**

在 `update_theta_for_attempt` 中：

```rust
let gradient = residual * q_k;
*g2_k += gradient * gradient;
let delta = (params.eta * gradient / (*g2_k + params.adagrad_epsilon).sqrt())
    .clamp(-params.step_cap, params.step_cap);
*theta_k += delta;
```

### 任务 3：绿灯与文档

**文件：**
- 修改：`docs/DATA_MODEL.md`
- 修改：`docs/tickets/QUEUE.md`
- 修改：`docs/tickets/TICKET_P06G_THETA_ADAGRAD.md`

- [ ] **步骤 1：运行专项测试**

```powershell
cargo test -p polaris-core --test p03a_mirt
```

- [ ] **步骤 2：同步文档**

`DATA_MODEL.md` §4 将固定步长公式改为 AdaGrad 公式，§10 参数表加入 `mirt.adagrad_epsilon`。

- [ ] **步骤 3：最终验收**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --test p03a_mirt
git diff --check
```

- [ ] **步骤 4：交付记录**

在票尾写入变更清单、红灯输出、验收输出、技术选择说明和回滚方式。
