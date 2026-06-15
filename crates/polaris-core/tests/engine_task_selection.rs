mod common;

use common::workspace_pack_path;
use polaris_core::db::migrate;
use polaris_core::engine::{Engine, SubmitInput};
use rusqlite::Connection;

#[test]
fn init_pack_installs_concepts_and_next_returns_first_open_concept() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);

    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    let next = engine.next_task().unwrap().expect("next task");

    assert_eq!(next.concept_id, "ownership");
    let rubric: String = engine
        .conn()
        .query_row(
            "SELECT value FROM meta WHERE key='pack.rust.rubric'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(rubric.contains("Rubric"));
    assert!(next.reason.contains("选它因为"));
}

#[test]
fn integration_seed_flow_prioritizes_high_confidence_low_final_score() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let concepts = [
        "ownership",
        "ownership",
        "ownership",
        "references",
        "references",
        "pattern_matching",
        "pattern_matching",
        "traits",
        "traits",
        "modules",
        "closures",
    ];
    let mut receipts = Vec::new();
    for (idx, concept) in concepts.iter().enumerate() {
        let receipt = engine
            .submit(SubmitInput {
                session_id: "integration".to_owned(),
                concept_id: (*concept).to_owned(),
                task_type: "recall".to_owned(),
                prompt_text: format!("Explain {concept}."),
                response_text: format!("{concept} answer {idx}"),
                self_confidence: if *concept == "ownership" { 5 } else { 3 },
                latency_ms: 1000 + idx as i64,
                hint_count: 0,
            })
            .unwrap();
        receipts.push(((*concept).to_owned(), receipt));
    }

    for (concept, receipt) in &receipts {
        let final_score = if concept == "ownership" { 0.20 } else { 0.82 };
        engine
            .apply_final_score(&receipt.attempt_id, final_score)
            .unwrap();
    }

    let next = engine.next_task().unwrap().expect("next task");
    assert_eq!(next.concept_id, "ownership");

    let state = engine.mastery_state("ownership").unwrap().unwrap();
    assert!(state.calib_gap > 0.25);

    let queued: i64 = engine
        .conn()
        .query_row("SELECT COUNT(*) FROM grade_queue", [], |row| row.get(0))
        .unwrap();
    assert_eq!(queued, 11);

    let due: String = engine
        .conn()
        .query_row(
            "SELECT next_due_at FROM mastery_states WHERE concept_id='ownership'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(due.ends_with("T04:00:00Z"), "due was {due}");

    let due_order: i64 = engine
        .conn()
        .query_row(
            "SELECT julianday((SELECT next_due_at FROM mastery_states WHERE concept_id='ownership')) <=
                    julianday((SELECT next_due_at FROM mastery_states WHERE concept_id='traits'))",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(due_order, 1);
}

#[test]
fn next_task_uses_engine_misconception_window_semantics() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, task_type, self_confidence, provisional_score,
                                  final_score, misconception_id, created_at)
             VALUES ('mis1', 'references', 'recall', 5, 0.20, 0.20, 'm1', strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, fsrs_json, calib_gap, brier_ewma, attempt_count, updated_at)
             VALUES ('references', 0.20, NULL, 0.0, 0.0, 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();

    let next = engine.next_task().unwrap().unwrap();
    assert_eq!(next.concept_id, "references");

    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, task_type, self_confidence, provisional_score,
                                  final_score, created_at)
             VALUES ('pass1', 'references', 'recall', 3, 0.90, 0.90, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [],
        )
        .unwrap();

    let next_after_success = engine.next_task().unwrap().unwrap();
    assert_ne!(next_after_success.concept_id, "references");
}
