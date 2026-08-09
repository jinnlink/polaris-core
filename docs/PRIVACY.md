# Polaris Privacy Inventory

本文列出 polaris-core 当前会触发外部模型调用的通道。设置 `POLARIS_TIER0_ONLY=1` 可禁用这些通道，并走各自降级路径。

## LLM Grading

id: llm_grade_attempt

- tier: Tier 1
- trigger: `submit` / `grade-pending` / MCP evidence grading
- env: `POLARIS_LLM_FAST_BASE_URL`, `POLARIS_LLM_FAST_MODEL`, `POLARIS_LLM_FAST_API_KEY`, `POLARIS_LLM_STRONG_BASE_URL`, `POLARIS_LLM_STRONG_MODEL`, `POLARIS_LLM_STRONG_API_KEY`
- data_sent: attempt response text; domain rubric; active G_u prompt context; strict-citation evidence prompt
- degradation: heuristic score + grade_queue retry
- disabled_when_tier0_only: true

## Mirror Report Narrative

id: llm_mirror_narrative

- tier: Tier 1
- trigger: `report --narrative` / MCP `run_mirror_report(narrative=true)`
- env: `POLARIS_LLM_FAST_BASE_URL`, `POLARIS_LLM_FAST_MODEL`, `POLARIS_LLM_FAST_API_KEY`, `POLARIS_LLM_STRONG_BASE_URL`, `POLARIS_LLM_STRONG_MODEL`, `POLARIS_LLM_STRONG_API_KEY`
- data_sent: mirror report assertion/hypothesis/suggestion claims
- degradation: raw mirror report without narrative
- disabled_when_tier0_only: true

## Concept Embedding

id: embed_concept

- tier: Tier 1
- trigger: geometry embedding refresh
- env: `POLARIS_EMBED_BASE_URL`, `POLARIS_EMBED_MODEL`, `POLARIS_EMBED_API_KEY`
- data_sent: concept and schema names used for embedding
- degradation: geometry layer disabled; symbolic and latent layers continue
- disabled_when_tier0_only: true

## Global Learner Profile（仅本机）

- 默认在本地启用，首次记录回答前必须向用户说明并取得确认；用户可随时暂停或关闭。
- 原始回答只写本地 `behavior_events(type='profile_measurement')`，不会进入任何外部模型调用，也永不通过 HTTP/MCP 返回。
- HTTP/MCP 只提供分维度摘要，且 `summary_sharing_enabled` 默认关闭；显式开启后仍只包含均值、方差、证据数、模型版本、门状态和 provenance。
- `profile export` 是用户主动触发的本地文件导出，会包含原始回答；`profile reset` 保留学习尝试，`profile delete-all` 在 Engine 关闭后删除旧数据库、SQLite sidecar 与注入的本机密钥，并在原路径建立空库。

## Extension Rule

Any future outbound channel must add a `PrivacyCallInventory` entry, document the same `id:` here, and provide a Tier0-only suppression or degradation path.
