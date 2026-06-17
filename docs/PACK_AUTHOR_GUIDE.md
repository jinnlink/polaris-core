# Pack 作者 30 分钟上手指南

本指南面向已经有一门课、一组知识点或一个训练主题，想把它接入 Polaris Core 的作者。完整契约见 `docs/COURSE_INTEGRATION_PROTOCOL.md`；本指南只提供最短可跑通路径。

## 你要交付什么

一个 pack 是一个目录，至少包含 5 个文件：

```text
packs/<pack_id>/
  pack.toml
  concepts.toml
  misconceptions.toml
  rubric.md
  moves.toml
  ingest.toml          # 可选，当前只是协议预留
```

pack 只声明知识结构、误解、评分标准和教学 move。它不包含可执行代码，也不能直接写 `mastery_states`、`theta` 或调度参数。

## 30 分钟路径

1. 复制模板：

```powershell
Copy-Item -Recurse packs\template packs\my_course
```

2. 修改 `packs/my_course/pack.toml`：

```toml
id = "my_course"
title = "My Course"
version = "0.1.0"
lang = "zh"
```

`id` 是机器 ID，会写入 `concepts.pack`，建议只用小写 ASCII、数字、下划线或连字符。复制模板后必须改掉 `id`，否则会和模板 pack 冲突。

3. 修改 `concepts.toml`：

- 保留约 5 到 10 个核心概念起步。
- 每个 `[[concept]]` 必须有 `id`、`name`、`seed_order`。
- 把所有 `template_*` concept ID 改成你的 pack 前缀，例如 `my_course_core_terms`。不要只改 `pack.toml`，否则多个从模板复制出来的 pack 会在安装时发生 concept ID 冲突。
- 同步修改同文件里所有 `[[edge]]` 的 `src`、`dst`，确保它们引用新的 concept ID。
- 只有总览或能力框架类节点才写 `kind = "schema"`。
- 真正前置关系写 `type = "prerequisite"`；组成关系写 `component_of`；容易混淆但非前置的关系写 `confusion`。

4. 修改 `misconceptions.toml`：

- 每条 `[[misconception]]` 必须引用已存在的 `concept_id`。
- 把 `concept_id = "template_..."` 同步改成你在 `concepts.toml` 中的新 concept ID。
- `pattern` 建议使用 DATA_MODEL §9 的 8 类枚举，例如 `fluency-illusion`、`boundary-blindness`、`granularity-mismatch`。

5. 修改 `rubric.md`：

写清楚正确性、深度、边界和证据要求。rubric 不是随意 prompt 片段；它是评分契约。

6. 修改 `moves.toml`：

建议保留 7 个 move：`recall`、`explain`、`apply`、`analyze`、`evaluate`、`create`、`transfer`。模板里可以使用 `{concept}` 占位符，运行时会替换为概念名称。

7. 先验证模板本身：

```powershell
cargo run -p polaris-cli -- pack validate packs/template
```

8. 验证你的 pack：

```powershell
cargo run -p polaris-cli -- pack validate packs/my_course
```

9. 用内存沙箱跑完整闭环，避免污染默认库：

```powershell
cargo run -p polaris-cli -- pack sandbox packs/my_course --days 7
cargo run -p polaris-cli -- pack sandbox packs/my_course --profile all --days 7
```

`pack sandbox` 会先 validate，再在内存 SQLite 中 init/switch/simulate。它不会读取或写入默认 `polaris.sqlite`，也不会调用外部 LLM。`status=pass` 表示虚拟学习者闭环能跑通且没有死锁；`status=warn` 表示能跑通但提升信号偏弱；`status=fail` 需要先修 pack 结构或教学闭环。

如果你还想人工查看具体 `next` 任务文本，再用临时数据库做一次手动检查：

```powershell
cargo run -p polaris-cli -- --db target/my-course.sqlite init --pack packs/my_course
cargo run -p polaris-cli -- --db target/my-course.sqlite pack switch my_course --theta-mode isolated
cargo run -p polaris-cli -- --db target/my-course.sqlite next
```

## 当前 validator 检查什么

`polaris pack validate <dir>` 当前检查：

- 必须存在 `pack.toml`、`concepts.toml`、`misconceptions.toml`、`rubric.md`、`moves.toml`。
- `pack.toml` 必须能解析，且 `id`、`title` 非空。
- `concepts.toml` 必须能解析，concept kind 必须合法。
- edge type 必须合法。
- edge 的 `src`、`dst` 必须引用已声明 concept。
- misconception 的 `concept_id` 必须引用已声明 concept。
- `moves.toml` 至少包含 1 条 move，且归一化后的 `id`、`task_type`、`template` 非空。
- `rubric.md` 不能为空。

当前 validator 不检查：

- `ingest.toml`。
- `version` 是否符合 SemVer。
- prerequisite 是否为 DAG。
- `alignment_json` 是否为合法 JSON。
- `misconception.pattern` 是否属于推荐枚举。

这些仍然是作者责任。后续如果升级为强制规则，应走新票和兼容窗口。

## 常见报错

| 报错 | 含义 | 修复 |
|---|---|---|
| `missing required pack file` | 缺少必需文件 | 补齐前 5 个文件 |
| `failed to parse` | TOML 语法错或字段类型不匹配 | 检查 `[[concept]]`、`[[edge]]`、`[[move]]` 写法 |
| `edge ... references missing concept` | 边引用了不存在的 concept | 修正 `src` / `dst`，或补 concept |
| `misconception ... references missing concept` | 误解挂到了不存在的 concept | 修正 `concept_id` |
| `concept ... has invalid kind` | concept kind 不合法 | 使用 `concept`、`schema` 或 `misconception_induced` |
| `edge ... has invalid type` | edge type 不合法 | 使用 `prerequisite`、`confusion`、`component_of`、`instantiates` 或 `maps_to` |
| `pack must contain at least one move template` | move 为空或字段为空 | 至少声明一条有效 move |
| `rubric.md is empty` | rubric 没有有效内容 | 写入评分标准 |

## 发布前自查

- `pack.toml` 的 `id` 已从模板改成你的 pack ID。
- 所有 `template_*` concept ID、edge 引用和 misconception 引用都已替换，不和已安装 pack 冲突。
- `seed_order` 从 1 开始稳定排序。
- 只有真正前置才写 `prerequisite`。
- rubric 明确写了评分边界和证据要求。
- 7 个 move 都能生成一个清晰任务。
- `pack sandbox packs/my_course --profile all --days 7` 能跑完，并且你理解每个 profile 的 `status` 和提示。
