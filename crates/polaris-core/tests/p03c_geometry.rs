use std::sync::Mutex;

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::geometry::EmbeddingProvider;
use polaris_core::mirt::{decode_vector, encode_vector};
use rusqlite::{params, Connection};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn embedding_refresh_without_env_is_disabled_and_does_not_write() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvRestore::without_embed_env();

    let conn = setup_geometry_graph();
    let engine = Engine::new(conn);

    let summary = engine.refresh_missing_embeddings().unwrap();

    assert!(summary.disabled);
    assert_eq!(summary.refreshed, 0);
    assert_eq!(summary.skipped, concept_count(engine.conn()) as usize);
    assert_eq!(summary.dimension, None);
    assert_eq!(count_embeddings(engine.conn()), 0);
}

#[test]
fn embedding_refresh_normalizes_and_records_dimension() {
    let conn = setup_geometry_graph();
    let engine = Engine::new(conn);
    let provider = StaticEmbeddingProvider {
        vectors: vec![vec![3.0, 4.0]; concept_count(engine.conn()) as usize],
    };

    let summary = engine
        .refresh_missing_embeddings_with_provider(&provider)
        .unwrap();

    assert!(!summary.disabled);
    assert_eq!(summary.refreshed, concept_count(engine.conn()) as usize);
    assert_eq!(summary.dimension, Some(2));

    let blob: Vec<u8> = engine
        .conn()
        .query_row(
            "SELECT embedding FROM concepts WHERE id='schema:drop'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let embedding = decode_vector(&blob).unwrap();
    assert!((embedding[0] - 0.6).abs() < 1e-6);
    assert!((embedding[1] - 0.8).abs() < 1e-6);

    let dim: String = engine
        .conn()
        .query_row(
            "SELECT value FROM meta WHERE key='embedding.dim'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dim, "2");
}

#[test]
fn embedding_refresh_rejects_dimension_mismatch_without_partial_write() {
    let conn = setup_geometry_graph();
    let engine = Engine::new(conn);
    let mut vectors = vec![vec![1.0, 0.0]; concept_count(engine.conn()) as usize];
    vectors[1] = vec![1.0, 0.0, 0.0];
    let provider = StaticEmbeddingProvider { vectors };

    let error = engine
        .refresh_missing_embeddings_with_provider(&provider)
        .unwrap_err();

    assert!(error.to_string().contains("embedding.dim"));
    assert_eq!(count_embeddings(engine.conn()), 0);
    let dim: String = engine
        .conn()
        .query_row(
            "SELECT value FROM meta WHERE key='embedding.dim'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dim, "0");
}

#[test]
fn geometry_candidates_use_hnsw_and_combined_scores() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvRestore::with_embed_env();
    let conn = setup_geometry_graph();
    store_embedding(&conn, "schema:drop", &[1.0, 0.0]);
    store_embedding(&conn, "schema:raii", &[0.8, 0.2]);
    store_embedding(&conn, "schema:unrelated", &[0.0, 1.0]);
    store_q(&conn, "schema:drop", &[1.0, 0.0]);
    store_q(&conn, "schema:raii", &[1.0, 0.0]);
    store_q(&conn, "schema:unrelated", &[0.0, 1.0]);
    seed_matching_residuals(&conn, "schema:drop", "schema:raii");
    let engine = Engine::new(conn);

    let candidates = engine.geometry_candidates("schema:drop", 3).unwrap();

    let raii = candidates
        .iter()
        .find(|candidate| candidate.target == "schema:raii")
        .expect("schema:raii candidate");
    let cos_e = 0.8_f64 / (0.8_f64.powi(2) + 0.2_f64.powi(2)).sqrt();
    assert!((raii.cos_e - cos_e).abs() < 1e-6);
    assert!((raii.cos_q - 1.0).abs() < 1e-9);
    assert!((raii.struct_score - 1.0).abs() < 1e-9);
    assert!((raii.coh - 1.0).abs() < 1e-9);
    assert!((raii.assoc - (0.15 * cos_e + 0.85)).abs() < 1e-6);
    assert!((raii.discover - (0.85 * (1.0 - cos_e))).abs() < 1e-6);
}

#[test]
fn geometry_candidates_keep_semantically_far_discover_candidates() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvRestore::with_embed_env();
    let conn = setup_geometry_graph();
    store_embedding(&conn, "schema:drop", &[1.0, 0.0]);
    store_embedding(&conn, "schema:raii", &[-1.0, 0.0]);
    store_embedding(&conn, "schema:unrelated", &[0.0, 1.0]);
    store_q(&conn, "schema:drop", &[1.0, 0.0]);
    store_q(&conn, "schema:raii", &[1.0, 0.0]);
    seed_matching_residuals(&conn, "schema:drop", "schema:raii");
    let engine = Engine::new(conn);

    let candidates = engine.geometry_candidates("schema:drop", 5).unwrap();

    let raii = candidates
        .iter()
        .find(|candidate| candidate.target == "schema:raii")
        .expect("schema:raii should survive negative cos_E");
    assert!(raii.cos_e < 0.0);
    assert!(raii.discover > 1.0);
}

#[test]
fn geometry_maps_to_candidates_respect_structure_gate() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvRestore::with_embed_env();
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "schema:left", "schema");
    insert_concept(&conn, "schema:right", "schema");
    store_embedding(&conn, "schema:left", &[1.0, 0.0]);
    store_embedding(&conn, "schema:right", &[0.95, 0.05]);
    let mut engine = Engine::new(conn);

    let low = engine
        .upsert_geometry_maps_to_candidates("schema:left", 5)
        .unwrap();
    assert!(low.is_empty());
    assert_eq!(count_maps_to(engine.conn()), 0);

    insert_concept(engine.conn(), "shared", "concept");
    insert_edge(
        engine.conn(),
        "left_shared",
        "schema:left",
        "shared",
        "component_of",
    );
    insert_edge(
        engine.conn(),
        "right_shared",
        "schema:right",
        "shared",
        "component_of",
    );

    let high = engine
        .upsert_geometry_maps_to_candidates("schema:left", 5)
        .unwrap();

    assert_eq!(high.len(), 1);
    assert_eq!(count_maps_to(engine.conn()), 1);
    let alignment_json: String = engine
        .conn()
        .query_row(
            "SELECT alignment_json FROM edges WHERE id='maps_to:schema:left:schema:right'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let alignment: serde_json::Value = serde_json::from_str(&alignment_json).unwrap();
    assert_eq!(
        alignment["requires_llm_verification"],
        serde_json::json!(true)
    );
}

#[test]
fn geometry_maps_to_is_disabled_without_env_even_with_embeddings() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvRestore::without_embed_env();
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "schema:left", "schema");
    insert_concept(&conn, "schema:right", "schema");
    store_embedding(&conn, "schema:left", &[1.0, 0.0]);
    store_embedding(&conn, "schema:right", &[0.95, 0.05]);
    insert_concept(&conn, "shared", "concept");
    insert_edge(
        &conn,
        "left_shared",
        "schema:left",
        "shared",
        "component_of",
    );
    insert_edge(
        &conn,
        "right_shared",
        "schema:right",
        "shared",
        "component_of",
    );
    let mut engine = Engine::new(conn);

    let mappings = engine
        .upsert_geometry_maps_to_candidates("schema:left", 5)
        .unwrap();

    assert!(mappings.is_empty());
    assert_eq!(count_maps_to(engine.conn()), 0);
}

struct StaticEmbeddingProvider {
    vectors: Vec<Vec<f64>>,
}

struct EnvRestore {
    saved: Vec<(&'static str, Option<String>)>,
}

impl EnvRestore {
    fn without_embed_env() -> Self {
        let saved = Self::capture();
        for (key, _) in &saved {
            std::env::remove_var(key);
        }
        Self { saved }
    }

    fn with_embed_env() -> Self {
        let saved = Self::capture();
        std::env::set_var("POLARIS_EMBED_BASE_URL", "http://127.0.0.1:9");
        std::env::set_var("POLARIS_EMBED_MODEL", "test-embedding");
        std::env::set_var("POLARIS_EMBED_API_KEY", "test-key");
        Self { saved }
    }

    fn capture() -> Vec<(&'static str, Option<String>)> {
        let keys = [
            "POLARIS_EMBED_BASE_URL",
            "POLARIS_EMBED_MODEL",
            "POLARIS_EMBED_API_KEY",
        ];
        keys.into_iter()
            .map(|key| (key, std::env::var(key).ok()))
            .collect::<Vec<_>>()
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

impl EmbeddingProvider for StaticEmbeddingProvider {
    fn embed(&self, inputs: &[String]) -> polaris_core::error::Result<Vec<Vec<f64>>> {
        assert_eq!(inputs.len(), self.vectors.len());
        Ok(self.vectors.clone())
    }
}

fn setup_geometry_graph() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    for (id, kind) in [
        ("schema:drop", "schema"),
        ("schema:raii", "schema"),
        ("schema:unrelated", "schema"),
        ("resource", "concept"),
        ("owner", "concept"),
        ("drop", "concept"),
    ] {
        insert_concept(&conn, id, kind);
    }
    for (id, src, dst, edge_type) in [
        ("drop_resource", "schema:drop", "resource", "component_of"),
        ("drop_owner", "schema:drop", "owner", "component_of"),
        ("drop_rule", "owner", "drop", "prerequisite"),
        ("raii_resource", "schema:raii", "resource", "component_of"),
        ("raii_owner", "schema:raii", "owner", "component_of"),
        ("raii_rule", "owner", "drop", "prerequisite"),
    ] {
        insert_edge(&conn, id, src, dst, edge_type);
    }
    conn
}

fn insert_concept(conn: &Connection, id: &str, kind: &str) {
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, q, provenance, evidence_ids_json, created_at)
         VALUES (?1, 'test', ?1, ?2, 1, ?3, 'test', '[]', '2026-06-11T00:00:00Z')",
        params![id, kind, encode_vector(&[1.0, 0.0])],
    )
    .unwrap();
}

fn insert_edge(conn: &Connection, id: &str, src: &str, dst: &str, edge_type: &str) {
    conn.execute(
        "INSERT INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
         VALUES (?1, ?2, ?3, ?4, 1.0, 'test', '[]', '2026-06-11T00:00:00Z')",
        params![id, src, dst, edge_type],
    )
    .unwrap();
}

fn store_embedding(conn: &Connection, concept_id: &str, values: &[f64]) {
    let embedding = unit(values);
    conn.execute(
        "UPDATE concepts SET embedding=?1 WHERE id=?2",
        params![encode_vector(&embedding), concept_id],
    )
    .unwrap();
}

fn store_q(conn: &Connection, concept_id: &str, values: &[f64]) {
    conn.execute(
        "UPDATE concepts SET q=?1 WHERE id=?2",
        params![encode_vector(values), concept_id],
    )
    .unwrap();
}

fn seed_matching_residuals(conn: &Connection, left: &str, right: &str) {
    for (week, value) in [
        ("2026-W01", -0.2),
        ("2026-W02", 0.4),
        ("2026-W03", 0.1),
        ("2026-W04", 0.6),
    ] {
        for concept in [left, right] {
            conn.execute(
                "INSERT INTO residual_stats(concept_id, week, mean_resid, n)
                 VALUES (?1, ?2, ?3, 3)",
                params![concept, week, value],
            )
            .unwrap();
        }
    }
}

fn unit(values: &[f64]) -> Vec<f64> {
    let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
    values.iter().map(|value| value / norm).collect()
}

fn concept_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM concepts", [], |row| row.get(0))
        .unwrap()
}

fn count_maps_to(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM edges WHERE type='maps_to'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

fn count_embeddings(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM concepts WHERE embedding IS NOT NULL",
        [],
        |row| row.get(0),
    )
    .unwrap()
}
