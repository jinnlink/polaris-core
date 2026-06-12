use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::config::meta_f64;
use crate::error::{PolarisError, Result};

pub const CONCEPT_KIND_CONCEPT: &str = "concept";
pub const CONCEPT_KIND_SCHEMA: &str = "schema";
pub const CONCEPT_KIND_MISCONCEPTION_INDUCED: &str = "misconception_induced";

pub const EDGE_PREREQUISITE: &str = "prerequisite";
pub const EDGE_CONFUSION: &str = "confusion";
pub const EDGE_COMPONENT_OF: &str = "component_of";
pub const EDGE_INSTANTIATES: &str = "instantiates";
pub const EDGE_MAPS_TO: &str = "maps_to";

pub fn is_valid_concept_kind(kind: &str) -> bool {
    matches!(
        kind,
        CONCEPT_KIND_CONCEPT | CONCEPT_KIND_SCHEMA | CONCEPT_KIND_MISCONCEPTION_INDUCED
    )
}

pub fn is_valid_edge_type(edge_type: &str) -> bool {
    matches!(
        edge_type,
        EDGE_PREREQUISITE | EDGE_CONFUSION | EDGE_COMPONENT_OF | EDGE_INSTANTIATES | EDGE_MAPS_TO
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuralMapping {
    pub left: String,
    pub right: String,
    pub score: f64,
    pub matched_edges: usize,
    pub total_edges: usize,
    pub node_matches: Vec<NodeMatch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeMatch {
    pub left: String,
    pub right: String,
    pub score: f64,
}

#[derive(Debug, Clone)]
struct GraphNode {
    id: String,
    kind: String,
    embedding: Option<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TypedEdge {
    src: String,
    dst: String,
    edge_type: String,
}

#[derive(Debug)]
struct GraphSnapshot {
    nodes: BTreeMap<String, GraphNode>,
    edges: Vec<TypedEdge>,
    adjacency: HashMap<String, Vec<usize>>,
}

#[derive(Debug)]
struct Neighborhood {
    root: String,
    nodes: BTreeMap<String, GraphNode>,
    edges: Vec<TypedEdge>,
}

#[derive(Debug)]
struct PairCandidate {
    left: String,
    right: String,
    score: f64,
}

impl StructuralMapping {
    fn alignment_json(&self) -> Result<String> {
        #[derive(Serialize)]
        struct Alignment<'a> {
            method: &'a str,
            left: &'a str,
            right: &'a str,
            score: f64,
            matched_edges: usize,
            total_edges: usize,
            requires_llm_verification: bool,
            node_matches: Vec<NodeMatchJson<'a>>,
        }

        #[derive(Serialize)]
        struct NodeMatchJson<'a> {
            left: &'a str,
            right: &'a str,
            score: f64,
        }

        let node_matches = self
            .node_matches
            .iter()
            .map(|item| NodeMatchJson {
                left: &item.left,
                right: &item.right,
                score: item.score,
            })
            .collect();

        serde_json::to_string(&Alignment {
            method: "typed_2hop_struct",
            left: &self.left,
            right: &self.right,
            score: self.score,
            matched_edges: self.matched_edges,
            total_edges: self.total_edges,
            requires_llm_verification: true,
            node_matches,
        })
        .map_err(Into::into)
    }
}

pub fn structural_mapping_score(
    conn: &Connection,
    left: &str,
    right: &str,
) -> Result<StructuralMapping> {
    let graph = GraphSnapshot::load(conn)?;
    let left_neighborhood = graph.two_hop_neighborhood(left)?;
    let right_neighborhood = graph.two_hop_neighborhood(right)?;
    Ok(score_neighborhoods(left_neighborhood, right_neighborhood))
}

pub fn upsert_maps_to_candidate(
    conn: &Connection,
    left: &str,
    right: &str,
) -> Result<Option<StructuralMapping>> {
    ensure_schema(conn, left)?;
    ensure_schema(conn, right)?;

    let mapping = structural_mapping_score(conn, left, right)?;
    let threshold = meta_f64(conn, "graph.struct_threshold")?;
    if mapping.score < threshold {
        return Ok(None);
    }

    let edge_id = maps_to_edge_id(left, right);
    let alignment_json = mapping.alignment_json()?;
    conn.execute(
        "INSERT INTO edges(id, src, dst, type, weight, alignment_json, provenance, evidence_ids_json, created_at)
         VALUES (?1, ?2, ?3, 'maps_to', ?4, ?5, 'engine', '[]', strftime('%Y-%m-%dT%H:%M:%SZ','now'))
         ON CONFLICT(id) DO UPDATE SET
            weight=excluded.weight,
            alignment_json=excluded.alignment_json,
            provenance=excluded.provenance,
            evidence_ids_json=excluded.evidence_ids_json,
            created_at=excluded.created_at",
        params![edge_id, left, right, mapping.score, alignment_json],
    )?;

    Ok(Some(mapping))
}

fn ensure_schema(conn: &Connection, id: &str) -> Result<()> {
    let kind = conn
        .query_row("SELECT kind FROM concepts WHERE id=?1", [id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .ok_or_else(|| PolarisError::MissingConcept(id.to_owned()))?;

    if kind == CONCEPT_KIND_SCHEMA {
        Ok(())
    } else {
        Err(PolarisError::InvalidGraphNode {
            id: id.to_owned(),
            expected: CONCEPT_KIND_SCHEMA.to_owned(),
        })
    }
}

fn maps_to_edge_id(left: &str, right: &str) -> String {
    format!("maps_to:{left}:{right}")
}

impl GraphSnapshot {
    fn load(conn: &Connection) -> Result<Self> {
        let mut node_stmt = conn.prepare("SELECT id, kind, embedding FROM concepts")?;
        let node_rows = node_stmt.query_map([], |row| {
            let embedding_blob: Option<Vec<u8>> = row.get(2)?;
            Ok(GraphNode {
                id: row.get(0)?,
                kind: row.get(1)?,
                embedding: embedding_blob.and_then(|blob| parse_embedding(&blob)),
            })
        })?;
        let mut nodes = BTreeMap::new();
        for node in node_rows {
            let node = node?;
            nodes.insert(node.id.clone(), node);
        }

        let mut edge_stmt =
            conn.prepare("SELECT src, dst, type FROM edges WHERE type != 'maps_to'")?;
        let edge_rows = edge_stmt.query_map([], |row| {
            Ok(TypedEdge {
                src: row.get(0)?,
                dst: row.get(1)?,
                edge_type: row.get(2)?,
            })
        })?;
        let mut edges = Vec::new();
        for edge in edge_rows {
            edges.push(edge?);
        }

        let mut adjacency: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, edge) in edges.iter().enumerate() {
            adjacency.entry(edge.src.clone()).or_default().push(idx);
        }

        Ok(Self {
            nodes,
            edges,
            adjacency,
        })
    }

    fn two_hop_neighborhood(&self, root: &str) -> Result<Neighborhood> {
        if !self.nodes.contains_key(root) {
            return Err(PolarisError::MissingConcept(root.to_owned()));
        }

        let mut distances = BTreeMap::new();
        distances.insert(root.to_owned(), 0usize);
        let mut queue = VecDeque::from([(root.to_owned(), 0usize)]);
        while let Some((node_id, distance)) = queue.pop_front() {
            if distance >= 2 {
                continue;
            }
            for edge_idx in self.adjacency.get(&node_id).into_iter().flatten() {
                let edge = &self.edges[*edge_idx];
                let other = edge.dst.as_str();
                if distances.contains_key(other) {
                    continue;
                }
                let next_distance = distance + 1;
                distances.insert(other.to_owned(), next_distance);
                queue.push_back((other.to_owned(), next_distance));
            }
        }

        let node_ids: BTreeSet<String> = distances.keys().cloned().collect();
        let nodes = node_ids
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|node| (id.clone(), node.clone())))
            .collect();
        let edges = self
            .edges
            .iter()
            .filter(|edge| node_ids.contains(&edge.src) && node_ids.contains(&edge.dst))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        Ok(Neighborhood {
            root: root.to_owned(),
            nodes,
            edges,
        })
    }
}

fn score_neighborhoods(left: Neighborhood, right: Neighborhood) -> StructuralMapping {
    let node_matches = match_nodes(&left, &right);
    let node_map: HashMap<&str, &str> = node_matches
        .iter()
        .map(|item| (item.left.as_str(), item.right.as_str()))
        .collect();

    let mut right_edge_counts = BTreeMap::<(String, String, String), usize>::new();
    for edge in &right.edges {
        *right_edge_counts
            .entry((edge.src.clone(), edge.dst.clone(), edge.edge_type.clone()))
            .or_default() += 1;
    }

    let mut matched_edges = 0;
    for edge in &left.edges {
        let (Some(mapped_src), Some(mapped_dst)) = (
            node_map.get(edge.src.as_str()),
            node_map.get(edge.dst.as_str()),
        ) else {
            continue;
        };
        let key = (
            (*mapped_src).to_owned(),
            (*mapped_dst).to_owned(),
            edge.edge_type.clone(),
        );
        if let Some(count) = right_edge_counts.get_mut(&key) {
            if *count > 0 {
                matched_edges += 1;
                *count -= 1;
            }
        }
    }

    let total_edges = left.edges.len().max(right.edges.len());
    let score = if total_edges == 0 {
        0.0
    } else {
        matched_edges as f64 / total_edges as f64
    };

    StructuralMapping {
        left: left.root,
        right: right.root,
        score,
        matched_edges,
        total_edges,
        node_matches,
    }
}

fn match_nodes(left: &Neighborhood, right: &Neighborhood) -> Vec<NodeMatch> {
    let mut matches = vec![NodeMatch {
        left: left.root.clone(),
        right: right.root.clone(),
        score: 1.0,
    }];
    let mut used_left = HashSet::from([left.root.clone()]);
    let mut used_right = HashSet::from([right.root.clone()]);

    let mut candidates = Vec::new();
    for left_node in left.nodes.values() {
        if left_node.id == left.root {
            continue;
        }
        for right_node in right.nodes.values() {
            if right_node.id == right.root {
                continue;
            }
            if let Some(score) = node_similarity(left_node, right_node) {
                candidates.push(PairCandidate {
                    left: left_node.id.clone(),
                    right: right_node.id.clone(),
                    score,
                });
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.left.cmp(&b.left))
            .then_with(|| a.right.cmp(&b.right))
    });

    for candidate in candidates {
        if used_left.contains(&candidate.left) || used_right.contains(&candidate.right) {
            continue;
        }
        used_left.insert(candidate.left.clone());
        used_right.insert(candidate.right.clone());
        matches.push(NodeMatch {
            left: candidate.left,
            right: candidate.right,
            score: candidate.score,
        });
    }

    matches
}

fn node_similarity(left: &GraphNode, right: &GraphNode) -> Option<f64> {
    if left.id == right.id {
        return Some(1.0);
    }
    if left.kind != right.kind {
        return None;
    }
    let similarity = cosine(left.embedding.as_ref()?, right.embedding.as_ref()?)?;
    (similarity > 0.0).then_some(similarity)
}

fn parse_embedding(blob: &[u8]) -> Option<Vec<f64>> {
    if blob.is_empty() || !blob.len().is_multiple_of(4) {
        return None;
    }
    Some(
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f64)
            .collect(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_concept_kinds_and_edge_types() {
        assert!(is_valid_concept_kind("concept"));
        assert!(is_valid_concept_kind("schema"));
        assert!(is_valid_concept_kind("misconception_induced"));
        assert!(!is_valid_concept_kind("ontology"));

        assert!(is_valid_edge_type("prerequisite"));
        assert!(is_valid_edge_type("confusion"));
        assert!(is_valid_edge_type("component_of"));
        assert!(is_valid_edge_type("instantiates"));
        assert!(is_valid_edge_type("maps_to"));
        assert!(!is_valid_edge_type("causes"));
    }
}
