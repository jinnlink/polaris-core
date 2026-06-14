use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::mirt::{decode_vector, encode_vector};
use rusqlite::Connection;

#[test]
fn fallback_q_uses_distinct_pack_latent_dimensions() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
        .init_pack(workspace_pack_path("packs/algorithms"))
        .unwrap();

    let dims = latent_dims(&engine);
    assert_eq!(dims[0], "pack:rust");
    assert_eq!(dims[1], "pack:algorithms");

    let rust_q = concept_q(&engine, "ownership");
    let algorithms_q = concept_q(&engine, "complexity_basics");
    assert_eq!(rust_q.len(), 32);
    assert_eq!(algorithms_q.len(), 32);
    assert_eq!(rust_q[0], 1.0);
    assert_eq!(rust_q[1], 0.0);
    assert_eq!(algorithms_q[0], 0.0);
    assert_eq!(algorithms_q[1], 1.0);

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    assert_eq!(latent_dims(&engine), dims);
    assert_eq!(concept_q(&engine, "ownership"), rust_q);
}

#[test]
fn fallback_q_preserves_existing_concept_q_when_reinitializing_pack() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    let mut custom_q = vec![0.0; 32];
    custom_q[7] = 1.0;
    engine
        .conn()
        .execute(
            "UPDATE concepts SET q=?1 WHERE id='ownership'",
            [encode_vector(&custom_q)],
        )
        .unwrap();

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    assert_eq!(concept_q(&engine, "ownership"), custom_q);
}

#[test]
fn fallback_q_rejects_new_pack_when_latent_dimensions_are_full() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute("UPDATE meta SET value='1' WHERE key='latent.k'", [])
        .unwrap();
    let mut engine = Engine::new(conn);

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    let result = engine.init_pack(workspace_pack_path("packs/algorithms"));

    assert!(
        result.is_err(),
        "second pack must not silently reuse an occupied latent dimension"
    );
    assert_eq!(latent_dims(&engine), vec!["pack:rust".to_owned()]);
    let algorithms_concepts: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE pack='algorithms'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(algorithms_concepts, 0);
}

fn latent_dims(engine: &Engine) -> Vec<String> {
    let json: String = engine
        .conn()
        .query_row(
            "SELECT value FROM meta WHERE key='latent.dims'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

fn concept_q(engine: &Engine, concept_id: &str) -> Vec<f64> {
    let q_blob: Vec<u8> = engine
        .conn()
        .query_row("SELECT q FROM concepts WHERE id=?1", [concept_id], |row| {
            row.get(0)
        })
        .unwrap();
    decode_vector(&q_blob).unwrap()
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
