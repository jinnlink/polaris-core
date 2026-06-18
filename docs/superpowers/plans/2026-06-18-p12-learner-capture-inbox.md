# P12 学习收件箱实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 把「学生在任何地方学到的东西」先无感保存为证据，再通过学习收件箱转成可验证练习。

**架构：** Polaris Core 继续做本地事实源、评分、调度和镜像报告；学生入口由 `Learned` / Aura / `labctl` 承载。P12 先在 Polaris Core 建 capture/inbox/practice bridge 的稳定语义，再让 Aura 或 `labctl` 作为壳接入。

**技术栈：** Rust workspace、SQLite、现有 `Engine::submit`、现有 HTTP/MCP/CLI 门、Domain Pack validator、无 Node/Tauri 新依赖。

---

## 范围拆分

本计划按后续正式票拆分，不要求一次完成。

| 票号 | 名称 | 产出 |
|---|---|---|
| P12B | Learning Project Manifest v1 | 每个学习项目用 `p-os.toml` 声明已接入 P-OS |
| P12C | Capture Queue v1 | 外部知识先写 evidence 与队列状态，不生成 attempt |
| P12D | Learner Inbox v1 | 学生可查看、延后、忽略、转练习 |
| P12E | Inbox Practice Bridge | 收件箱条目转成 prompt，学生作答后走 `Engine::submit` |
| P12F | Concept Suggestion + Pack Patch | 候选概念/误解/边进入草案，不直接改正式 pack |
| P12G | Learned / Aura Bridge | `开工` 入口合并日课、due、弱项和收件箱 |

## 文件结构建议

### P12B

**创建：**

- `docs/PROJECT_MANIFEST_PROTOCOL.md`：`p-os.toml` 学习项目声明协议。
- `examples/project-manifests/rust-mastery-lab.toml`：Rust 项目声明样例。
- `examples/project-manifests/english.toml`：英语项目声明样例。
- `examples/project-manifests/biology.toml`：生物项目声明样例。
- `docs/tickets/TICKET_P12B_PROJECT_MANIFEST.md`：正式票据。

**修改：**

- `docs/LEARNER_CAPTURE_ROADMAP.md`：记录项目声明状态。
- `docs/PRODUCT_ROADMAP.md`：链接项目声明协议。
- `docs/tickets/QUEUE.md`：单票认领和完成状态。

### P12C

**创建：**

- `crates/polaris-core/src/capture_queue.rs`：capture queue 类型、状态枚举、写入和查询函数。
- `crates/polaris-core/tests/p12c_capture_queue.rs`：队列 DDL、写入、状态和 mastery 不变性测试。

**修改：**

- `crates/polaris-core/src/db.rs`：迁移中新增队列表或 evidence 状态结构。
- `crates/polaris-core/src/lib.rs`：导出 capture queue 模块。
- `crates/polaris-core/src/engine.rs`：增加薄封装，避免 UI/CLI 直接操作低层表。
- `crates/polaris-cli/src/main.rs`：增加 `capture` 命令或在 `ingest` 下增加 `--record-only` 学生入口别名。
- `crates/polaris-cli/src/http.rs`：增加本地 `POST /capture`，返回 `recorded_only`。
- `docs/API_CONTRACT.md`：登记稳定前的实验性入口，或明确 P12C 暂不稳定。
- `docs/DATA_MODEL.md`：登记新增表、字段和不变量。
- `docs/tickets/TICKET_P12C_CAPTURE_QUEUE.md`：正式票据。
- `docs/tickets/QUEUE.md`：单票认领和完成状态。

### P12D

**创建：**

- `crates/polaris-core/src/learner_inbox.rs`：学生收件箱只读聚合与状态流转。
- `crates/polaris-core/tests/p12c_learner_inbox.rs`：列表、状态转移、学生动作测试。

**修改：**

- `crates/polaris-cli/src/main.rs`：`polaris inbox list|defer|ignore|archive|practice-ready`。
- `crates/polaris-cli/src/http.rs`：`GET /inbox`、`POST /inbox/{id}/action`。
- `crates/polaris-cli/src/mcp.rs`：给 Tier 2 暴露只读/动作工具，但不得直接评分。
- `docs/API_CONTRACT.md`：新增接口稳定边界。
- `docs/tickets/TICKET_P12D_LEARNER_INBOX.md`：正式票据。

### P12E

**创建：**

- `crates/polaris-core/src/inbox_practice.rs`：把 evidence + candidate mapping 转成 prompt 草案。
- `crates/polaris-core/tests/p12d_inbox_practice.rs`：桥接到 `Engine::submit` 的端到端测试。

**修改：**

- `crates/polaris-core/src/engine.rs`：增加 `start_inbox_practice` 或等价薄封装。
- `crates/polaris-cli/src/main.rs`：`polaris inbox practice <item-id> --response ... --confidence ...`。
- `crates/polaris-cli/src/http.rs`：`POST /inbox/{id}/practice`。
- `docs/tickets/TICKET_P12E_INBOX_PRACTICE_BRIDGE.md`：正式票据。

### P12F

**创建：**

- `crates/polaris-core/src/concept_suggestions.rs`：候选概念、误解、边的数据结构和 strict-citation 校验。
- `crates/polaris-core/tests/p12e_concept_suggestions.rs`：证据引用、草案输出和不过门留存测试。
- `packs/overlays/README.md` 或等价草案目录说明：个人 overlay pack 的文件边界。

**修改：**

- `crates/polaris-cli/src/main.rs`：`polaris inbox suggest`、`polaris pack patch validate` 等维护者入口。
- `docs/PACK_AUTHOR_GUIDE.md`：补个人 overlay pack 草案说明。
- `docs/tickets/TICKET_P12F_CONCEPT_SUGGESTIONS.md`：正式票据。

### P12G

**只在取得 `Learned` 写权限和该仓库正式票后执行。**

**可能修改：**

- `C:\MyProject\Learned\rust-mastery-lab\labctl\src\...`
- `C:\MyProject\Learned\rust-mastery-lab\aura\src\...`
- `C:\MyProject\Learned\rust-mastery-lab\docs\...`

**Polaris Core 侧可能修改：**

- `docs/LEARNER_CAPTURE_ROADMAP.md`：记录接入状态。
- `docs/API_CONTRACT.md`：稳定 Aura 所需的接口字段。

## P12B 详细任务

以下任务只有在 `docs/tickets/QUEUE.md` 将 P12B 标为唯一 In Progress 后才能执行。本计划不是直接开工授权。

### 任务 1：定义学习项目声明协议

**文件：**

- 创建：`docs/PROJECT_MANIFEST_PROTOCOL.md`
- 创建：`examples/project-manifests/rust-mastery-lab.toml`
- 创建：`examples/project-manifests/english.toml`
- 创建：`examples/project-manifests/biology.toml`

- [ ] **步骤 1：写协议文档**

最小字段：

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
```

- [ ] **步骤 2：写样例**

Rust、英语、生物各给一个 `p-os.toml` 样例。样例必须体现：项目声明负责「当前学习现场」，Domain Pack 负责「知识结构」。

- [ ] **步骤 3：验证文档不把项目声明混同为 pack**

运行：

```powershell
rg -n "p-os.toml|学习项目声明|Domain Pack|default_pack" docs\PROJECT_MANIFEST_PROTOCOL.md examples\project-manifests
```

预期：存在匹配，退出码 0。

### 任务 2：增加项目发现和校验入口

**文件：**

- 创建：`crates/polaris-core/src/project_manifest.rs`
- 创建：`crates/polaris-core/tests/p12b_project_manifest.rs`
- 修改：`crates/polaris-core/src/lib.rs`
- 修改：`crates/polaris-cli/src/main.rs`

- [ ] **步骤 1：写失败测试**

```rust
#[test]
fn discovers_nearest_p_os_manifest_by_walking_upward() {
    let root = temp_project_with_manifest("rust-mastery-lab", "rust");
    let nested = root.path().join("exercises/day01/src");
    std::fs::create_dir_all(&nested).unwrap();

    let manifest = discover_project_manifest(&nested).unwrap().unwrap();

    assert_eq!(manifest.project_id, "rust-mastery-lab");
    assert_eq!(manifest.default_pack, "rust");
}
```

- [ ] **步骤 2：实现最小解析**

只解析 `schema_version`、`project_id`、`title`、`default_pack`、`entry.today_command`、`evidence.include`、`evidence.ignore`。未知字段保留向后兼容，不报错。

- [ ] **步骤 3：CLI 校验**

建议命令：

```powershell
cargo run -p polaris-cli -- project detect --path C:\MyProject\Learned\rust-mastery-lab
```

预期输出：

```text
project_id: rust-mastery-lab
default_pack: rust
entry: today
```

## P12C 详细任务

以下任务只有在 `docs/tickets/QUEUE.md` 将 P12C 标为唯一 In Progress 后才能执行。本计划不是直接开工授权。

### 任务 1：写失败测试，证明 raw capture 不影响掌握度

**文件：**

- 创建：`crates/polaris-core/tests/p12c_capture_queue.rs`

- [ ] **步骤 1：编写失败测试**

```rust
#[test]
fn captured_evidence_is_recorded_only_and_does_not_create_attempt_or_mastery() {
    let (conn, engine) = test_engine_with_rust_pack();

    let before_attempts = count_rows(&conn, "attempts");
    let before_mastery = read_mastery_rows(&conn);

    let item = engine
        .capture_learning_evidence(CaptureInput {
            session_id: Some("test-session".to_string()),
            source: "paste".to_string(),
            content_type: "text/plain".to_string(),
            text: "I read a note about ownership moves.".to_string(),
            learner_kind: LearnerCaptureKind::Reference,
            concept_ids: vec![],
        })
        .unwrap();

    assert_eq!(item.effect, CaptureEffect::RecordedOnly);
    assert_eq!(count_rows(&conn, "attempts"), before_attempts);
    assert_eq!(read_mastery_rows(&conn), before_mastery);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p polaris-core --test p12c_capture_queue captured_evidence_is_recorded_only_and_does_not_create_attempt_or_mastery
```

预期：编译失败，缺少 `capture_learning_evidence`、`CaptureInput`、`CaptureEffect` 等类型。

### 任务 2：新增 capture queue 数据结构

**文件：**

- 修改：`crates/polaris-core/src/db.rs`
- 创建：`crates/polaris-core/src/capture_queue.rs`
- 修改：`crates/polaris-core/src/lib.rs`

- [ ] **步骤 1：在迁移中新增最小表**

建议 DDL：

```sql
capture_queue(
    id TEXT PRIMARY KEY,
    evidence_id TEXT NOT NULL,
    status TEXT NOT NULL,
    learner_kind TEXT NOT NULL,
    candidate_concept_ids_json TEXT NOT NULL DEFAULT '[]',
    note TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(evidence_id) REFERENCES evidence_items(id)
)
```

`status` 初始枚举：

```rust
pub enum CaptureStatus {
    Pending,
    Mapped,
    PracticeReady,
    Practiced,
    Ignored,
    Archived,
}
```

P12C 只允许新写入 `Pending`。`Mapped`、`PracticeReady`、`Practiced` 等状态是为 P12D/P12E 预留的枚举值，不能在 P12C 中产生状态流转。

`learner_kind` 初始枚举：

```rust
pub enum LearnerCaptureKind {
    Reference,
    OwnAnswer,
    ErrorLog,
    CodeChange,
    ChatExcerpt,
    Unknown,
}
```

- [ ] **步骤 2：实现写入函数**

```rust
pub fn capture_learning_evidence(
    conn: &Connection,
    input: CaptureInput,
) -> Result<CaptureRecord> {
    // 1. 写 evidence_items
    // 2. 写 capture_queue，status=Pending
    // 3. 返回 RecordedOnly
}
```

- [ ] **步骤 3：运行专项测试确认通过**

运行：

```powershell
cargo test -p polaris-core --test p12c_capture_queue
```

预期：P12C capture queue 专项测试通过。

### 任务 3：CLI 与 HTTP 入口

**文件：**

- 修改：`crates/polaris-cli/src/main.rs`
- 修改：`crates/polaris-cli/src/http.rs`
- 修改：`docs/API_CONTRACT.md`

- [ ] **步骤 1：CLI 增加学生友好入口**

建议命令：

```powershell
cargo run -p polaris-cli -- --db target\p12.db capture --text "我刚看了 Rust 所有权的一段解释" --source paste
```

预期输出：

```text
recorded_only: true
message: 已保存为学习资料，不会直接算作掌握。
```

- [ ] **步骤 2：HTTP 增加本地入口**

请求：

```http
POST /capture
Content-Type: application/json

{
  "session": "aura",
  "source": "paste",
  "content_type": "text/plain",
  "text": "我刚看了 Rust 所有权的一段解释",
  "learner_kind": "reference"
}
```

响应：

```json
{
  "capture_id": "...",
  "evidence_id": "...",
  "recorded_only": true,
  "message": "已保存为学习资料，不会直接算作掌握。"
}
```

- [ ] **步骤 3：测试入口**

运行：

```powershell
cargo test -p polaris-cli capture
cargo test -p polaris-cli http_capture
```

预期：CLI 和 HTTP capture 测试通过。

### 任务 4：整体验收

**文件：**

- 修改：`docs/tickets/TICKET_P12C_CAPTURE_QUEUE.md`
- 修改：`docs/tickets/QUEUE.md`

- [ ] **步骤 1：运行基线验收**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

- [ ] **步骤 2：记录真实输出**

把真实输出写入 P12C 票尾。若默认 target 遇到 Windows 文件锁，保留失败原文，再用隔离 `CARGO_TARGET_DIR` 跑同参数命令。

- [ ] **步骤 3：请求审查**

至少安排一个建设 agent 和一个审查 agent：

- 建设 agent：实现 P12C DDL、core、CLI/HTTP。
- 审查 agent：检查是否有 raw evidence 直接影响 mastery、是否破坏 API 合约、是否泄露外部评分字段。

## 后续票验收门

P12C 以后每票都必须显式验证：

- 学生动作最多 3 个。
- raw evidence 不直接改掌握度。
- 外部 AI 输出不作为 `final_score` 权威。
- 所有 LLM 派生 suggestion 都带 evidence id 和 quote。
- 不在 core crate 写 Rust 课程专用逻辑。
- 不修改 `C:\MyProject\Learned`，除非该仓库另开正式票。
