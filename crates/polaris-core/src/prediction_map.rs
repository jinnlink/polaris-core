use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::meta_f64;
use crate::engine::TaskAssignment;
use crate::error::Result;
use crate::graph::EDGE_MAPS_TO;
use crate::knowledge_map::{
    KnowledgeMapDimensionSummary, KnowledgeMapDueStatus, KnowledgeMapEdgeProvenance,
    KnowledgeMapGateStatus, KnowledgeMapNode, KnowledgeMapPackSummary, KnowledgeMapProvenance,
    KnowledgeMapScope, KnowledgeMapSnapshot, KnowledgeMapStateSource,
};
use crate::mirt::{latent_prediction, latent_prediction_variance};
use crate::pack_state::{theta_mode_for_pack, ThetaMode};

pub use crate::knowledge_map::KnowledgeMapQuery as PredictionMapQuery;

pub const PREDICTION_MAP_MODEL_VERSION: &str = "prediction-map-v1";
const INTERVAL_LEVEL: f64 = 0.95;
const NORMAL_95_Z: f64 = 1.959_963_984_540_054;
const MAX_ANCHORS: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionMapSnapshot {
    pub generated_at: String,
    pub model_version: String,
    pub query: PredictionMapQuery,
    pub summary: PredictionMapSummary,
    pub nodes: Vec<PredictionMapNode>,
    pub anchors: Vec<PredictionAnchor>,
    pub initial_paths: Vec<PredictionPath>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionMapSummary {
    pub scope: KnowledgeMapScope,
    pub resolved_pack: Option<String>,
    pub theta_mode: Option<String>,
    pub cross_domain_enabled: bool,
    pub total_nodes: usize,
    pub returned_nodes: usize,
    pub observed_nodes: usize,
    pub latent_prediction_nodes: usize,
    pub inherited_prior_nodes: usize,
    pub shadow_predictions: usize,
    pub unfit_predictions: usize,
    pub anchor_count: usize,
    pub path_count: usize,
    pub packs: Vec<KnowledgeMapPackSummary>,
    pub dimensions: Vec<KnowledgeMapDimensionSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionMapNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub pack: Option<String>,
    pub phase: String,
    pub due_status: KnowledgeMapDueStatus,
    pub attempt_count: usize,
    pub evidence_count: usize,
    pub observed: Option<PredictionEstimate>,
    pub latent_prediction: Option<PredictionEstimate>,
    pub inherited_prior: Option<PredictionEstimate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionEstimate {
    pub value: f64,
    pub interval: PredictionInterval,
    pub source: KnowledgeMapStateSource,
    pub gate_status: KnowledgeMapGateStatus,
    pub model_version: String,
    pub theta_scope: Option<String>,
    pub cross_domain: bool,
    pub provenance: KnowledgeMapProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionInterval {
    pub lower: f64,
    pub upper: f64,
    pub level: f64,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionAnchor {
    pub id: String,
    pub source_concept_id: String,
    pub source_name: String,
    pub source_pack: Option<String>,
    pub target_id: String,
    pub target_name: String,
    pub target_pack: Option<String>,
    pub structural_score: f64,
    pub difference: String,
    pub provenance: KnowledgeMapEdgeProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionPath {
    pub rank: usize,
    pub concept_id: String,
    pub concept_name: String,
    pub pack: Option<String>,
    #[serde(rename = "move")]
    pub move_name: String,
    pub task_type: String,
    pub prompt: String,
    pub phase: String,
    pub expected_success: f64,
    pub anchor_id: Option<String>,
}

#[derive(Debug)]
struct SeedPrior {
    value: f64,
    origin: Option<String>,
    evidence_ids: Vec<String>,
}

pub(crate) fn prediction_map_snapshot(
    conn: &Connection,
    knowledge: KnowledgeMapSnapshot,
    assignments: Vec<TaskAssignment>,
) -> Result<PredictionMapSnapshot> {
    let resolved_pack = knowledge.summary.resolved_pack.clone();
    let packs = knowledge.summary.packs.clone();
    let dimensions = knowledge.summary.dimensions.clone();
    let theta_mode = resolved_pack
        .as_deref()
        .map(|pack| theta_mode_for_pack(conn, pack))
        .transpose()?;
    let cross_domain_enabled = theta_mode == Some(ThetaMode::Shared);
    let theta_scope = theta_mode.map(|mode| match mode {
        ThetaMode::Shared => "shared".to_owned(),
        ThetaMode::Isolated => format!("pack:{}", resolved_pack.as_deref().unwrap_or_default()),
    });
    let anchors = match resolved_pack.as_deref() {
        Some(pack) => load_prediction_anchors(conn, pack)?,
        None => Vec::new(),
    };
    let mut nodes = Vec::with_capacity(knowledge.nodes.len());
    for node in knowledge.nodes {
        nodes.push(prediction_node(
            conn,
            node,
            theta_scope.as_deref(),
            cross_domain_enabled,
        )?);
    }
    let initial_paths = assignments
        .into_iter()
        .enumerate()
        .map(|(index, assignment)| PredictionPath {
            rank: index + 1,
            concept_id: assignment.concept_id,
            concept_name: assignment.concept_name,
            pack: resolved_pack.clone(),
            move_name: assignment.move_name,
            task_type: assignment.task_type,
            prompt: assignment.template,
            phase: assignment.phase.as_str().to_owned(),
            expected_success: assignment.expected_success,
            anchor_id: None,
        })
        .collect::<Vec<_>>();
    let observed_nodes = nodes.iter().filter(|node| node.observed.is_some()).count();
    let latent_prediction_nodes = nodes
        .iter()
        .filter(|node| node.latent_prediction.is_some())
        .count();
    let inherited_prior_nodes = nodes
        .iter()
        .filter(|node| node.inherited_prior.is_some())
        .count();
    let shadow_predictions = nodes
        .iter()
        .filter(|node| {
            node.latent_prediction
                .as_ref()
                .is_some_and(|estimate| estimate.gate_status == KnowledgeMapGateStatus::Shadow)
        })
        .count();
    let unfit_predictions = nodes
        .iter()
        .filter(|node| {
            node.latent_prediction
                .as_ref()
                .is_some_and(|estimate| estimate.gate_status == KnowledgeMapGateStatus::Unfit)
        })
        .count();

    Ok(PredictionMapSnapshot {
        generated_at: knowledge.generated_at,
        model_version: PREDICTION_MAP_MODEL_VERSION.to_owned(),
        query: knowledge.query,
        summary: PredictionMapSummary {
            scope: knowledge.summary.scope,
            resolved_pack,
            theta_mode: theta_mode.map(|mode| mode.as_str().to_owned()),
            cross_domain_enabled,
            total_nodes: knowledge.summary.total_nodes,
            returned_nodes: nodes.len(),
            observed_nodes,
            latent_prediction_nodes,
            inherited_prior_nodes,
            shadow_predictions,
            unfit_predictions,
            anchor_count: anchors.len(),
            path_count: initial_paths.len(),
            packs,
            dimensions,
        },
        nodes,
        anchors,
        initial_paths,
        next_cursor: knowledge.next_cursor,
    })
}

fn prediction_node(
    conn: &Connection,
    node: KnowledgeMapNode,
    theta_scope: Option<&str>,
    cross_domain: bool,
) -> Result<PredictionMapNode> {
    let seed = seed_prior(conn, &node.id)?;
    let inherited_prior = Some(PredictionEstimate {
        value: seed.value,
        interval: bernoulli_interval(seed.value),
        source: KnowledgeMapStateSource::InheritedPrior,
        gate_status: KnowledgeMapGateStatus::PriorOnly,
        model_version: "pack-seed-prior-v1".to_owned(),
        theta_scope: None,
        cross_domain: false,
        provenance: KnowledgeMapProvenance {
            source: KnowledgeMapStateSource::InheritedPrior,
            gate_status: KnowledgeMapGateStatus::PriorOnly,
            complete: seed.origin.is_some(),
            origin: seed.origin,
            evidence_ids: seed.evidence_ids,
        },
    });
    let observed = (node.attempt_count > 0).then(|| PredictionEstimate {
        value: node.p_known,
        interval: normal_interval(
            node.p_known,
            Some(node.uncertainty.p_known_variance),
            &node.uncertainty.method,
        ),
        source: KnowledgeMapStateSource::Observed,
        gate_status: KnowledgeMapGateStatus::Active,
        model_version: "knowledge-map-observed-v1".to_owned(),
        theta_scope: None,
        cross_domain: false,
        provenance: node.provenance.clone(),
    });
    let latent = latent_prediction(conn, &node.id, "recall")?;
    let variance = latent_prediction_variance(conn, &node.id, latent.p_hat);
    let gate_status = if variance.is_some() {
        KnowledgeMapGateStatus::Shadow
    } else {
        KnowledgeMapGateStatus::Unfit
    };
    let latent_prediction = Some(PredictionEstimate {
        value: latent.p_hat,
        interval: normal_interval(latent.p_hat, variance, "mirt_q_theta_information"),
        source: KnowledgeMapStateSource::LatentPrediction,
        gate_status,
        model_version: format!("mirt-v{}", latent.theta_version),
        theta_scope: theta_scope.map(str::to_owned),
        cross_domain,
        provenance: KnowledgeMapProvenance {
            source: KnowledgeMapStateSource::LatentPrediction,
            gate_status,
            origin: Some("mirt:q·theta-b-d_t".to_owned()),
            evidence_ids: Vec::new(),
            complete: variance.is_some(),
        },
    });

    Ok(PredictionMapNode {
        id: node.id,
        name: node.name,
        kind: node.kind,
        pack: node.pack,
        phase: node.phase,
        due_status: node.due_status,
        attempt_count: node.attempt_count,
        evidence_count: node.evidence_count,
        observed,
        latent_prediction,
        inherited_prior,
    })
}

fn seed_prior(conn: &Connection, concept_id: &str) -> Result<SeedPrior> {
    let (value, origin, evidence_json): (f64, Option<String>, Option<String>) = conn.query_row(
        "SELECT COALESCE(p_init, CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL)),
                provenance, evidence_ids_json
         FROM concepts WHERE id=?1",
        [concept_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    Ok(SeedPrior {
        value: value.clamp(0.0, 1.0),
        origin: origin.filter(|value| !value.trim().is_empty()),
        evidence_ids: parse_evidence_ids(evidence_json.as_deref())?,
    })
}

fn load_prediction_anchors(conn: &Connection, target_pack: &str) -> Result<Vec<PredictionAnchor>> {
    let threshold = meta_f64(conn, "graph.struct_threshold")?;
    let mut stmt = conn.prepare(
        "SELECT e.id, e.weight, e.alignment_json, e.provenance, e.evidence_ids_json,
                src.id, src.name, src.pack, COALESCE(src_ms.attempt_count, 0),
                dst.id, dst.name, dst.pack, COALESCE(dst_ms.attempt_count, 0)
         FROM edges e
         JOIN concepts src ON src.id=e.src
         JOIN concepts dst ON dst.id=e.dst
         LEFT JOIN mastery_states src_ms ON src_ms.concept_id=src.id
         LEFT JOIN mastery_states dst_ms ON dst_ms.concept_id=dst.id
         WHERE e.type=?1 AND COALESCE(e.weight, 0.0)>=?2
           AND e.provenance IS NOT NULL AND trim(e.provenance)<>''
           AND ((dst.pack=?3 AND COALESCE(src.pack, '')<>?3)
             OR (src.pack=?3 AND COALESCE(dst.pack, '')<>?3))
         ORDER BY e.weight DESC, e.id ASC",
    )?;
    let rows = stmt.query_map(params![EDGE_MAPS_TO, threshold, target_pack], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, i64>(12)?,
        ))
    })?;
    let mut anchors = Vec::new();
    for row in rows {
        let (
            id,
            score,
            alignment_json,
            origin,
            evidence_json,
            src_id,
            src_name,
            src_pack,
            src_attempts,
            dst_id,
            dst_name,
            dst_pack,
            dst_attempts,
        ) = row?;
        let (
            source_concept_id,
            source_name,
            source_pack,
            source_attempts,
            target_id,
            target_name,
            resolved_target_pack,
        ) = if dst_pack.as_deref() == Some(target_pack) {
            (
                src_id,
                src_name,
                src_pack,
                src_attempts,
                dst_id,
                dst_name,
                dst_pack,
            )
        } else {
            (
                dst_id,
                dst_name,
                dst_pack,
                dst_attempts,
                src_id,
                src_name,
                src_pack,
            )
        };
        if source_attempts <= 0 {
            continue;
        }
        let evidence_ids = parse_evidence_ids(evidence_json.as_deref())?;
        let difference = anchor_difference(&source_name, &target_name, alignment_json.as_deref());
        anchors.push(PredictionAnchor {
            id,
            source_concept_id,
            source_name,
            source_pack,
            target_id,
            target_name,
            target_pack: resolved_target_pack,
            structural_score: score,
            difference,
            provenance: KnowledgeMapEdgeProvenance {
                origin,
                evidence_ids,
            },
        });
        if anchors.len() == MAX_ANCHORS {
            break;
        }
    }
    Ok(anchors)
}

fn anchor_difference(source: &str, target: &str, alignment_json: Option<&str>) -> String {
    let alignment =
        alignment_json.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
    let matched = alignment
        .as_ref()
        .and_then(|value| value.get("matched_edges"))
        .and_then(serde_json::Value::as_u64);
    let total = alignment
        .as_ref()
        .and_then(|value| value.get("total_edges"))
        .and_then(serde_json::Value::as_u64);
    match (matched, total) {
        (Some(matched), Some(total)) => format!(
            "Use {source} as a structural anchor for {target}; {matched}/{total} relations align, and the remaining relations must be learned as differences."
        ),
        _ => format!(
            "Use {source} as a structural anchor for {target}, then verify the target-specific differences before transfer."
        ),
    }
}

fn bernoulli_interval(value: f64) -> PredictionInterval {
    normal_interval(value, Some(value * (1.0 - value)), "bernoulli_prior")
}

fn normal_interval(value: f64, variance: Option<f64>, method: &str) -> PredictionInterval {
    let Some(variance) = variance.filter(|value| value.is_finite() && *value >= 0.0) else {
        return PredictionInterval {
            lower: 0.0,
            upper: 1.0,
            level: INTERVAL_LEVEL,
            method: format!("{method}:unavailable"),
        };
    };
    let radius = NORMAL_95_Z * variance.sqrt();
    PredictionInterval {
        lower: (value - radius).clamp(0.0, 1.0),
        upper: (value + radius).clamp(0.0, 1.0),
        level: INTERVAL_LEVEL,
        method: format!("{method}:normal_approx"),
    }
}

fn parse_evidence_ids(raw: Option<&str>) -> Result<Vec<String>> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    let mut values = serde_json::from_str::<Vec<String>>(raw)?;
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
    Ok(values)
}
