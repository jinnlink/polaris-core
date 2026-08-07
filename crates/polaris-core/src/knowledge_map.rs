use std::collections::{BTreeSet, VecDeque};

use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::config::meta_f64;
use crate::error::{PolarisError, Result};
use crate::fsrs::{retrievability, FsrsState};
use crate::mirt::{bkt_p_known_variance, decode_vector};
use crate::pack_state::{active_pack, ensure_known_pack, theta_mode_for_pack};
use crate::phase::Phase;

pub const KNOWLEDGE_MAP_MODEL_VERSION: &str = "knowledge-map-v1";
pub const KNOWLEDGE_MAP_DEFAULT_LIMIT: usize = 100;
pub const KNOWLEDGE_MAP_MAX_LIMIT: usize = 500;
pub const KNOWLEDGE_MAP_MAX_DEPTH: u32 = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeMapScope {
    #[default]
    Pack,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeMapDueStatus {
    New,
    Due,
    Scheduled,
    Unscheduled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KnowledgeMapQuery {
    pub scope: KnowledgeMapScope,
    pub pack: Option<String>,
    pub root: Option<String>,
    pub depth: Option<u32>,
    pub phase: Option<String>,
    pub due: Option<KnowledgeMapDueStatus>,
    pub min_confidence: Option<f64>,
    pub limit: usize,
    pub cursor: Option<String>,
}

impl Default for KnowledgeMapQuery {
    fn default() -> Self {
        Self {
            scope: KnowledgeMapScope::Pack,
            pack: None,
            root: None,
            depth: None,
            phase: None,
            due: None,
            min_confidence: None,
            limit: KNOWLEDGE_MAP_DEFAULT_LIMIT,
            cursor: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapSnapshot {
    pub generated_at: String,
    pub model_version: String,
    pub query: KnowledgeMapQuery,
    pub summary: KnowledgeMapSummary,
    pub nodes: Vec<KnowledgeMapNode>,
    pub edges: Vec<KnowledgeMapEdge>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapSummary {
    pub scope: KnowledgeMapScope,
    pub resolved_pack: Option<String>,
    pub total_nodes: usize,
    pub returned_nodes: usize,
    pub returned_edges: usize,
    pub concept_count: usize,
    pub schema_count: usize,
    pub due_count: usize,
    pub observed_count: usize,
    pub inherited_prior_count: usize,
    pub omitted_edges_missing_provenance: usize,
    pub packs: Vec<KnowledgeMapPackSummary>,
    pub dimensions: Vec<KnowledgeMapDimensionSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapPackSummary {
    pub id: Option<String>,
    pub title: Option<String>,
    pub theta_mode: Option<String>,
    pub concept_count: usize,
    pub schema_count: usize,
    pub due_count: usize,
    pub observed_count: usize,
    pub mean_p_known: f64,
    pub mean_confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapDimensionSummary {
    pub id: String,
    pub concept_count: usize,
    pub loading_weight: f64,
    pub mean_p_known: f64,
    pub mean_confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub pack: Option<String>,
    pub retrieval: Option<f64>,
    pub p_known: f64,
    pub calibration_gap: f64,
    pub brier_ewma: f64,
    pub last_depth: Option<String>,
    pub max_depth: Option<String>,
    pub phase: String,
    pub phase_label: String,
    pub phase_summary: String,
    pub next_due_at: Option<String>,
    pub due_status: KnowledgeMapDueStatus,
    pub attempt_count: usize,
    pub evidence_count: usize,
    pub uncertainty: KnowledgeMapUncertainty,
    pub provenance: KnowledgeMapProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapUncertainty {
    pub p_known_variance: f64,
    pub confidence: f64,
    pub method: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeMapStateSource {
    Observed,
    LatentPrediction,
    InheritedPrior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeMapGateStatus {
    Active,
    Shadow,
    Unfit,
    PriorOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapProvenance {
    pub source: KnowledgeMapStateSource,
    pub gate_status: KnowledgeMapGateStatus,
    pub origin: Option<String>,
    pub evidence_ids: Vec<String>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub weight: f64,
    pub provenance: KnowledgeMapEdgeProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeMapEdgeProvenance {
    pub origin: String,
    pub evidence_ids: Vec<String>,
}

struct QueryPlan {
    query: KnowledgeMapQuery,
    resolved_pack: Option<String>,
    allowed_ids: Option<BTreeSet<String>>,
    offset: usize,
    fuse_n0: f64,
}

struct SummaryCounts {
    total_nodes: usize,
    concept_count: usize,
    schema_count: usize,
    due_count: usize,
    observed_count: usize,
}

struct RawNode {
    id: String,
    name: String,
    kind: String,
    pack: Option<String>,
    p_known: f64,
    fsrs_json: Option<String>,
    elapsed_days: Option<f64>,
    next_due_at: Option<String>,
    calibration_gap: f64,
    brier_ewma: f64,
    last_depth: Option<String>,
    max_depth: Option<String>,
    phase: String,
    due_status: KnowledgeMapDueStatus,
    attempt_count: usize,
    evidence_count: usize,
    origin: Option<String>,
    evidence_ids_json: Option<String>,
}

#[derive(Default)]
struct DimensionAccumulator {
    concept_count: usize,
    loading_weight: f64,
    weighted_p_known: f64,
    weighted_confidence: f64,
}

pub fn knowledge_map_snapshot(
    conn: &Connection,
    query: KnowledgeMapQuery,
) -> Result<KnowledgeMapSnapshot> {
    let plan = prepare_query(conn, query)?;
    let generated_at =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
            row.get(0)
        })?;
    let counts = summary_counts(conn, &plan)?;
    let packs = pack_summaries(conn, &plan)?;
    let dimensions = dimension_summaries(conn, &plan)?;

    if plan.query.scope == KnowledgeMapScope::Global {
        return Ok(KnowledgeMapSnapshot {
            generated_at,
            model_version: KNOWLEDGE_MAP_MODEL_VERSION.to_owned(),
            query: plan.query.clone(),
            summary: KnowledgeMapSummary {
                scope: plan.query.scope,
                resolved_pack: None,
                total_nodes: counts.total_nodes,
                returned_nodes: 0,
                returned_edges: 0,
                concept_count: counts.concept_count,
                schema_count: counts.schema_count,
                due_count: counts.due_count,
                observed_count: counts.observed_count,
                inherited_prior_count: counts.total_nodes.saturating_sub(counts.observed_count),
                omitted_edges_missing_provenance: 0,
                packs,
                dimensions,
            },
            nodes: Vec::new(),
            edges: Vec::new(),
            next_cursor: None,
        });
    }

    let nodes = load_nodes(conn, &plan)?;
    let node_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let (edges, omitted_edges_missing_provenance) = load_edges(conn, &node_ids)?;
    let next_offset = plan.offset.saturating_add(nodes.len());
    let next_cursor = (next_offset < counts.total_nodes).then(|| format_cursor(next_offset));

    Ok(KnowledgeMapSnapshot {
        generated_at,
        model_version: KNOWLEDGE_MAP_MODEL_VERSION.to_owned(),
        query: plan.query.clone(),
        summary: KnowledgeMapSummary {
            scope: plan.query.scope,
            resolved_pack: plan.resolved_pack,
            total_nodes: counts.total_nodes,
            returned_nodes: nodes.len(),
            returned_edges: edges.len(),
            concept_count: counts.concept_count,
            schema_count: counts.schema_count,
            due_count: counts.due_count,
            observed_count: counts.observed_count,
            inherited_prior_count: counts.total_nodes.saturating_sub(counts.observed_count),
            omitted_edges_missing_provenance,
            packs,
            dimensions,
        },
        nodes,
        edges,
        next_cursor,
    })
}

fn prepare_query(conn: &Connection, mut query: KnowledgeMapQuery) -> Result<QueryPlan> {
    query.pack = normalized_optional(query.pack);
    query.root = normalized_optional(query.root);
    query.phase = normalized_optional(query.phase);

    if query.limit == 0 || query.limit > KNOWLEDGE_MAP_MAX_LIMIT {
        return Err(invalid_parameter(
            "knowledge_map.limit",
            format!(
                "expected 1..={KNOWLEDGE_MAP_MAX_LIMIT}, got {}",
                query.limit
            ),
        ));
    }
    if let Some(depth) = query.depth {
        if query.root.is_none() || depth > KNOWLEDGE_MAP_MAX_DEPTH {
            return Err(invalid_parameter(
                "knowledge_map.depth",
                format!("requires root and must be <= {KNOWLEDGE_MAP_MAX_DEPTH}"),
            ));
        }
    }
    if let Some(phase) = query.phase.as_deref() {
        if Phase::parse(phase).is_none() {
            return Err(invalid_parameter("knowledge_map.phase", phase.to_owned()));
        }
    }
    if let Some(confidence) = query.min_confidence {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(invalid_parameter(
                "knowledge_map.min_confidence",
                confidence.to_string(),
            ));
        }
    }

    let offset = query
        .cursor
        .as_deref()
        .map(parse_cursor)
        .transpose()?
        .unwrap_or(0);
    let fuse_n0 = meta_f64(conn, "mirt.fuse_n0")?;
    let resolved_pack = match query.scope {
        KnowledgeMapScope::Pack => {
            if let Some(pack) = query.pack.as_deref() {
                ensure_known_pack(conn, pack)?;
                Some(pack.to_owned())
            } else {
                active_pack(conn)?
            }
        }
        KnowledgeMapScope::Global => {
            if query.pack.is_some() || query.root.is_some() || query.depth.is_some() {
                return Err(invalid_parameter(
                    "knowledge_map.scope",
                    "global scope cannot be combined with pack/root/depth",
                ));
            }
            if query.cursor.is_some() {
                return Err(invalid_parameter(
                    "knowledge_map.cursor",
                    "global scope is aggregate-only and does not paginate",
                ));
            }
            None
        }
    };
    let allowed_ids = if let Some(root) = query.root.as_deref() {
        ensure_root_in_scope(conn, root, resolved_pack.as_deref())?;
        Some(reachable_ids(
            conn,
            root,
            query.depth.unwrap_or(1),
            resolved_pack.as_deref(),
        )?)
    } else {
        None
    };

    Ok(QueryPlan {
        query,
        resolved_pack,
        allowed_ids,
        offset,
        fuse_n0,
    })
}

fn summary_counts(conn: &Connection, plan: &QueryPlan) -> Result<SummaryCounts> {
    let (where_sql, params) = filters(plan);
    let sql = format!(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN c.kind='schema' THEN 0 ELSE 1 END), 0),
                COALESCE(SUM(CASE WHEN c.kind='schema' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN COALESCE(ms.attempt_count, 0) > 0
                                      AND ms.next_due_at IS NOT NULL
                                      AND julianday(ms.next_due_at) <= julianday('now')
                                 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN COALESCE(ms.attempt_count, 0) > 0 THEN 1 ELSE 0 END), 0)
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         WHERE {where_sql}"
    );
    conn.query_row(&sql, params_from_iter(params.iter()), |row| {
        Ok(SummaryCounts {
            total_nodes: non_negative_usize(row.get::<_, i64>(0)?),
            concept_count: non_negative_usize(row.get::<_, i64>(1)?),
            schema_count: non_negative_usize(row.get::<_, i64>(2)?),
            due_count: non_negative_usize(row.get::<_, i64>(3)?),
            observed_count: non_negative_usize(row.get::<_, i64>(4)?),
        })
    })
    .map_err(Into::into)
}

fn load_nodes(conn: &Connection, plan: &QueryPlan) -> Result<Vec<KnowledgeMapNode>> {
    let (where_sql, mut params) = filters(plan);
    let sql = format!(
        "SELECT c.id, c.name, c.kind, c.pack,
                COALESCE(ms.p_known, c.p_init,
                         CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                ms.fsrs_json,
                CASE WHEN ms.last_review_at IS NULL THEN NULL
                     ELSE julianday('now') - julianday(ms.last_review_at) END,
                ms.next_due_at,
                COALESCE(ms.calib_gap, 0.0),
                COALESCE(ms.brier_ewma, 0.0),
                ms.last_depth,
                ms.max_depth,
                COALESCE(ms.phase, 'undetermined'),
                CASE WHEN COALESCE(ms.attempt_count, 0)=0 THEN 'new'
                     WHEN ms.next_due_at IS NULL THEN 'unscheduled'
                     WHEN julianday(ms.next_due_at)<=julianday('now') THEN 'due'
                     ELSE 'scheduled' END,
                COALESCE(ms.attempt_count, 0),
                (SELECT COUNT(*) FROM (
                    SELECT a.response_evidence_id AS evidence_id
                    FROM attempts a
                    WHERE a.concept_id=c.id AND a.response_evidence_id IS NOT NULL
                    UNION
                    SELECT CAST(je.value AS TEXT)
                    FROM json_each(CASE WHEN json_valid(c.evidence_ids_json)
                                        THEN c.evidence_ids_json ELSE '[]' END) je
                    UNION
                    SELECT ei.id
                    FROM evidence_items ei,
                         json_each(CASE WHEN json_valid(ei.concept_ids_json)
                                        THEN ei.concept_ids_json ELSE '[]' END) je
                    WHERE CAST(je.value AS TEXT)=c.id
                )),
                c.provenance,
                (SELECT json_group_array(evidence_id) FROM (
                    SELECT a.response_evidence_id AS evidence_id
                    FROM attempts a
                    WHERE a.concept_id=c.id AND a.response_evidence_id IS NOT NULL
                    UNION
                    SELECT CAST(je.value AS TEXT)
                    FROM json_each(CASE WHEN json_valid(c.evidence_ids_json)
                                        THEN c.evidence_ids_json ELSE '[]' END) je
                    UNION
                    SELECT ei.id
                    FROM evidence_items ei,
                         json_each(CASE WHEN json_valid(ei.concept_ids_json)
                                        THEN ei.concept_ids_json ELSE '[]' END) je
                    WHERE CAST(je.value AS TEXT)=c.id
                ))
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         WHERE {where_sql}
         ORDER BY c.seed_order ASC, c.id ASC
         LIMIT ? OFFSET ?"
    );
    params.push(SqlValue::Integer(plan.query.limit as i64));
    params.push(SqlValue::Integer(plan.offset as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), raw_node_from_row)?;
    let mut nodes = Vec::new();
    for row in rows {
        nodes.push(node_from_raw(row?, plan.fuse_n0)?);
    }
    Ok(nodes)
}

fn raw_node_from_row(row: &Row<'_>) -> rusqlite::Result<RawNode> {
    let due_status = match row.get::<_, String>(13)?.as_str() {
        "new" => KnowledgeMapDueStatus::New,
        "due" => KnowledgeMapDueStatus::Due,
        "scheduled" => KnowledgeMapDueStatus::Scheduled,
        _ => KnowledgeMapDueStatus::Unscheduled,
    };
    Ok(RawNode {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        pack: row.get(3)?,
        p_known: row.get(4)?,
        fsrs_json: row.get(5)?,
        elapsed_days: row.get(6)?,
        next_due_at: row.get(7)?,
        calibration_gap: row.get(8)?,
        brier_ewma: row.get(9)?,
        last_depth: row.get(10)?,
        max_depth: row.get(11)?,
        phase: row.get(12)?,
        due_status,
        attempt_count: non_negative_usize(row.get::<_, i64>(14)?),
        evidence_count: non_negative_usize(row.get::<_, i64>(15)?),
        origin: row.get(16)?,
        evidence_ids_json: row.get(17)?,
    })
}

fn node_from_raw(raw: RawNode, fuse_n0: f64) -> Result<KnowledgeMapNode> {
    if !raw.p_known.is_finite() || !(0.0..=1.0).contains(&raw.p_known) {
        return Err(invalid_parameter(
            "knowledge_map.p_known",
            format!("{}={}", raw.id, raw.p_known),
        ));
    }
    let phase = Phase::parse(&raw.phase).unwrap_or(Phase::Undetermined);
    let retrieval = raw
        .fsrs_json
        .as_deref()
        .map(serde_json::from_str::<FsrsState>)
        .transpose()?
        .map(|state| retrievability(state.stability, raw.elapsed_days.unwrap_or(0.0).max(0.0)));
    let confidence = evidence_confidence(raw.attempt_count, fuse_n0);
    let evidence_ids = parse_evidence_ids(raw.evidence_ids_json.as_deref())?;
    let seed_origin = raw.origin.filter(|value| !value.trim().is_empty());
    let (source, gate_status, origin, complete) = if raw.attempt_count > 0 {
        (
            KnowledgeMapStateSource::Observed,
            KnowledgeMapGateStatus::Active,
            Some("attempts+mastery_states".to_owned()),
            !evidence_ids.is_empty(),
        )
    } else {
        (
            KnowledgeMapStateSource::InheritedPrior,
            KnowledgeMapGateStatus::PriorOnly,
            seed_origin.clone(),
            seed_origin.is_some(),
        )
    };

    Ok(KnowledgeMapNode {
        id: raw.id,
        name: raw.name,
        kind: raw.kind,
        pack: raw.pack,
        retrieval,
        p_known: raw.p_known,
        calibration_gap: raw.calibration_gap,
        brier_ewma: raw.brier_ewma,
        last_depth: raw.last_depth,
        max_depth: raw.max_depth,
        phase: phase.as_str().to_owned(),
        phase_label: phase.label().to_owned(),
        phase_summary: phase.summary().to_owned(),
        next_due_at: raw.next_due_at,
        due_status: raw.due_status,
        attempt_count: raw.attempt_count,
        evidence_count: raw.evidence_count,
        uncertainty: KnowledgeMapUncertainty {
            p_known_variance: bkt_p_known_variance(raw.p_known, raw.attempt_count as f64),
            confidence,
            method: "bkt_bernoulli_approx+attempt_weight".to_owned(),
        },
        provenance: KnowledgeMapProvenance {
            source,
            gate_status,
            complete,
            origin,
            evidence_ids,
        },
    })
}

fn pack_summaries(conn: &Connection, plan: &QueryPlan) -> Result<Vec<KnowledgeMapPackSummary>> {
    let (where_sql, filter_params) = filters(plan);
    let mut params = vec![SqlValue::Real(plan.fuse_n0)];
    params.extend(filter_params);
    let sql = format!(
        "SELECT c.pack,
                CASE WHEN c.pack IS NULL OR c.pack='' THEN NULL
                     ELSE COALESCE((SELECT value FROM meta WHERE key='pack.' || c.pack || '.title'), c.pack)
                END,
                COALESCE(SUM(CASE WHEN c.kind='schema' THEN 0 ELSE 1 END), 0),
                COALESCE(SUM(CASE WHEN c.kind='schema' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN COALESCE(ms.attempt_count, 0)>0
                                      AND ms.next_due_at IS NOT NULL
                                      AND julianday(ms.next_due_at)<=julianday('now')
                                 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN COALESCE(ms.attempt_count, 0)>0 THEN 1 ELSE 0 END), 0),
                AVG(COALESCE(ms.p_known, c.p_init,
                             CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL))),
                AVG(CASE WHEN COALESCE(ms.attempt_count, 0)<=0 THEN 0.0
                         ELSE CAST(ms.attempt_count AS REAL) /
                              (CAST(ms.attempt_count AS REAL) + ?)
                    END)
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         WHERE {where_sql}
         GROUP BY c.pack
         ORDER BY c.pack ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            non_negative_usize(row.get::<_, i64>(2)?),
            non_negative_usize(row.get::<_, i64>(3)?),
            non_negative_usize(row.get::<_, i64>(4)?),
            non_negative_usize(row.get::<_, i64>(5)?),
            row.get::<_, f64>(6)?,
            row.get::<_, f64>(7)?,
        ))
    })?;
    let mut summaries = Vec::new();
    for row in rows {
        let (
            id,
            title,
            concept_count,
            schema_count,
            due_count,
            observed_count,
            mean_p_known,
            mean_confidence,
        ) = row?;
        let theta_mode = id
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(|pack| theta_mode_for_pack(conn, pack).map(|mode| mode.as_str().to_owned()))
            .transpose()?;
        summaries.push(KnowledgeMapPackSummary {
            id,
            title,
            theta_mode,
            concept_count,
            schema_count,
            due_count,
            observed_count,
            mean_p_known,
            mean_confidence,
        });
    }
    Ok(summaries)
}

fn dimension_summaries(
    conn: &Connection,
    plan: &QueryPlan,
) -> Result<Vec<KnowledgeMapDimensionSummary>> {
    let labels = latent_dimension_labels(conn)?;
    if labels.is_empty() {
        return Ok(Vec::new());
    }
    let (where_sql, params) = filters(plan);
    let sql = format!(
        "SELECT c.q,
                COALESCE(ms.p_known, c.p_init,
                         CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                COALESCE(ms.attempt_count, 0)
         FROM concepts c
         LEFT JOIN mastery_states ms ON ms.concept_id=c.id
         WHERE {where_sql} AND c.q IS NOT NULL
         ORDER BY c.seed_order ASC, c.id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, f64>(1)?,
            non_negative_usize(row.get::<_, i64>(2)?),
        ))
    })?;
    let mut accumulators = (0..labels.len())
        .map(|_| DimensionAccumulator::default())
        .collect::<Vec<_>>();
    for row in rows {
        let (q_blob, p_known, attempt_count) = row?;
        let q = decode_vector(&q_blob)?;
        let confidence = evidence_confidence(attempt_count, plan.fuse_n0);
        for (index, loading) in q.into_iter().take(labels.len()).enumerate() {
            let weight = loading.abs();
            if weight <= f64::EPSILON {
                continue;
            }
            let accumulator = &mut accumulators[index];
            accumulator.concept_count += 1;
            accumulator.loading_weight += weight;
            accumulator.weighted_p_known += weight * p_known;
            accumulator.weighted_confidence += weight * confidence;
        }
    }

    Ok(labels
        .into_iter()
        .zip(accumulators)
        .filter_map(|(id, accumulator)| {
            (accumulator.loading_weight > 0.0).then(|| KnowledgeMapDimensionSummary {
                id,
                concept_count: accumulator.concept_count,
                loading_weight: accumulator.loading_weight,
                mean_p_known: accumulator.weighted_p_known / accumulator.loading_weight,
                mean_confidence: accumulator.weighted_confidence / accumulator.loading_weight,
            })
        })
        .collect())
}

fn load_edges(conn: &Connection, node_ids: &[String]) -> Result<(Vec<KnowledgeMapEdge>, usize)> {
    if node_ids.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let placeholders = sql_placeholders(node_ids.len());
    let mut params = node_ids
        .iter()
        .cloned()
        .map(SqlValue::Text)
        .collect::<Vec<_>>();
    params.extend(node_ids.iter().cloned().map(SqlValue::Text));
    let endpoint_clause = format!("src IN ({placeholders}) AND dst IN ({placeholders})");
    let missing_sql = format!(
        "SELECT COUNT(*) FROM edges
         WHERE {endpoint_clause} AND (provenance IS NULL OR trim(provenance)='')"
    );
    let omitted = conn.query_row(&missing_sql, params_from_iter(params.iter()), |row| {
        row.get::<_, i64>(0)
    })?;
    let sql = format!(
        "SELECT id, src, dst, type, COALESCE(weight, 1.0), provenance, evidence_ids_json
         FROM edges
         WHERE {endpoint_clause} AND provenance IS NOT NULL AND trim(provenance)<>''
         ORDER BY type ASC, src ASC, dst ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, f64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    let mut edges = Vec::new();
    for row in rows {
        let (id, source_id, target_id, kind, weight, origin, evidence_ids_json) = row?;
        edges.push(KnowledgeMapEdge {
            id,
            source_id,
            target_id,
            kind,
            weight,
            provenance: KnowledgeMapEdgeProvenance {
                origin,
                evidence_ids: parse_evidence_ids(evidence_ids_json.as_deref())?,
            },
        });
    }
    Ok((edges, non_negative_usize(omitted)))
}

fn filters(plan: &QueryPlan) -> (String, Vec<SqlValue>) {
    let mut clauses = vec!["1=1".to_owned()];
    let mut params = Vec::new();
    if let Some(pack) = plan.resolved_pack.as_deref() {
        clauses.push("c.pack=?".to_owned());
        params.push(SqlValue::Text(pack.to_owned()));
    }
    if let Some(ids) = plan.allowed_ids.as_ref() {
        if ids.is_empty() {
            clauses.push("0=1".to_owned());
        } else {
            clauses.push(format!("c.id IN ({})", sql_placeholders(ids.len())));
            params.extend(ids.iter().cloned().map(SqlValue::Text));
        }
    }
    if let Some(phase) = plan.query.phase.as_deref() {
        clauses.push("COALESCE(ms.phase, 'undetermined')=?".to_owned());
        params.push(SqlValue::Text(phase.to_owned()));
    }
    if let Some(due) = plan.query.due {
        clauses.push(match due {
            KnowledgeMapDueStatus::New => "COALESCE(ms.attempt_count, 0)=0",
            KnowledgeMapDueStatus::Due => {
                "COALESCE(ms.attempt_count, 0)>0 AND ms.next_due_at IS NOT NULL AND julianday(ms.next_due_at)<=julianday('now')"
            }
            KnowledgeMapDueStatus::Scheduled => {
                "COALESCE(ms.attempt_count, 0)>0 AND ms.next_due_at IS NOT NULL AND julianday(ms.next_due_at)>julianday('now')"
            }
            KnowledgeMapDueStatus::Unscheduled => {
                "COALESCE(ms.attempt_count, 0)>0 AND ms.next_due_at IS NULL"
            }
        }
        .to_owned());
    }
    if let Some(confidence) = plan.query.min_confidence {
        clauses.push(
            "CASE WHEN COALESCE(ms.attempt_count, 0)<=0 THEN 0.0
                  ELSE CAST(ms.attempt_count AS REAL) /
                       (CAST(ms.attempt_count AS REAL) + ?)
             END >= ?"
                .to_owned(),
        );
        params.push(SqlValue::Real(plan.fuse_n0));
        params.push(SqlValue::Real(confidence));
    }
    (clauses.join(" AND "), params)
}

fn ensure_root_in_scope(conn: &Connection, root: &str, pack: Option<&str>) -> Result<()> {
    let root_pack = conn
        .query_row("SELECT pack FROM concepts WHERE id=?1", [root], |row| {
            row.get::<_, Option<String>>(0)
        })
        .optional()?;
    let Some(root_pack) = root_pack else {
        return Err(PolarisError::MissingConcept(root.to_owned()));
    };
    if let Some(pack) = pack {
        if root_pack.as_deref() != Some(pack) {
            return Err(invalid_parameter(
                "knowledge_map.root",
                format!("{root} is outside pack {pack}"),
            ));
        }
    }
    Ok(())
}

fn reachable_ids(
    conn: &Connection,
    root: &str,
    max_depth: u32,
    pack: Option<&str>,
) -> Result<BTreeSet<String>> {
    let mut visited = BTreeSet::from([root.to_owned()]);
    let mut queue = VecDeque::from([(root.to_owned(), 0u32)]);
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for neighbor in neighbors(conn, &node, pack)? {
            if visited.insert(neighbor.clone()) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }
    Ok(visited)
}

fn neighbors(conn: &Connection, node: &str, pack: Option<&str>) -> Result<Vec<String>> {
    let mut neighbors = BTreeSet::new();
    for (endpoint, other) in [("src", "dst"), ("dst", "src")] {
        let mut sql = format!(
            "SELECT e.{other}
             FROM edges e
             JOIN concepts other ON other.id=e.{other}
             WHERE e.{endpoint}=?1
               AND e.provenance IS NOT NULL AND trim(e.provenance)<>''"
        );
        if pack.is_some() {
            sql.push_str(" AND other.pack=?2");
        }
        let mut stmt = conn.prepare(&sql)?;
        if let Some(pack) = pack {
            let rows = stmt.query_map([node, pack], |row| row.get::<_, String>(0))?;
            for row in rows {
                neighbors.insert(row?);
            }
        } else {
            let rows = stmt.query_map([node], |row| row.get::<_, String>(0))?;
            for row in rows {
                neighbors.insert(row?);
            }
        }
    }
    Ok(neighbors.into_iter().collect())
}

fn latent_dimension_labels(conn: &Connection) -> Result<Vec<String>> {
    let json = conn
        .query_row(
            "SELECT value FROM meta WHERE key='latent.dims'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    json.map(|value| serde_json::from_str(&value).map_err(Into::into))
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn evidence_confidence(attempt_count: usize, fuse_n0: f64) -> f64 {
    if attempt_count == 0 {
        return 0.0;
    }
    if fuse_n0 <= 0.0 {
        return 1.0;
    }
    let n = attempt_count as f64;
    (n / (n + fuse_n0)).clamp(0.0, 1.0)
}

fn parse_evidence_ids(raw: Option<&str>) -> Result<Vec<String>> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(raw).map_err(Into::into)
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

fn format_cursor(offset: usize) -> String {
    format!("km1:{offset}")
}

fn parse_cursor(cursor: &str) -> Result<usize> {
    cursor
        .strip_prefix("km1:")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| invalid_parameter("knowledge_map.cursor", cursor.to_owned()))
}

fn sql_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn non_negative_usize(value: i64) -> usize {
    value.max(0) as usize
}

fn invalid_parameter(key: &str, value: impl Into<String>) -> PolarisError {
    PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: value.into(),
    }
}
