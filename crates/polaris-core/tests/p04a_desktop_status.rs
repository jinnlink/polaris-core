use std::collections::BTreeMap;

use polaris_core::db::migrate;
use polaris_core::phase::Phase;
use polaris_core::status::status_snapshot;
use rusqlite::Connection;

#[test]
fn status_snapshot_exposes_stable_phase_counts_for_desktop_mirror() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    seed_concept(&conn, "ownership", "Ownership", 1);
    seed_concept(&conn, "borrowing", "Borrowing", 2);
    seed_concept(&conn, "lifetimes", "Lifetimes", 3);
    insert_mastery_phase(&conn, "ownership", 0.35, Phase::Phantom);
    insert_mastery_phase(&conn, "borrowing", 0.82, Phase::Transfer);

    let snapshot = status_snapshot(&conn).unwrap();
    let counts = snapshot
        .phase_counts
        .iter()
        .map(|item| (item.phase.as_str(), item.count))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(snapshot.generated_at.len(), "2026-06-13T00:00:00Z".len());
    assert_eq!(snapshot.phase_counts.len(), Phase::ALL.len());
    assert_eq!(counts.get(Phase::Phantom.as_str()), Some(&1));
    assert_eq!(counts.get(Phase::Transfer.as_str()), Some(&1));
    assert_eq!(counts.get(Phase::Undetermined.as_str()), Some(&1));
    assert_eq!(counts.get(Phase::Generation.as_str()), Some(&0));
    assert_eq!(snapshot.concepts[0].concept_id, "ownership");
    assert_eq!(snapshot.concepts[0].phase, Phase::Phantom.as_str());
    assert_eq!(snapshot.concepts[2].phase, Phase::Undetermined.as_str());
}

fn seed_concept(conn: &Connection, id: &str, name: &str, seed_order: i64) {
    conn.execute(
        "INSERT INTO concepts(id, pack, name, kind, seed_order, p_init, provenance, evidence_ids_json, created_at)
         VALUES (?1, 'test', ?2, 'concept', ?3, 0.20, 'pack-seed', '[]', '2026-06-13T00:00:00Z')",
        (id, name, seed_order),
    )
    .unwrap();
}

fn insert_mastery_phase(conn: &Connection, concept_id: &str, p_known: f64, phase: Phase) {
    conn.execute(
        "INSERT INTO mastery_states(concept_id, p_known, calib_gap, brier_ewma, attempt_count, phase, updated_at)
         VALUES (?1, ?2, 0.0, 0.0, 2, ?3, '2026-06-13T00:00:00Z')",
        (concept_id, p_known, phase.as_str()),
    )
    .unwrap();
}
