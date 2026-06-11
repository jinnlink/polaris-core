use polaris_core::db::migrate;
use polaris_core::status::status_snapshot;
use polaris_core::teaching::teaching_instruction;
use rusqlite::Connection;

#[test]
fn teaching_instruction_focuses_failed_prerequisite_gap() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_two_concepts_with_prereq(&conn);
    insert_failed_attempt(&conn, "target", 0.20);
    insert_mastery(&conn, "pre", 0.10);

    let instruction = teaching_instruction(&conn, "target").unwrap();

    assert_eq!(instruction.target, "pre");
    assert_eq!(instruction.move_name, "repair_prerequisite");
    assert_eq!(instruction.focus.kind, "prerequisite_gap");
    assert!(instruction.anchor.contains("target"));
    assert!(instruction.do_text.contains("先让学习者作答"));
    assert!(instruction.dont.contains("不要直接改掌握度"));
}

#[test]
fn status_snapshot_lists_concepts_and_due_count() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_two_concepts_with_prereq(&conn);
    insert_mastery_with_due(&conn, "pre", 0.70, "1970-01-01T00:00:00Z");

    let snapshot = status_snapshot(&conn).unwrap();

    assert_eq!(snapshot.due_today, 1);
    assert_eq!(snapshot.concepts.len(), 2);
    assert_eq!(snapshot.concepts[0].concept_id, "pre");
    assert_eq!(snapshot.concepts[0].phase, "正常");
}

fn seed_two_concepts_with_prereq(conn: &Connection) {
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
         VALUES ('pre', 'test', 'Prerequisite', 'concept', 1, 0.20, 'pack-seed', '[]', '2026-06-11T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
         VALUES ('target', 'test', 'Target', 'concept', 2, 0.20, 'pack-seed', '[]', '2026-06-11T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO edges(id, src, dst, type, weight, provenance, evidence_ids_json, created_at)
         VALUES ('e1', 'pre', 'target', 'prerequisite', 1.0, 'pack-seed', '[]', '2026-06-11T00:00:00Z')",
        [],
    )
    .unwrap();
}

fn insert_failed_attempt(conn: &Connection, concept_id: &str, score: f64) {
    conn.execute(
        "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence, provisional_score, final_score, created_at)
         VALUES ('a1', 's1', ?1, 'recall', 5, ?2, ?2, '2026-06-11T00:00:00Z')",
        (concept_id, score),
    )
    .unwrap();
}

fn insert_mastery(conn: &Connection, concept_id: &str, p_known: f64) {
    insert_mastery_with_due(conn, concept_id, p_known, "2099-01-01T00:00:00Z");
}

fn insert_mastery_with_due(conn: &Connection, concept_id: &str, p_known: f64, next_due_at: &str) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, next_due_at, calib_gap, brier_ewma, attempt_count, updated_at)
         VALUES (?1, ?2, ?3, 0.0, 0.0, 1, '2026-06-11T00:00:00Z')",
        (concept_id, p_known, next_due_at),
    )
    .unwrap();
}
