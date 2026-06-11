use std::collections::BTreeMap;
use std::env;

use hnsw_rs::prelude::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::config::meta_usize;
use crate::error::{PolarisError, Result};
use crate::graph::{
    structural_mapping_score, upsert_maps_to_candidate, StructuralMapping, CONCEPT_KIND_SCHEMA,
};
use crate::mirt::{decode_vector, encode_vector};

const MIN_COMMON_WEEKS: usize = 4;

pub trait EmbeddingProvider {
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f64>>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingRefreshSummary {
    pub disabled: bool,
    pub refreshed: usize,
    pub skipped: usize,
    pub dimension: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryCandidate {
    pub source: String,
    pub target: String,
    pub cos_e: f64,
    pub cos_q: f64,
    pub struct_score: f64,
    pub coh: f64,
    pub assoc: f64,
    pub discover: f64,
}

#[derive(Debug, Clone)]
struct EmbeddingConcept {
    id: String,
    kind: String,
    name: String,
}

#[derive(Debug, Clone)]
struct GeometryItem {
    id: String,
    embedding: Vec<f64>,
    q: Option<Vec<f64>>,
}

pub fn refresh_missing_embeddings(conn: &Connection) -> Result<EmbeddingRefreshSummary> {
    let Some(provider) = OpenAiEmbeddingProvider::from_env() else {
        return Ok(EmbeddingRefreshSummary {
            disabled: true,
            refreshed: 0,
            skipped: concept_count(conn)?,
            dimension: existing_embedding_dim(conn)?,
        });
    };
    refresh_missing_embeddings_with_provider(conn, &provider)
}

pub fn refresh_missing_embeddings_with_provider<P: EmbeddingProvider>(
    conn: &Connection,
    provider: &P,
) -> Result<EmbeddingRefreshSummary> {
    let concepts = concepts_missing_embedding(conn)?;
    let skipped = concept_count(conn)?.saturating_sub(concepts.len());
    if concepts.is_empty() {
        return Ok(EmbeddingRefreshSummary {
            disabled: false,
            refreshed: 0,
            skipped,
            dimension: existing_embedding_dim(conn)?,
        });
    }

    let inputs = concepts.iter().map(embedding_input).collect::<Vec<_>>();
    let embeddings = provider.embed(&inputs)?;
    if embeddings.len() != concepts.len() {
        return Err(PolarisError::InvalidParameter {
            key: "embedding.count".to_owned(),
            value: format!("{} != {}", embeddings.len(), concepts.len()),
        });
    }

    let mut dimension = existing_embedding_dim(conn)?;
    let mut normalized_embeddings = Vec::with_capacity(embeddings.len());
    for embedding in &embeddings {
        let normalized = normalize_embedding(embedding)?;
        if let Some(expected) = dimension {
            if expected != normalized.len() {
                return Err(PolarisError::InvalidParameter {
                    key: "embedding.dim".to_owned(),
                    value: format!("{} != {}", normalized.len(), expected),
                });
            }
        } else {
            dimension = Some(normalized.len());
        }
        normalized_embeddings.push(normalized);
    }

    let tx = conn.unchecked_transaction()?;
    if let Some(dimension) = dimension {
        tx.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('embedding.dim', ?1)",
            [dimension.to_string()],
        )?;
    }
    for (concept, normalized) in concepts.iter().zip(normalized_embeddings.iter()) {
        tx.execute(
            "UPDATE concepts SET embedding=?1 WHERE id=?2",
            params![encode_vector(normalized), concept.id],
        )?;
    }
    tx.commit()?;

    Ok(EmbeddingRefreshSummary {
        disabled: false,
        refreshed: concepts.len(),
        skipped,
        dimension,
    })
}

pub fn geometry_candidates(
    conn: &Connection,
    source: &str,
    limit: usize,
) -> Result<Vec<GeometryCandidate>> {
    ensure_concept_exists(conn, source)?;
    if !embedding_env_available() {
        return Ok(Vec::new());
    }
    if limit == 0 {
        return Ok(Vec::new());
    }

    let items = geometry_items(conn)?;
    let Some(source_idx) = items.iter().position(|item| item.id == source) else {
        return Ok(Vec::new());
    };

    let m = meta_usize(conn, "geometry.hnsw_m")?;
    let ef_search = meta_usize(conn, "geometry.ef_search")?;
    let neighbor_indices = hnsw_neighbor_indices(&items, source_idx, limit, m, ef_search);
    let source_item = &items[source_idx];
    let mut candidates = Vec::new();
    for idx in neighbor_indices {
        let target_item = &items[idx];
        let Some(cos_e) = cosine(&source_item.embedding, &target_item.embedding) else {
            continue;
        };
        let cos_q = match (&source_item.q, &target_item.q) {
            (Some(left), Some(right)) => cosine(left, right).unwrap_or(0.0),
            _ => 0.0,
        };
        let struct_score = structural_mapping_score(conn, source, &target_item.id)
            .map(|mapping| mapping.score)
            .unwrap_or(0.0);
        let coh = residual_correlation(conn, source, &target_item.id)?;
        let assoc = 0.15 * cos_e + 0.35 * cos_q + 0.25 * struct_score + 0.25 * coh;
        let discover = (0.35 * cos_q + 0.25 * struct_score + 0.25 * coh) * (1.0 - cos_e);
        candidates.push(GeometryCandidate {
            source: source.to_owned(),
            target: target_item.id.clone(),
            cos_e,
            cos_q,
            struct_score,
            coh,
            assoc,
            discover,
        });
    }
    candidates.sort_by(|left, right| {
        right
            .assoc
            .total_cmp(&left.assoc)
            .then_with(|| left.target.cmp(&right.target))
    });
    candidates.truncate(limit);
    Ok(candidates)
}

pub fn upsert_geometry_maps_to_candidates(
    conn: &Connection,
    source: &str,
    limit: usize,
) -> Result<Vec<StructuralMapping>> {
    let source_kind = concept_kind(conn, source)?;
    if source_kind != CONCEPT_KIND_SCHEMA {
        return Ok(Vec::new());
    }

    let mut mappings = Vec::new();
    for candidate in geometry_candidates(conn, source, limit)? {
        if concept_kind(conn, &candidate.target)? != CONCEPT_KIND_SCHEMA {
            continue;
        }
        if let Some(mapping) = upsert_maps_to_candidate(conn, source, &candidate.target)? {
            mappings.push(mapping);
        }
    }
    Ok(mappings)
}

pub struct OpenAiEmbeddingProvider {
    base_url: String,
    model: String,
    api_key: String,
    client: reqwest::blocking::Client,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingItem {
    index: usize,
    embedding: Vec<f64>,
}

impl OpenAiEmbeddingProvider {
    fn from_env() -> Option<Self> {
        let base_url = required_env("POLARIS_EMBED_BASE_URL")?;
        let model = required_env("POLARIS_EMBED_MODEL")?;
        let api_key = required_env("POLARIS_EMBED_API_KEY")?;
        Some(Self {
            base_url,
            model,
            api_key,
            client: reqwest::blocking::Client::new(),
        })
    }
}

fn embedding_env_available() -> bool {
    required_env("POLARIS_EMBED_BASE_URL").is_some()
        && required_env("POLARIS_EMBED_MODEL").is_some()
        && required_env("POLARIS_EMBED_API_KEY").is_some()
}

fn required_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

impl EmbeddingProvider for OpenAiEmbeddingProvider {
    fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f64>>> {
        #[derive(Serialize)]
        struct Request<'a> {
            model: &'a str,
            input: &'a [String],
        }

        #[derive(Deserialize)]
        struct Response {
            data: Vec<OpenAiEmbeddingItem>,
        }

        let url = format!("{}/v1/embeddings", self.base_url.trim_end_matches('/'));
        let data = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&Request {
                model: &self.model,
                input: inputs,
            })
            .send()?
            .error_for_status()?
            .json::<Response>()?
            .data;
        ordered_embeddings(data, inputs.len())
    }
}

fn ordered_embeddings(
    data: Vec<OpenAiEmbeddingItem>,
    expected_len: usize,
) -> Result<Vec<Vec<f64>>> {
    if data.len() != expected_len {
        return Err(PolarisError::InvalidParameter {
            key: "embedding.response_count".to_owned(),
            value: format!("{} != {}", data.len(), expected_len),
        });
    }
    let mut ordered = vec![None; expected_len];
    for item in data {
        if item.index >= expected_len || ordered[item.index].is_some() {
            return Err(PolarisError::InvalidParameter {
                key: "embedding.index".to_owned(),
                value: item.index.to_string(),
            });
        }
        ordered[item.index] = Some(item.embedding);
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| {
            embedding.ok_or_else(|| PolarisError::InvalidParameter {
                key: "embedding.index".to_owned(),
                value: format!("missing {index}"),
            })
        })
        .collect()
}

fn hnsw_neighbor_indices(
    items: &[GeometryItem],
    source_idx: usize,
    limit: usize,
    m: usize,
    ef_search: usize,
) -> Vec<usize> {
    let vectors = items
        .iter()
        .map(|item| {
            item.embedding
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let ef = ef_search.max(limit + 1).max(m);
    let hnsw = Hnsw::<f32, DistCosine>::new(m, items.len(), 16, ef, DistCosine {});
    for (idx, vector) in vectors.iter().enumerate() {
        hnsw.insert((&vector[..], idx));
    }
    hnsw.search(&vectors[source_idx], limit + 1, ef)
        .into_iter()
        .map(|neighbor| neighbor.d_id)
        .filter(|idx| *idx != source_idx)
        .take(limit)
        .collect()
}

fn geometry_items(conn: &Connection) -> Result<Vec<GeometryItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, q, embedding
         FROM concepts
         WHERE embedding IS NOT NULL
         ORDER BY id ASC",
    )?;
    let mut rows = stmt.query([])?;
    let mut items = Vec::new();
    while let Some(row) = rows.next()? {
        let q_blob: Option<Vec<u8>> = row.get(1)?;
        let embedding_blob: Vec<u8> = row.get(2)?;
        items.push(GeometryItem {
            id: row.get(0)?,
            q: q_blob.and_then(|blob| decode_vector(&blob).ok()),
            embedding: normalize_embedding(&decode_vector(&embedding_blob)?)?,
        });
    }
    Ok(items)
}

fn concepts_missing_embedding(conn: &Connection) -> Result<Vec<EmbeddingConcept>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name
         FROM concepts
         WHERE embedding IS NULL
         ORDER BY seed_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(EmbeddingConcept {
            id: row.get(0)?,
            kind: row.get(1)?,
            name: row.get(2)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn embedding_input(concept: &EmbeddingConcept) -> String {
    format!(
        "kind: {}\nid: {}\nname: {}",
        concept.kind, concept.id, concept.name
    )
}

fn normalize_embedding(values: &[f64]) -> Result<Vec<f64>> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return Err(PolarisError::InvalidParameter {
            key: "embedding".to_owned(),
            value: format!("{} values", values.len()),
        });
    }
    let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm == 0.0 {
        return Err(PolarisError::InvalidParameter {
            key: "embedding".to_owned(),
            value: "zero norm".to_owned(),
        });
    }
    Ok(values.iter().map(|value| value / norm).collect())
}

fn residual_correlation(conn: &Connection, left: &str, right: &str) -> Result<f64> {
    let left_series = residual_series(conn, left)?;
    let right_series = residual_series(conn, right)?;
    Ok(correlation(&left_series, &right_series).unwrap_or(0.0))
}

fn residual_series(conn: &Connection, concept_id: &str) -> Result<BTreeMap<String, f64>> {
    let mut stmt = conn.prepare(
        "SELECT week, mean_resid
         FROM residual_stats
         WHERE concept_id=?1
         ORDER BY week ASC",
    )?;
    let rows = stmt.query_map([concept_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut series = BTreeMap::new();
    for row in rows {
        let (week, mean) = row?;
        series.insert(week, mean);
    }
    Ok(series)
}

fn correlation(left: &BTreeMap<String, f64>, right: &BTreeMap<String, f64>) -> Option<f64> {
    let pairs = left
        .iter()
        .filter_map(|(week, left_value)| {
            right
                .get(week)
                .map(|right_value| (*left_value, *right_value))
        })
        .collect::<Vec<_>>();
    if pairs.len() < MIN_COMMON_WEEKS {
        return None;
    }
    let left_mean = pairs.iter().map(|(left, _)| left).sum::<f64>() / pairs.len() as f64;
    let right_mean = pairs.iter().map(|(_, right)| right).sum::<f64>() / pairs.len() as f64;
    let numerator = pairs
        .iter()
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>();
    let left_var = pairs
        .iter()
        .map(|(left, _)| (left - left_mean).powi(2))
        .sum::<f64>();
    let right_var = pairs
        .iter()
        .map(|(_, right)| (right - right_mean).powi(2))
        .sum::<f64>();
    if left_var == 0.0 || right_var == 0.0 {
        return None;
    }
    Some(numerator / (left_var.sqrt() * right_var.sqrt()))
}

fn cosine(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        return None;
    }
    Some(dot / (left_norm * right_norm))
}

fn existing_embedding_dim(conn: &Connection) -> Result<Option<usize>> {
    conn.query_row(
        "SELECT value FROM meta WHERE key='embedding.dim'",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()?
    .map(|value| {
        value
            .parse::<usize>()
            .map_err(|_| PolarisError::InvalidParameter {
                key: "embedding.dim".to_owned(),
                value,
            })
    })
    .transpose()
    .map(|dimension| dimension.filter(|value| *value > 0))
}

fn concept_count(conn: &Connection) -> Result<usize> {
    conn.query_row("SELECT COUNT(*) FROM concepts", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|count| count.max(0) as usize)
    .map_err(Into::into)
}

fn concept_kind(conn: &Connection, concept_id: &str) -> Result<String> {
    conn.query_row(
        "SELECT kind FROM concepts WHERE id=?1",
        [concept_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| PolarisError::MissingConcept(concept_id.to_owned()))
}

fn ensure_concept_exists(conn: &Connection, concept_id: &str) -> Result<()> {
    concept_kind(conn, concept_id).map(|_| ())
}
