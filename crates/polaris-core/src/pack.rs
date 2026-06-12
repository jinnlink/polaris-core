use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use crate::graph::{is_valid_concept_kind, is_valid_edge_type};
use crate::moves::{normalize_move_template, MoveTemplate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackValidationReport {
    pub concept_count: usize,
    pub prerequisite_count: usize,
    pub misconception_count: usize,
}

#[derive(Debug)]
pub struct PackData {
    pub id: String,
    pub concepts: Vec<ConceptToml>,
    pub edges: Vec<EdgeToml>,
    pub misconceptions: Vec<MisconceptionToml>,
    pub rubric: String,
    pub moves: Vec<MoveTemplate>,
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("missing required pack file: {0}")]
    MissingFile(String),
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("edge {edge_id} references missing concept {concept_id}")]
    MissingEdgeReference { edge_id: String, concept_id: String },
    #[error("misconception {misconception_id} references missing concept {concept_id}")]
    MissingConceptReference {
        misconception_id: String,
        concept_id: String,
    },
    #[error("concept {concept_id} has invalid kind {kind}")]
    InvalidConceptKind { concept_id: String, kind: String },
    #[error("edge {edge_id} has invalid type {edge_type}")]
    InvalidEdgeType { edge_id: String, edge_type: String },
    #[error("pack must contain at least one move template")]
    MissingMove,
    #[error("rubric.md is empty")]
    EmptyRubric,
}

#[derive(Debug, Deserialize)]
struct PackToml {
    id: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct ConceptsToml {
    concept: Vec<ConceptToml>,
    #[serde(default)]
    edge: Vec<EdgeToml>,
}

#[derive(Debug, Deserialize)]
pub struct ConceptToml {
    pub id: String,
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub seed_order: i64,
    pub p_init: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct EdgeToml {
    pub id: String,
    pub src: String,
    pub dst: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    pub alignment_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MisconceptionsToml {
    misconception: Vec<MisconceptionToml>,
}

#[derive(Debug, Deserialize)]
pub struct MisconceptionToml {
    pub id: String,
    pub concept_id: String,
    pub title: String,
    pub pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MovesToml {
    #[serde(rename = "move")]
    moves: Vec<MoveToml>,
}

#[derive(Debug, Deserialize)]
struct MoveToml {
    id: String,
    task_type: Option<String>,
    template: String,
}

pub fn validate_pack_path(path: impl AsRef<Path>) -> Result<PackValidationReport, PackError> {
    let path = path.as_ref();
    for file in [
        "pack.toml",
        "concepts.toml",
        "misconceptions.toml",
        "rubric.md",
        "moves.toml",
    ] {
        if !path.join(file).is_file() {
            return Err(PackError::MissingFile(file.to_owned()));
        }
    }

    let pack: PackToml = read_toml(path, "pack.toml")?;
    if pack.id.trim().is_empty() || pack.title.trim().is_empty() {
        return Err(PackError::MissingFile("pack.toml id/title".to_owned()));
    }

    let concepts: ConceptsToml = read_toml(path, "concepts.toml")?;
    let misconceptions: MisconceptionsToml = read_toml(path, "misconceptions.toml")?;
    let moves: MovesToml = read_toml(path, "moves.toml")?;
    let moves = normalize_moves(moves);
    if moves.is_empty()
        || moves.iter().any(|item| {
            item.id.trim().is_empty()
                || item.task_type.trim().is_empty()
                || item.template.trim().is_empty()
        })
    {
        return Err(PackError::MissingMove);
    }

    let rubric =
        std::fs::read_to_string(path.join("rubric.md")).map_err(|source| PackError::Read {
            path: path.join("rubric.md").display().to_string(),
            source,
        })?;
    if rubric.trim().is_empty() {
        return Err(PackError::EmptyRubric);
    }

    let concept_ids: HashSet<&str> = concepts
        .concept
        .iter()
        .map(|concept| concept.id.as_str())
        .collect();

    for concept in &concepts.concept {
        if !is_valid_concept_kind(&concept.kind) {
            return Err(PackError::InvalidConceptKind {
                concept_id: concept.id.clone(),
                kind: concept.kind.clone(),
            });
        }
    }

    for edge in &concepts.edge {
        if !is_valid_edge_type(&edge.edge_type) {
            return Err(PackError::InvalidEdgeType {
                edge_id: edge.id.clone(),
                edge_type: edge.edge_type.clone(),
            });
        }
        if !concept_ids.contains(edge.src.as_str()) {
            return Err(PackError::MissingEdgeReference {
                edge_id: edge.id.clone(),
                concept_id: edge.src.clone(),
            });
        }
        if !concept_ids.contains(edge.dst.as_str()) {
            return Err(PackError::MissingEdgeReference {
                edge_id: edge.id.clone(),
                concept_id: edge.dst.clone(),
            });
        }
    }

    for misconception in &misconceptions.misconception {
        if !concept_ids.contains(misconception.concept_id.as_str()) {
            return Err(PackError::MissingConceptReference {
                misconception_id: misconception.id.clone(),
                concept_id: misconception.concept_id.clone(),
            });
        }
    }

    Ok(PackValidationReport {
        concept_count: concepts.concept.len(),
        prerequisite_count: concepts
            .edge
            .iter()
            .filter(|edge| edge.edge_type == "prerequisite")
            .count(),
        misconception_count: misconceptions.misconception.len(),
    })
}

pub fn load_pack(path: impl AsRef<Path>) -> Result<PackData, PackError> {
    let path = path.as_ref();
    validate_pack_path(path)?;

    let pack: PackToml = read_toml(path, "pack.toml")?;
    let concepts: ConceptsToml = read_toml(path, "concepts.toml")?;
    let misconceptions: MisconceptionsToml = read_toml(path, "misconceptions.toml")?;
    let moves: MovesToml = read_toml(path, "moves.toml")?;
    let rubric = read_text(path, "rubric.md")?;

    Ok(PackData {
        id: pack.id,
        concepts: concepts.concept,
        edges: concepts.edge,
        misconceptions: misconceptions.misconception,
        rubric,
        moves: normalize_moves(moves),
    })
}

fn normalize_moves(moves: MovesToml) -> Vec<MoveTemplate> {
    moves
        .moves
        .into_iter()
        .map(|item| normalize_move_template(item.id, item.task_type, item.template))
        .collect()
}

fn read_toml<T: for<'de> Deserialize<'de>>(root: &Path, file_name: &str) -> Result<T, PackError> {
    let path = root.join(file_name);
    let source = std::fs::read_to_string(&path).map_err(|source| PackError::Read {
        path: path.display().to_string(),
        source,
    })?;
    toml::from_str(&source).map_err(|source| PackError::Parse {
        path: path.display().to_string(),
        source,
    })
}

fn read_text(root: &Path, file_name: &str) -> Result<String, PackError> {
    let path = root.join(file_name);
    std::fs::read_to_string(&path).map_err(|source| PackError::Read {
        path: path.display().to_string(),
        source,
    })
}

fn default_kind() -> String {
    "concept".to_owned()
}

fn default_weight() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn validates_builtin_rust_pack_shape() {
        let report = validate_pack_path(workspace_pack_path("packs/rust")).unwrap();
        assert!(report.concept_count >= 24);
        assert!(report.prerequisite_count >= 15);
        assert!(report.misconception_count >= 10);
    }

    #[test]
    fn rejects_misconception_with_missing_concept_reference() {
        let root = temp_pack_dir("missing-reference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("pack.toml"), "id = \"bad\"\ntitle = \"Bad\"\n").unwrap();
        fs::write(
            root.join("concepts.toml"),
            r#"
[[concept]]
id = "ownership"
name = "Ownership"
seed_order = 1
"#,
        )
        .unwrap();
        fs::write(
            root.join("misconceptions.toml"),
            r#"
[[misconception]]
id = "bad"
concept_id = "missing"
title = "Bad reference"
"#,
        )
        .unwrap();
        fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
        fs::write(
            root.join("moves.toml"),
            "[[move]]\nid = \"recall\"\ntemplate = \"Explain {concept}.\"\n",
        )
        .unwrap();

        let result = validate_pack_path(&root);

        let _ = fs::remove_dir_all(root);
        assert!(matches!(
            result,
            Err(PackError::MissingConceptReference { .. })
        ));
    }

    #[test]
    fn rejects_unknown_concept_kind() {
        let root = temp_pack_dir("unknown-kind");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("pack.toml"), "id = \"bad\"\ntitle = \"Bad\"\n").unwrap();
        fs::write(
            root.join("concepts.toml"),
            r#"
[[concept]]
id = "schema1"
name = "Schema 1"
kind = "ontology"
seed_order = 1
"#,
        )
        .unwrap();
        fs::write(root.join("misconceptions.toml"), "misconception = []\n").unwrap();
        fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
        fs::write(
            root.join("moves.toml"),
            "[[move]]\nid = \"recall\"\ntemplate = \"Explain {concept}.\"\n",
        )
        .unwrap();

        let result = validate_pack_path(&root);

        let _ = fs::remove_dir_all(root);
        assert!(matches!(result, Err(PackError::InvalidConceptKind { .. })));
    }

    #[test]
    fn rejects_unknown_edge_type() {
        let root = temp_pack_dir("unknown-edge-type");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("pack.toml"), "id = \"bad\"\ntitle = \"Bad\"\n").unwrap();
        fs::write(
            root.join("concepts.toml"),
            r#"
[[concept]]
id = "a"
name = "A"
seed_order = 1

[[concept]]
id = "b"
name = "B"
seed_order = 2

[[edge]]
id = "bad_edge"
src = "a"
dst = "b"
type = "causes"
"#,
        )
        .unwrap();
        fs::write(root.join("misconceptions.toml"), "misconception = []\n").unwrap();
        fs::write(root.join("rubric.md"), "# Rubric\n").unwrap();
        fs::write(
            root.join("moves.toml"),
            "[[move]]\nid = \"recall\"\ntemplate = \"Explain {concept}.\"\n",
        )
        .unwrap();

        let result = validate_pack_path(&root);

        let _ = fs::remove_dir_all(root);
        assert!(matches!(result, Err(PackError::InvalidEdgeType { .. })));
    }

    fn temp_pack_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "polaris-core-pack-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn workspace_pack_path(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(path)
    }
}
