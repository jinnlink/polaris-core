# 学习项目声明协议 v1

`p-os.toml` 是学习项目声明。它说明当前文件夹是一个学习现场，入口壳可以据此知道默认 pack、开工命令和哪些路径可作为学习证据来源。

它不是 Domain Pack。两者分工如下：

| 层 | 文件 | 作用 | 面向对象 |
|---|---|---|---|
| 学习项目声明 | `p-os.toml` | 绑定当前学习现场、默认入口和证据路径 | Aura、labctl、AI 入口壳 |
| Domain Pack | `packs/<domain>/...` | 声明概念、边、误解、rubric、moves | Polaris Core 与 pack 作者 |

## 最小格式

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

## 字段

| 字段 | 必填 | 含义 |
|---|---:|---|
| `schema_version` | 是 | 当前仅支持 `1` |
| `project_id` | 是 | 稳定项目 ID，例如 `rust-mastery-lab` |
| `title` | 是 | 学生可读项目名 |
| `kind` | 否 | 项目类型，默认 `course` |
| `default_pack` | 是 | 默认 Domain Pack ID |
| `default_entry` | 否 | 默认入口名，默认 `today` |
| `entry.today_command` | 是 | 学生说「开工」时入口壳可调用的命令模板 |
| `entry.*_label` | 否 | 学生第一屏 3 个动作的显示文案 |
| `evidence.include` | 否 | 可作为学习证据来源的路径范围 |
| `evidence.ignore` | 否 | 应忽略的构建产物、依赖目录和私有目录 |
| `ui.preferred_shell` | 否 | 首选入口壳，例如 `aura` |

未知字段必须被忽略，以便后续版本扩展时旧二进制仍能读取基础信息。

## 发现规则

入口壳从当前工作目录向上查找 `p-os.toml`：

1. 找到后，当前会话绑定到该学习项目。
2. `开工` 解析为 `entry.today_command` 或后续稳定入口。
3. `default_pack` 只说明默认知识结构来源，不自动安装、不自动切换、不改变评分。
4. 找不到声明时，不假装已经接入；提示可以为当前项目创建声明。

Polaris Core 提供只读探测入口：

```powershell
cargo run -p polaris-cli -- project detect --path examples\project-manifests\rust-mastery-lab
```

输出会包含：

```text
project_id: rust-mastery-lab
default_pack: rust
entry: today
```

完整的 AI IDE 接入流程见 [AI IDE 使用 Polaris 学习指南](AI_IDE_USAGE.md)。

## 边界

- 项目声明不写数据库。
- 项目声明不创建 attempt。
- 项目声明不改变调度、相图或评分。
- `default_pack` 不是课程内容本身，只是入口默认绑定。
- Rust、英语、生物等领域逻辑仍必须放在 Domain Pack 或学习项目自己的课程壳中，不能写进 core crate。

## 样例

- `examples/project-manifests/rust-mastery-lab/p-os.toml`
- `examples/project-manifests/english-learning/p-os.toml`
- `examples/project-manifests/biology-foundations/p-os.toml`
