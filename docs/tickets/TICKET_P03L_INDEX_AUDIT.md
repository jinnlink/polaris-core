# P03L 索引审计 (Index Audit)

状态：Completed

服务主命题环节：全环节（Tier 0 <10ms 预算铁律的结构保障，SPEC §5 / DATA_MODEL §11）

## 背景

全库目前没有任何 CREATE INDEX。热路径全部依赖全表扫描：attempts 按 (concept_id, created_at) 的 fold/重放与按 created_at 的时间序装载（P03J 调参、P03I 报告）；behavior_events 按 (type, at) 与 (session_id, type) 的事件挖掘（报告/心智状态/G_u）；`json_extract(payload_json,'$.attempt_id')` 的快照回查；gu_rules 按 status 的规则查询；edges 按 src/dst 的邻域展开。数据量增长后这些会线性恶化并威胁 <10ms 预算。

## 范围

1. `db.rs::migrate()` 增加幂等索引（`CREATE INDEX IF NOT EXISTS`）：
   - `idx_attempts_concept_created` ON attempts(concept_id, created_at)
   - `idx_attempts_created` ON attempts(created_at)
   - `idx_behavior_type_at` ON behavior_events(type, at)
   - `idx_behavior_session_type_at` ON behavior_events(session_id, type, at)
   - `idx_behavior_attempt` ON behavior_events(json_extract(payload_json, '$.attempt_id'))（表达式索引，json_extract 确定性）
   - `idx_gu_rules_status` ON gu_rules(status)
   - `idx_edges_src` / `idx_edges_dst` ON edges(src) / edges(dst)
2. 测试：
   - 迁移后上述索引存在于 sqlite_master。
   - EXPLAIN QUERY PLAN 断言代表性热查询用上索引（SEARCH ... USING INDEX，而非全表 SCAN）：behavior_events 按 type+at、attempts 按 concept_id、edges 按 src。
   - 既有全量测试全绿（功能不变性）。
3. 不做：criterion 性能预算基准（roadmap 单列票）；schema 形状变更；查询改写。

## 验收

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p polaris-core --lib db
git diff --check
```

## 禁区

- 不改任何表结构与查询语义；只加索引。
- 不修改冻结参考仓库。

## 交付记录

### 2026-06-13 开工记录

- 当前状态：P03K 已提交（`7adc6d3`）；本票按推荐序（轴 2 性能 S 票）认领为唯一 In Progress。
- 预计修改面：`db.rs`（索引 + 测试）。
- 验收命令：见上。

### 2026-06-13 交付记录

变更清单：

- `crates/polaris-core/src/db.rs`：迁移新增 8 个幂等索引（attempts 概念+时间/时间、behavior_events 类型+时间/会话+类型+时间/attempt_id 表达式索引、gu_rules 状态、edges src/dst）；新增 2 个测试——sqlite_master 索引存在性、EXPLAIN QUERY PLAN 断言 4 条代表性热查询走 USING INDEX 且无全表 SCAN。
- `docs/tickets/QUEUE.md`：P03L 标记完成。

技术选择说明：

- 表达式索引 `json_extract(payload_json,'$.attempt_id')` 覆盖快照回查热路径（json_extract 是确定性函数，SQLite 允许）；按 `julianday(...)` 排序的查询不强行加表达式索引——计划层面行数有限且改查询语义不在本票范围。
- 全部 `CREATE INDEX IF NOT EXISTS`，对既有库幂等，无 schema 形状变更、零查询改写，功能不变性由全量测试背书。

验收输出：

```powershell
> cargo fmt --check
# exit 0

> cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.50s

> cargo test --workspace
test result: ok. 10 passed; ... （CLI）
test result: ok. 65 passed; 0 failed（lib，含 2 个新索引测试）
（其余 14 个集成套件全绿，共 177 个用例，全文见会话记录）

> cargo test -p polaris-core --lib db
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 61 filtered out

> git diff --check
# exit 0（仅 CRLF 行尾告警）
```

回滚方式：

```powershell
git restore crates/polaris-core/src/db.rs docs/tickets/QUEUE.md
Remove-Item docs/tickets/TICKET_P03L_INDEX_AUDIT.md
```
