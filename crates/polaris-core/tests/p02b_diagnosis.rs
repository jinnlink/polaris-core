use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::{params, Connection};

#[test]
fn failed_target_with_unmet_prerequisite_focuses_prerequisite() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "borrowing", "Borrowing");
    insert_concept(&conn, "lifetimes", "Lifetimes");
    insert_edge(
        &conn,
        "e_borrowing_lifetimes",
        "borrowing",
        "lifetimes",
        "prerequisite",
        1.0,
    );
    insert_mastery(&conn, "borrowing", 0.20);
    insert_attempt(&conn, "a1", "lifetimes", 0.20, "2026-06-11T00:00:00Z");
    let engine = Engine::new(conn);

    let diagnosis = engine.diagnose_concept("lifetimes").unwrap();

    assert!(diagnosis.latest_failed);
    let focus = diagnosis.focus.expect("focus");
    assert_eq!(focus.kind, "prerequisite_gap");
    assert_eq!(focus.concept_id, "borrowing");
    assert_eq!(focus.reason, "latest_failure_with_unmet_prerequisite");
    assert_eq!(diagnosis.unmet_prerequisites.len(), 1);
    assert_eq!(diagnosis.unmet_prerequisites[0].concept_id, "borrowing");
}

#[test]
fn non_failed_latest_attempt_does_not_propagate_prerequisite() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "borrowing", "Borrowing");
    insert_concept(&conn, "lifetimes", "Lifetimes");
    insert_edge(
        &conn,
        "e_borrowing_lifetimes",
        "borrowing",
        "lifetimes",
        "prerequisite",
        1.0,
    );
    insert_mastery(&conn, "borrowing", 0.20);
    insert_attempt(&conn, "a1", "lifetimes", 0.70, "2026-06-11T00:00:00Z");
    let engine = Engine::new(conn);

    let diagnosis = engine.diagnose_concept("lifetimes").unwrap();

    assert!(!diagnosis.latest_failed);
    assert!(diagnosis.focus.is_none());
    assert_eq!(diagnosis.unmet_prerequisites.len(), 1);
}

#[test]
fn unmet_prerequisites_are_sorted_deterministically() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    for (id, name) in [
        ("target", "Target"),
        ("low_b", "Low B"),
        ("low_a", "Low A"),
        ("higher", "Higher"),
    ] {
        insert_concept(&conn, id, name);
    }
    insert_edge(&conn, "e_low_b", "low_b", "target", "prerequisite", 2.0);
    insert_edge(&conn, "e_low_a", "low_a", "target", "prerequisite", 1.0);
    insert_edge(&conn, "e_higher", "higher", "target", "prerequisite", 3.0);
    insert_mastery(&conn, "low_b", 0.20);
    insert_mastery(&conn, "low_a", 0.20);
    insert_mastery(&conn, "higher", 0.50);
    insert_attempt(&conn, "a1", "target", 0.10, "2026-06-11T00:00:00Z");
    let engine = Engine::new(conn);

    let diagnosis = engine.diagnose_concept("target").unwrap();

    assert_eq!(
        diagnosis
            .unmet_prerequisites
            .iter()
            .map(|item| item.concept_id.as_str())
            .collect::<Vec<_>>(),
        vec!["low_b", "low_a", "higher"]
    );
    assert_eq!(
        diagnosis.focus.expect("focus").concept_id,
        "low_b",
        "same p_known should prefer higher edge weight before id"
    );
}

#[test]
fn confusion_edge_generates_discrimination_task() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    insert_concept(&conn, "traits", "Traits");
    insert_concept(&conn, "inheritance", "Inheritance");
    insert_edge(
        &conn,
        "e_traits_inheritance",
        "traits",
        "inheritance",
        "confusion",
        1.0,
    );
    insert_attempt(&conn, "a1", "traits", 0.20, "2026-06-11T00:00:00Z");
    let engine = Engine::new(conn);

    let diagnosis = engine.diagnose_concept("traits").unwrap();

    assert_eq!(diagnosis.confusion_tasks.len(), 1);
    let task = &diagnosis.confusion_tasks[0];
    assert_eq!(task.task_type, "discriminate");
    assert_eq!(task.concept_id, "traits");
    assert_eq!(task.contrast_concept_id, "inheritance");
    assert!(task.prompt.contains("Traits"));
    assert!(task.prompt.contains("Inheritance"));
    assert!(task.prompt.contains("boundary"));
}

fn insert_concept(conn: &Connection, id: &str, name: &str) {
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, provenance, evidence_ids_json, created_at)
         VALUES (?1, 'test', ?2, 'concept', 1, 'test', '[]', '2026-06-11T00:00:00Z')",
        params![id, name],
    )
    .unwrap();
}

fn insert_edge(conn: &Connection, id: &str, src: &str, dst: &str, edge_type: &str, weight: f64) {
    conn.execute(
        "INSERT INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'test', '[]', '2026-06-11T00:00:00Z')",
        params![id, src, dst, edge_type, weight],
    )
    .unwrap();
}

fn insert_mastery(conn: &Connection, concept_id: &str, p_known: f64) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, fsrs_json, calib_gap, brier_ewma, attempt_count, updated_at)
         VALUES (?1, ?2, NULL, 0.0, 0.0, 1, '2026-06-11T00:00:00Z')",
        params![concept_id, p_known],
    )
    .unwrap();
}

fn insert_attempt(
    conn: &Connection,
    id: &str,
    concept_id: &str,
    final_score: f64,
    created_at: &str,
) {
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence, provisional_score, final_score, created_at)
         VALUES (?1, 's1', ?2, 'recall', 3, ?3, ?3, ?4)",
        params![id, concept_id, final_score, created_at],
    )
    .unwrap();
}
