use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::{params, Connection};

#[test]
fn structural_mapping_scores_typed_two_hop_overlap() {
    let conn = setup_graph();
    let engine = Engine::new(conn);

    let mapping = engine
        .structural_mapping_score("schema:drop", "schema:raii")
        .unwrap();

    assert_eq!(mapping.left, "schema:drop");
    assert_eq!(mapping.right, "schema:raii");
    assert_eq!(mapping.matched_edges, 3);
    assert_eq!(mapping.total_edges, 3);
    assert!((mapping.score - 1.0).abs() < 1e-9);
}

#[test]
fn structural_mapping_requires_typed_edge_match() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "schema:left", "schema");
    insert_concept(&conn, "schema:right", "schema");
    insert_concept(&conn, "shared", "concept");
    insert_edge(
        &conn,
        "left_component",
        "schema:left",
        "shared",
        "component_of",
    );
    insert_edge(
        &conn,
        "right_confusion",
        "schema:right",
        "shared",
        "confusion",
    );
    let engine = Engine::new(conn);

    let mapping = engine
        .structural_mapping_score("schema:left", "schema:right")
        .unwrap();

    assert_eq!(mapping.matched_edges, 0);
    assert_eq!(mapping.total_edges, 1);
    assert_eq!(mapping.score, 0.0);
}

#[test]
fn maps_to_candidate_is_written_only_after_threshold() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "schema:left", "schema");
    insert_concept(&conn, "schema:right", "schema");
    insert_concept(&conn, "left_only", "concept");
    insert_concept(&conn, "right_only", "concept");
    insert_edge(
        &conn,
        "left_component",
        "schema:left",
        "left_only",
        "component_of",
    );
    insert_edge(
        &conn,
        "right_component",
        "schema:right",
        "right_only",
        "component_of",
    );
    let mut engine = Engine::new(conn);

    let low = engine
        .upsert_maps_to_candidate("schema:left", "schema:right")
        .unwrap();
    assert!(low.is_none());
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
        .upsert_maps_to_candidate("schema:left", "schema:right")
        .unwrap()
        .expect("candidate");

    assert!((high.score - 0.5).abs() < 1e-9);
    assert_eq!(count_maps_to(engine.conn()), 1);

    let stored_edge: (String, String, String, f64, String) = engine
        .conn()
        .query_row(
            "SELECT src, dst, type, weight, provenance FROM edges WHERE id='maps_to:schema:left:schema:right'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        stored_edge,
        (
            "schema:left".to_owned(),
            "schema:right".to_owned(),
            "maps_to".to_owned(),
            0.5,
            "engine".to_owned()
        )
    );

    let alignment_json: String = engine
        .conn()
        .query_row(
            "SELECT alignment_json FROM edges WHERE id='maps_to:schema:left:schema:right'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let alignment: serde_json::Value = serde_json::from_str(&alignment_json).unwrap();
    assert_eq!(alignment["score"], serde_json::json!(0.5));
    assert_eq!(alignment["matched_edges"], serde_json::json!(1));
    assert_eq!(alignment["total_edges"], serde_json::json!(2));
    assert_eq!(
        alignment["requires_llm_verification"],
        serde_json::json!(true)
    );
}

fn setup_graph() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    for (id, kind) in [
        ("schema:drop", "schema"),
        ("schema:raii", "schema"),
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
        "INSERT INTO concepts(id, pack, name, kind, seed_order, provenance, evidence_ids_json, created_at)
         VALUES (?1, 'test', ?1, ?2, 1, 'test', '[]', '2026-06-11T00:00:00Z')",
        params![id, kind],
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

fn count_maps_to(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM edges WHERE type='maps_to'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}
