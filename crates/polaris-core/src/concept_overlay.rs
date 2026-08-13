use std::collections::{HashMap, HashSet};
use std::fs;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::citation::{validate_citations_with_policy, Citation, CitationPolicy, EvidenceText};
use crate::error::{PolarisError, Result};
use crate::grader::LlmConfig;
use crate::graph::{is_valid_concept_kind, is_valid_edge_type};
use crate::pack::validate_pack_path;
use crate::sandbox::{run_pack_sandbox, SandboxOptions, SandboxStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    Concept,
    Schema,
    TypedEdge,
    Misconception,
}

impl SuggestionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Concept => "concept",
            Self::Schema => "schema",
            Self::TypedEdge => "typed_edge",
            Self::Misconception => "misconception",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "concept" => Some(Self::Concept),
            "schema" => Some(Self::Schema),
            "typed_edge" => Some(Self::TypedEdge),
            "misconception" => Some(Self::Misconception),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuggestionPayload {
    Concept {
        id: String,
        name: String,
        #[serde(default = "default_concept_kind")]
        concept_kind: String,
        #[serde(default = "default_generativity")]
        generativity: String,
        seed_order: i64,
        #[serde(default)]
        p_init: Option<f64>,
    },
    Schema {
        id: String,
        name: String,
        seed_order: i64,
    },
    TypedEdge {
        id: String,
        src: String,
        dst: String,
        edge_type: String,
        #[serde(default = "default_weight")]
        weight: f64,
    },
    Misconception {
        id: String,
        concept_id: String,
        title: String,
        #[serde(default)]
        pattern: Option<String>,
    },
}

impl SuggestionPayload {
    pub fn kind(&self) -> SuggestionKind {
        match self {
            Self::Concept { .. } => SuggestionKind::Concept,
            Self::Schema { .. } => SuggestionKind::Schema,
            Self::TypedEdge { .. } => SuggestionKind::TypedEdge,
            Self::Misconception { .. } => SuggestionKind::Misconception,
        }
    }

    fn entity_id(&self) -> &str {
        match self {
            Self::Concept { id, .. }
            | Self::Schema { id, .. }
            | Self::TypedEdge { id, .. }
            | Self::Misconception { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestedItem {
    pub payload: SuggestionPayload,
    pub citation: Citation,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuggestionResponse {
    pub suggestions: Vec<SuggestedItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConceptSuggestion {
    pub id: String,
    pub capture_id: String,
    pub evidence_id: String,
    pub base_pack_id: String,
    pub kind: SuggestionKind,
    pub status: String,
    pub payload: SuggestionPayload,
    pub quote: String,
    pub reason: String,
    pub model_version: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayDiffItem {
    pub suggestion_id: String,
    pub kind: SuggestionKind,
    pub entity_id: String,
    pub summary: String,
    pub evidence_id: String,
    pub quote: String,
    pub model_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayPreview {
    pub base_pack_id: String,
    pub next_version: i64,
    pub parent_version: Option<i64>,
    pub status: String,
    pub validation: String,
    pub items: Vec<OverlayDiffItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayInstallReceipt {
    pub overlay_version_id: String,
    pub base_pack_id: String,
    pub version: i64,
    pub status: String,
    pub validation: String,
    pub sandbox: String,
    pub installed_entity_ids: Vec<String>,
    pub attempts_unchanged: bool,
    pub mastery_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverlayRollbackReceipt {
    pub base_pack_id: String,
    pub rolled_back_version: i64,
    pub active_version: Option<i64>,
    pub restored_entity_ids: Vec<String>,
    pub attempts_unchanged: bool,
    pub mastery_unchanged: bool,
}

pub fn suggest_with_static_response(
    conn: &Connection,
    capture_id: &str,
    base_pack_id: &str,
    model_version: &str,
    response_json: &str,
) -> Result<Vec<ConceptSuggestion>> {
    let capture_id = required("suggestion.capture_id", capture_id)?;
    let base_pack_id = required("suggestion.base_pack_id", base_pack_id)?;
    let model_version = required("suggestion.model_version", model_version)?;
    let response: SuggestionResponse = serde_json::from_str(response_json)?;
    if response.suggestions.is_empty() {
        return Err(invalid(
            "suggestion.response",
            "suggestions must not be empty",
        ));
    }

    let (evidence_id, evidence_text, candidate_json): (String, String, String) = conn
        .query_row(
            "SELECT cq.evidence_id, e.text, cq.candidate_concept_ids_json
             FROM capture_queue cq
             JOIN evidence_items e ON e.id=cq.evidence_id
             WHERE cq.id=?1",
            [capture_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| invalid("suggestion.capture_id", "capture not found"))?;
    let mapped_ids: Vec<String> = serde_json::from_str(&candidate_json)?;
    if mapped_ids
        .iter()
        .any(|id| concept_exists(conn, id).unwrap_or(false))
    {
        return Err(invalid(
            "suggestion.capture_id",
            "capture already maps to an installed concept",
        ));
    }

    let allowed_evidence = [EvidenceText {
        id: evidence_id.clone(),
        text: evidence_text,
    }];
    let citations = response
        .suggestions
        .iter()
        .map(|item| item.citation.clone())
        .collect::<Vec<_>>();
    validate_citations_with_policy(
        &citations,
        &allowed_evidence,
        CitationPolicy::from_conn(conn)?,
    )
    .map_err(|error| invalid("suggestion.citation", &error.to_string()))?;

    let mut entity_ids = HashSet::new();
    for item in &response.suggestions {
        validate_payload(&item.payload)?;
        required("suggestion.reason", &item.reason)?;
        if !entity_ids.insert(item.payload.entity_id()) {
            return Err(invalid("suggestion.payload.id", "duplicate entity id"));
        }
    }

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<Vec<ConceptSuggestion>> {
        let mut recorded = Vec::with_capacity(response.suggestions.len());
        for item in response.suggestions {
            let id = Uuid::new_v4().to_string();
            let payload_json = serde_json::to_string(&item.payload)?;
            conn.execute(
                "INSERT INTO concept_suggestions(
                    id, capture_id, evidence_id, base_pack_id, kind, status,
                    payload_json, quote, reason, model_version, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?8, ?9,
                    strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                params![
                    id,
                    capture_id,
                    evidence_id,
                    base_pack_id,
                    item.payload.kind().as_str(),
                    payload_json,
                    item.citation.quote,
                    item.reason.trim(),
                    model_version,
                ],
            )?;
            recorded.push(load_suggestion(conn, &id)?);
        }
        Ok(recorded)
    })();
    finish_transaction(conn, result)
}

pub fn suggest_with_config(
    conn: &Connection,
    capture_id: &str,
    base_pack_id: &str,
    config: LlmConfig,
) -> Result<Vec<ConceptSuggestion>> {
    let LlmConfig::OpenAiCompatible {
        base_url,
        model,
        api_key,
    } = config
    else {
        return Err(invalid(
            "suggestion.model",
            "Tier 1 model is unavailable; raw capture was kept unchanged",
        ));
    };
    let (evidence_id, text): (String, String) = conn
        .query_row(
            "SELECT cq.evidence_id, e.text
             FROM capture_queue cq JOIN evidence_items e ON e.id=cq.evidence_id
             WHERE cq.id=?1",
            [capture_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| invalid("suggestion.capture_id", "capture not found"))?;
    let mut stmt = conn.prepare(
        "SELECT id, name, kind FROM concepts
         WHERE pack=?1 ORDER BY seed_order ASC, id ASC LIMIT 120",
    )?;
    let installed_concepts = stmt
        .query_map([base_pack_id], |row| {
            Ok(format!(
                "{} | {} | {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    let mut last_error = "model returned no valid suggestion".to_owned();
    for _ in 0..2 {
        match call_suggestion_model(
            &base_url,
            &model,
            &api_key,
            base_pack_id,
            &evidence_id,
            &text,
            &installed_concepts,
        )
        .and_then(|response| {
            suggest_with_static_response(conn, capture_id, base_pack_id, &model, &response)
        }) {
            Ok(suggestions) => return Ok(suggestions),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(invalid(
        "suggestion.model_response",
        &format!("{last_error}; raw capture was kept unchanged"),
    ))
}

fn call_suggestion_model(
    base_url: &str,
    model: &str,
    api_key: &str,
    base_pack_id: &str,
    evidence_id: &str,
    text: &str,
    installed_concepts: &str,
) -> Result<String> {
    #[derive(Deserialize)]
    struct ChatResponse {
        choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(Deserialize)]
    struct Message {
        content: String,
    }
    let endpoint = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "temperature": 0,
        "response_format": {"type":"json_object"},
        "messages": [
            {"role":"system","content":"You are Polaris Tier 1 concept mapper. Suggest only evidence-grounded personal overlay items. Return JSON {suggestions:[{payload,citation:{evidence_id,quote},reason}]}. Payload kind is concept, schema, typed_edge, or misconception. Every quote must be an exact substring of the supplied evidence. Never claim mastery and never modify the official pack."},
            {"role":"user","content":format!("Base pack: {base_pack_id}\nInstalled concepts (id | name | kind):\n{installed_concepts}\n\nEvidence id: {evidence_id}\nEvidence text:\n{text}\n\nUse globally unique snake_case ids for new entities. Reuse an installed concept id only as an edge endpoint or misconception concept_id. Concepts need id,name,seed_order and optional concept_kind,generativity,p_init. Schemas need id,name,seed_order. Typed edges need id,src,dst,edge_type,weight. Misconceptions need id,concept_id,title and optional pattern.")}
        ]
    });
    let response: ChatResponse = reqwest::blocking::Client::new()
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()?
        .error_for_status()?
        .json()?;
    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| PolarisError::InvalidGraderResponse("empty choices".to_owned()))
}

pub fn suggestions_for_capture(
    conn: &Connection,
    capture_id: &str,
) -> Result<Vec<ConceptSuggestion>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM concept_suggestions
         WHERE capture_id=?1
         ORDER BY created_at DESC, id ASC",
    )?;
    let ids = stmt
        .query_map([capture_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.iter().map(|id| load_suggestion(conn, id)).collect()
}

pub fn reject_suggestion(conn: &Connection, suggestion_id: &str) -> Result<ConceptSuggestion> {
    let changed = conn.execute(
        "UPDATE concept_suggestions
         SET status='rejected', decided_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?1 AND status='pending'",
        [suggestion_id],
    )?;
    if changed != 1 {
        return Err(invalid("suggestion.id", "pending suggestion not found"));
    }
    load_suggestion(conn, suggestion_id)
}

pub fn preview_overlay(
    conn: &Connection,
    base_pack_id: &str,
    suggestion_ids: &[String],
) -> Result<OverlayPreview> {
    let base_pack_id = required("overlay.base_pack_id", base_pack_id)?;
    if suggestion_ids.is_empty() {
        return Err(invalid("overlay.suggestion_ids", "must not be empty"));
    }
    if !base_pack_exists(conn, &base_pack_id)? {
        return Err(invalid("overlay.base_pack_id", "base pack not installed"));
    }

    let mut suggestions = suggestion_ids
        .iter()
        .map(|id| load_suggestion(conn, id))
        .collect::<Result<Vec<_>>>()?;
    suggestions.sort_by(|left, right| left.id.cmp(&right.id));
    for suggestion in &suggestions {
        if suggestion.base_pack_id != base_pack_id || suggestion.status != "pending" {
            return Err(invalid(
                "overlay.suggestion_ids",
                "all suggestions must be pending and belong to the base pack",
            ));
        }
    }
    let installed_version_id: Option<String> = conn
        .query_row(
            "SELECT id FROM overlay_versions
             WHERE base_pack_id=?1 AND status='installed'
             ORDER BY version DESC LIMIT 1",
            [base_pack_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let mut composed_entities = installed_version_id
        .as_deref()
        .map(|id| load_overlay_entities(conn, id))
        .transpose()?
        .unwrap_or_default();
    for suggestion in &suggestions {
        composed_entities.retain(|item| item.0.entity_id() != suggestion.payload.entity_id());
        composed_entities.push((suggestion.payload.clone(), suggestion.evidence_id.clone()));
    }
    validate_entity_set(conn, &base_pack_id, &composed_entities)?;

    let parent_version: Option<i64> = conn.query_row(
        "SELECT MAX(version) FROM overlay_versions
         WHERE base_pack_id=?1 AND status='installed'",
        [base_pack_id.as_str()],
        |row| row.get(0),
    )?;
    let next_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM overlay_versions WHERE base_pack_id=?1",
        [base_pack_id.as_str()],
        |row| row.get(0),
    )?;
    let items = suggestions
        .into_iter()
        .map(|suggestion| OverlayDiffItem {
            suggestion_id: suggestion.id,
            kind: suggestion.kind,
            entity_id: suggestion.payload.entity_id().to_owned(),
            summary: payload_summary(&suggestion.payload),
            evidence_id: suggestion.evidence_id,
            quote: suggestion.quote,
            model_version: suggestion.model_version,
        })
        .collect();
    Ok(OverlayPreview {
        base_pack_id,
        next_version,
        parent_version,
        status: "awaiting_user_acceptance".to_owned(),
        validation: "pack_validate_passed".to_owned(),
        items,
    })
}

pub fn accept_overlay(
    conn: &Connection,
    base_pack_id: &str,
    suggestion_ids: &[String],
) -> Result<OverlayInstallReceipt> {
    let preview = preview_overlay(conn, base_pack_id, suggestion_ids)?;
    let suggestions = suggestion_ids
        .iter()
        .map(|id| load_suggestion(conn, id))
        .collect::<Result<Vec<_>>>()?;
    let previous_version_id: Option<String> = conn
        .query_row(
            "SELECT id FROM overlay_versions
             WHERE base_pack_id=?1 AND status='installed'
             ORDER BY version DESC LIMIT 1",
            [base_pack_id],
            |row| row.get(0),
        )
        .optional()?;
    let mut all_entities = previous_version_id
        .as_deref()
        .map(|id| load_overlay_entities(conn, id))
        .transpose()?
        .unwrap_or_default();
    for suggestion in &suggestions {
        all_entities.retain(|item| item.0.entity_id() != suggestion.payload.entity_id());
        all_entities.push((suggestion.payload.clone(), suggestion.evidence_id.clone()));
    }
    validate_entity_set(conn, base_pack_id, &all_entities)?;

    let temp = tempfile::tempdir()?;
    materialize_validation_pack(conn, base_pack_id, &all_entities, temp.path())?;
    let validation = validate_pack_path(temp.path())?;
    let sandbox = run_pack_sandbox(SandboxOptions::new(temp.path()).with_days(3))?;
    if sandbox.status == SandboxStatus::Fail {
        return Err(invalid("overlay.sandbox", &sandbox.summary));
    }

    let attempt_count = table_count(conn, "attempts")?;
    let mastery_count = table_count(conn, "mastery_states")?;
    let version_id = Uuid::new_v4().to_string();
    let diff_json = serde_json::to_string(&preview.items)?;
    let validation_json = serde_json::to_string(&serde_json::json!({
        "status":"pass",
        "concept_count":validation.concept_count,
        "prerequisite_count":validation.prerequisite_count,
        "misconception_count":validation.misconception_count
    }))?;
    let sandbox_json = serde_json::to_string(&sandbox)?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<()> {
        conn.execute(
            "UPDATE overlay_versions SET status='superseded'
             WHERE base_pack_id=?1 AND status='installed'",
            [base_pack_id],
        )?;
        conn.execute(
            "INSERT INTO overlay_versions(
                id, base_pack_id, version, status, parent_version, diff_json,
                validation_json, sandbox_json, created_at, installed_at
             ) VALUES (?1, ?2, ?3, 'installed', ?4, ?5, ?6, ?7,
                strftime('%Y-%m-%dT%H:%M:%SZ','now'), strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            params![
                version_id,
                base_pack_id,
                preview.next_version,
                preview.parent_version,
                diff_json,
                validation_json,
                sandbox_json
            ],
        )?;
        for (payload, evidence_id) in &all_entities {
            conn.execute(
                "INSERT INTO overlay_entities(overlay_version_id, entity_id, kind, payload_json, evidence_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    version_id,
                    payload.entity_id(),
                    payload.kind().as_str(),
                    serde_json::to_string(payload)?,
                    evidence_id
                ],
            )?;
        }
        for suggestion in &suggestions {
            conn.execute(
                "INSERT INTO overlay_provenance(
                    overlay_version_id, suggestion_id, capture_id, evidence_id, quote, model_version
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    version_id,
                    suggestion.id,
                    suggestion.capture_id,
                    suggestion.evidence_id,
                    suggestion.quote,
                    suggestion.model_version
                ],
            )?;
            conn.execute(
                "UPDATE concept_suggestions
                 SET status='installed', decided_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
                 WHERE id=?1",
                [suggestion.id.as_str()],
            )?;
        }
        materialize_active_entities(conn, base_pack_id, &version_id, &all_entities)?;
        Ok(())
    })();
    finish_transaction(conn, result)?;

    Ok(OverlayInstallReceipt {
        overlay_version_id: version_id,
        base_pack_id: base_pack_id.to_owned(),
        version: preview.next_version,
        status: "installed".to_owned(),
        validation: "pass".to_owned(),
        sandbox: sandbox.status.as_str().to_owned(),
        installed_entity_ids: all_entities
            .iter()
            .map(|item| item.0.entity_id().to_owned())
            .collect(),
        attempts_unchanged: table_count(conn, "attempts")? == attempt_count,
        mastery_unchanged: table_count(conn, "mastery_states")? == mastery_count,
    })
}

pub fn rollback_overlay(conn: &Connection, base_pack_id: &str) -> Result<OverlayRollbackReceipt> {
    let (current_id, current_version, parent_version): (String, i64, Option<i64>) = conn
        .query_row(
            "SELECT id, version, parent_version FROM overlay_versions
             WHERE base_pack_id=?1 AND status='installed'
             ORDER BY version DESC LIMIT 1",
            [base_pack_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| invalid("overlay.base_pack_id", "no installed overlay"))?;
    let target = parent_version
        .map(|version| {
            conn.query_row(
                "SELECT id FROM overlay_versions WHERE base_pack_id=?1 AND version=?2",
                params![base_pack_id, version],
                |row| row.get::<_, String>(0),
            )
            .map(|id| (version, id))
            .map_err(PolarisError::from)
        })
        .transpose()?;
    let target_entities = target
        .as_ref()
        .map(|(_, id)| load_overlay_entities(conn, id))
        .transpose()?
        .unwrap_or_default();
    let attempt_count = table_count(conn, "attempts")?;
    let mastery_count = table_count(conn, "mastery_states")?;

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| -> Result<()> {
        conn.execute(
            "UPDATE overlay_versions SET status='rolled_back', rolled_back_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?1",
            [current_id.as_str()],
        )?;
        if let Some((_, target_id)) = &target {
            conn.execute(
                "UPDATE overlay_versions SET status='installed', installed_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?1",
                [target_id.as_str()],
            )?;
            materialize_active_entities(conn, base_pack_id, target_id, &target_entities)?;
        } else {
            clear_active_entities(conn, base_pack_id)?;
        }
        Ok(())
    })();
    finish_transaction(conn, result)?;

    Ok(OverlayRollbackReceipt {
        base_pack_id: base_pack_id.to_owned(),
        rolled_back_version: current_version,
        active_version: target.as_ref().map(|(version, _)| *version),
        restored_entity_ids: target_entities
            .iter()
            .map(|item| item.0.entity_id().to_owned())
            .collect(),
        attempts_unchanged: table_count(conn, "attempts")? == attempt_count,
        mastery_unchanged: table_count(conn, "mastery_states")? == mastery_count,
    })
}

fn validate_overlay_graph(
    conn: &Connection,
    base_pack_id: &str,
    suggestions: &[ConceptSuggestion],
) -> Result<()> {
    let mut node_ids = HashSet::new();
    let mut edge_ids = HashSet::new();
    let mut prerequisite_edges = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT id FROM concepts
         WHERE pack=?1 AND COALESCE(provenance, '') NOT LIKE 'overlay:%'",
    )?;
    for id in stmt.query_map([base_pack_id], |row| row.get::<_, String>(0))? {
        node_ids.insert(id?);
    }
    let mut stmt = conn.prepare(
        "SELECT e.id, e.src, e.dst, e.type
         FROM edges e
         JOIN concepts c ON c.id=e.src
         WHERE c.pack=?1 AND COALESCE(e.provenance, '') NOT LIKE 'overlay:%'",
    )?;
    let rows = stmt.query_map([base_pack_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (id, src, dst, edge_type) = row?;
        edge_ids.insert(id);
        if edge_type == "prerequisite" {
            prerequisite_edges.push((src, dst));
        }
    }

    for suggestion in suggestions {
        match &suggestion.payload {
            SuggestionPayload::Concept { id, .. } | SuggestionPayload::Schema { id, .. } => {
                if base_concept_exists(conn, id)? || !node_ids.insert(id.clone()) {
                    return Err(invalid(
                        "overlay.concept_id",
                        &format!("concept id collision: {id}"),
                    ));
                }
            }
            SuggestionPayload::TypedEdge { id, .. } => {
                if base_edge_exists(conn, id)? || !edge_ids.insert(id.clone()) {
                    return Err(invalid(
                        "overlay.edge_id",
                        &format!("edge id collision: {id}"),
                    ));
                }
            }
            SuggestionPayload::Misconception { .. } => {}
        }
    }
    for suggestion in suggestions {
        match &suggestion.payload {
            SuggestionPayload::TypedEdge {
                src,
                dst,
                edge_type,
                ..
            } => {
                if !node_ids.contains(src) || !node_ids.contains(dst) {
                    return Err(invalid(
                        "overlay.edge_reference",
                        &format!("unknown edge endpoint: {src} -> {dst}"),
                    ));
                }
                if edge_type == "prerequisite" {
                    prerequisite_edges.push((src.clone(), dst.clone()));
                }
            }
            SuggestionPayload::Misconception { concept_id, .. }
                if !node_ids.contains(concept_id) =>
            {
                return Err(invalid(
                    "overlay.misconception.concept_id",
                    &format!("unknown concept: {concept_id}"),
                ));
            }
            _ => {}
        }
    }
    if has_cycle(&node_ids, &prerequisite_edges) {
        return Err(invalid("overlay.prerequisite", "cyclic prerequisite graph"));
    }
    Ok(())
}

fn validate_entity_set(
    conn: &Connection,
    base_pack_id: &str,
    entities: &[(SuggestionPayload, String)],
) -> Result<()> {
    let suggestions = entities
        .iter()
        .map(|(payload, evidence_id)| ConceptSuggestion {
            id: payload.entity_id().to_owned(),
            capture_id: String::new(),
            evidence_id: evidence_id.clone(),
            base_pack_id: base_pack_id.to_owned(),
            kind: payload.kind(),
            status: "pending".to_owned(),
            payload: payload.clone(),
            quote: String::new(),
            reason: String::new(),
            model_version: String::new(),
            created_at: String::new(),
        })
        .collect::<Vec<_>>();
    validate_overlay_graph(conn, base_pack_id, &suggestions)
}

fn load_overlay_entities(
    conn: &Connection,
    version_id: &str,
) -> Result<Vec<(SuggestionPayload, String)>> {
    let mut stmt = conn.prepare(
        "SELECT payload_json, evidence_id FROM overlay_entities
         WHERE overlay_version_id=?1 ORDER BY entity_id",
    )?;
    let rows = stmt.query_map([version_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (payload, evidence_id) = row?;
        Ok((serde_json::from_str(&payload)?, evidence_id))
    })
    .collect()
}

fn materialize_validation_pack(
    conn: &Connection,
    base_pack_id: &str,
    entities: &[(SuggestionPayload, String)],
    path: &std::path::Path,
) -> Result<()> {
    let title: String = conn
        .query_row(
            "SELECT value FROM meta WHERE key=?1",
            [format!("pack.{base_pack_id}.title")],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_else(|| base_pack_id.to_owned());
    fs::write(
        path.join("pack.toml"),
        format!(
            "id = {}\ntitle = {}\n",
            toml_string(&format!("{base_pack_id}-overlay-validation")),
            toml_string(&format!("{title} personal overlay validation"))
        ),
    )?;

    let mut concepts = String::new();
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, seed_order, p_init, generativity
         FROM concepts WHERE pack=?1 AND COALESCE(provenance, '') NOT LIKE 'overlay:%'
         ORDER BY seed_order, id",
    )?;
    let rows = stmt.query_map([base_pack_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<f64>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    for row in rows {
        let (id, name, kind, seed_order, p_init, generativity) = row?;
        append_concept(
            &mut concepts,
            &id,
            &name,
            &kind,
            seed_order,
            p_init,
            &generativity,
        );
    }
    for (payload, _) in entities {
        match payload {
            SuggestionPayload::Concept {
                id,
                name,
                concept_kind,
                generativity,
                seed_order,
                p_init,
            } => append_concept(
                &mut concepts,
                id,
                name,
                concept_kind,
                *seed_order,
                *p_init,
                generativity,
            ),
            SuggestionPayload::Schema {
                id,
                name,
                seed_order,
            } => append_concept(
                &mut concepts,
                id,
                name,
                "schema",
                *seed_order,
                None,
                "unknown",
            ),
            _ => {}
        }
    }
    let mut stmt = conn.prepare(
        "SELECT e.id, e.src, e.dst, e.type, e.weight
         FROM edges e JOIN concepts c ON c.id=e.src
         WHERE c.pack=?1 AND COALESCE(e.provenance, '') NOT LIKE 'overlay:%'
         ORDER BY e.id",
    )?;
    let rows = stmt.query_map([base_pack_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;
    for row in rows {
        let (id, src, dst, edge_type, weight) = row?;
        append_edge(&mut concepts, &id, &src, &dst, &edge_type, weight);
    }
    for (payload, _) in entities {
        if let SuggestionPayload::TypedEdge {
            id,
            src,
            dst,
            edge_type,
            weight,
        } = payload
        {
            append_edge(&mut concepts, id, src, dst, edge_type, *weight);
        }
    }
    fs::write(path.join("concepts.toml"), concepts)?;

    let mut misconceptions = String::new();
    for (payload, _) in entities {
        if let SuggestionPayload::Misconception {
            id,
            concept_id,
            title,
            pattern,
        } = payload
        {
            misconceptions.push_str(&format!(
                "[[misconception]]\nid = {}\nconcept_id = {}\ntitle = {}\n",
                toml_string(id),
                toml_string(concept_id),
                toml_string(title)
            ));
            if let Some(pattern) = pattern {
                misconceptions.push_str(&format!("pattern = {}\n", toml_string(pattern)));
            }
            misconceptions.push('\n');
        }
    }
    if misconceptions.is_empty() {
        misconceptions.push_str("misconception = []\n");
    }
    fs::write(path.join("misconceptions.toml"), misconceptions)?;
    fs::write(
        path.join("rubric.md"),
        "# Personal overlay validation rubric\n\nAnswers require strict-citation evidence.\n",
    )?;
    fs::write(
        path.join("moves.toml"),
        "[[move]]\nid = \"overlay_recall\"\ntask_type = \"recall\"\ntemplate = \"Explain {{concept}} in your own words.\"\n",
    )?;
    Ok(())
}

fn materialize_active_entities(
    conn: &Connection,
    base_pack_id: &str,
    version_id: &str,
    entities: &[(SuggestionPayload, String)],
) -> Result<()> {
    clear_active_entities(conn, base_pack_id)?;
    let max_seed: i64 = conn.query_row(
        "SELECT COALESCE(MAX(seed_order), 0) FROM concepts WHERE pack=?1",
        [base_pack_id],
        |row| row.get(0),
    )?;
    let provenance = format!("overlay:{version_id}");
    for (index, (payload, evidence_id)) in entities.iter().enumerate() {
        let evidence_json = serde_json::to_string(&[evidence_id])?;
        match payload {
            SuggestionPayload::Concept {
                id,
                name,
                concept_kind,
                generativity,
                p_init,
                ..
            } => {
                conn.execute(
                    "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, generativity, provenance, evidence_ids_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                    params![id, base_pack_id, name, concept_kind, max_seed + index as i64 + 1, p_init, generativity, provenance, evidence_json],
                )?;
            }
            SuggestionPayload::Schema { id, name, .. } => {
                conn.execute(
                    "INSERT INTO concepts(id, pack, name, kind, seed_order, generativity, provenance, evidence_ids_json, created_at)
                     VALUES (?1, ?2, ?3, 'schema', ?4, 'unknown', ?5, ?6, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                    params![id, base_pack_id, name, max_seed + index as i64 + 1, provenance, evidence_json],
                )?;
            }
            SuggestionPayload::TypedEdge {
                id,
                src,
                dst,
                edge_type,
                weight,
            } => {
                conn.execute(
                    "INSERT INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                    params![id, src, dst, edge_type, weight, provenance, evidence_json],
                )?;
            }
            SuggestionPayload::Misconception { .. } => {}
        }
    }
    Ok(())
}

fn clear_active_entities(conn: &Connection, base_pack_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM edges WHERE provenance LIKE 'overlay:%' AND (
            src IN (SELECT id FROM concepts WHERE pack=?1) OR
            dst IN (SELECT id FROM concepts WHERE pack=?1)
         )",
        [base_pack_id],
    )?;
    conn.execute(
        "DELETE FROM concepts WHERE pack=?1 AND provenance LIKE 'overlay:%'",
        [base_pack_id],
    )?;
    Ok(())
}

fn append_concept(
    output: &mut String,
    id: &str,
    name: &str,
    kind: &str,
    seed_order: i64,
    p_init: Option<f64>,
    generativity: &str,
) {
    output.push_str(&format!(
        "[[concept]]\nid = {}\nname = {}\nkind = {}\nseed_order = {seed_order}\ngenerativity = {}\n",
        toml_string(id),
        toml_string(name),
        toml_string(kind),
        toml_string(generativity)
    ));
    if let Some(p_init) = p_init {
        output.push_str(&format!("p_init = {p_init}\n"));
    }
    output.push('\n');
}

fn append_edge(output: &mut String, id: &str, src: &str, dst: &str, edge_type: &str, weight: f64) {
    output.push_str(&format!(
        "[[edge]]\nid = {}\nsrc = {}\ndst = {}\ntype = {}\nweight = {weight}\n\n",
        toml_string(id),
        toml_string(src),
        toml_string(dst),
        toml_string(edge_type)
    ));
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("JSON and TOML basic strings share escaping")
}

fn has_cycle(nodes: &HashSet<String>, edges: &[(String, String)]) -> bool {
    let mut indegree = nodes
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<HashMap<_, _>>();
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for (src, dst) in edges {
        if let Some(value) = indegree.get_mut(dst) {
            *value += 1;
        }
        outgoing.entry(src).or_default().push(dst);
    }
    let mut stack = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(id) = stack.pop() {
        visited += 1;
        if let Some(targets) = outgoing.get(id.as_str()) {
            for target in targets {
                if let Some(value) = indegree.get_mut(*target) {
                    *value -= 1;
                    if *value == 0 {
                        stack.push((*target).to_owned());
                    }
                }
            }
        }
    }
    visited != nodes.len()
}

fn validate_payload(payload: &SuggestionPayload) -> Result<()> {
    required("suggestion.payload.id", payload.entity_id())?;
    match payload {
        SuggestionPayload::Concept {
            name,
            concept_kind,
            generativity,
            p_init,
            ..
        } => {
            required("suggestion.payload.name", name)?;
            if !is_valid_concept_kind(concept_kind) {
                return Err(invalid("suggestion.payload.concept_kind", concept_kind));
            }
            if !matches!(generativity.as_str(), "generative" | "item" | "unknown") {
                return Err(invalid("suggestion.payload.generativity", generativity));
            }
            if p_init.is_some_and(|value| !(0.0..=1.0).contains(&value)) {
                return Err(invalid("suggestion.payload.p_init", "must be within [0,1]"));
            }
        }
        SuggestionPayload::Schema { name, .. } => {
            required("suggestion.payload.name", name)?;
        }
        SuggestionPayload::TypedEdge {
            src,
            dst,
            edge_type,
            weight,
            ..
        } => {
            required("suggestion.payload.src", src)?;
            required("suggestion.payload.dst", dst)?;
            if !is_valid_edge_type(edge_type) {
                return Err(invalid("suggestion.payload.edge_type", edge_type));
            }
            if !weight.is_finite() || *weight <= 0.0 {
                return Err(invalid("suggestion.payload.weight", "must be positive"));
            }
        }
        SuggestionPayload::Misconception {
            concept_id, title, ..
        } => {
            required("suggestion.payload.concept_id", concept_id)?;
            required("suggestion.payload.title", title)?;
        }
    }
    Ok(())
}

fn load_suggestion(conn: &Connection, suggestion_id: &str) -> Result<ConceptSuggestion> {
    conn.query_row(
        "SELECT id, capture_id, evidence_id, base_pack_id, kind, status,
                payload_json, quote, reason, model_version, created_at
         FROM concept_suggestions WHERE id=?1",
        [suggestion_id],
        |row| {
            let kind: String = row.get(4)?;
            let payload_json: String = row.get(6)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                kind,
                row.get::<_, String>(5)?,
                payload_json,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        },
    )
    .optional()?
    .ok_or_else(|| invalid("suggestion.id", "suggestion not found"))
    .and_then(
        |(
            id,
            capture_id,
            evidence_id,
            base_pack_id,
            kind,
            status,
            payload_json,
            quote,
            reason,
            model_version,
            created_at,
        )| {
            Ok(ConceptSuggestion {
                id,
                capture_id,
                evidence_id,
                base_pack_id,
                kind: SuggestionKind::parse(&kind)
                    .ok_or_else(|| invalid("suggestion.kind", &kind))?,
                status,
                payload: serde_json::from_str(&payload_json)?,
                quote,
                reason,
                model_version,
                created_at,
            })
        },
    )
}

fn payload_summary(payload: &SuggestionPayload) -> String {
    match payload {
        SuggestionPayload::Concept { name, .. } => format!("新增知识点「{name}」"),
        SuggestionPayload::Schema { name, .. } => format!("新增图式「{name}」"),
        SuggestionPayload::TypedEdge {
            src,
            dst,
            edge_type,
            ..
        } => format!("新增 {edge_type} 关系：{src} → {dst}"),
        SuggestionPayload::Misconception { title, .. } => format!("记录误解「{title}」"),
    }
}

fn concept_exists(conn: &Connection, id: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM concepts WHERE id=?1)",
        [id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn base_concept_exists(conn: &Connection, id: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM concepts WHERE id=?1 AND COALESCE(provenance, '') NOT LIKE 'overlay:%')",
        [id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn base_edge_exists(conn: &Connection, id: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM edges WHERE id=?1 AND COALESCE(provenance, '') NOT LIKE 'overlay:%')",
        [id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn base_pack_exists(conn: &Connection, base_pack_id: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM concepts WHERE pack=?1)",
        [base_pack_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn table_count(conn: &Connection, table: &str) -> Result<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn finish_transaction<T>(conn: &Connection, result: Result<T>) -> Result<T> {
    match result {
        Ok(value) => {
            conn.execute_batch("COMMIT")?;
            Ok(value)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn required(key: &str, value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid(key, "must not be empty"));
    }
    Ok(value.to_owned())
}

fn invalid(key: &str, value: &str) -> PolarisError {
    PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: value.to_owned(),
    }
}

fn default_concept_kind() -> String {
    "concept".to_owned()
}

fn default_generativity() -> String {
    "unknown".to_owned()
}

fn default_weight() -> f64 {
    1.0
}
