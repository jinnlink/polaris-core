use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::report::MirrorReportNarrative;
use rusqlite::Connection;

#[test]
fn p06d_default_report_keeps_narrative_absent() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);

    let report = engine.run_mirror_report().unwrap();

    assert!(report.narrative.is_none());
    assert!(!report.assertions.is_empty());
}

#[test]
fn p06d_static_narrative_accepts_strict_citation_to_report_claim() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);

    let report = engine
        .run_mirror_report_with_static_narrative(
            r#"{
                "text": "本周优先复核 ownership 的幻影掌握风险，先用迁移题验证。",
                "citations": [{
                    "evidence_id": "calibration_phantom:ownership",
                    "quote": "你的自信持续高于实际表现"
                }]
            }"#,
        )
        .unwrap();

    let narrative = report.narrative.expect("valid narrative");
    assert_eq!(
        narrative.text,
        "本周优先复核 ownership 的幻影掌握风险，先用迁移题验证。"
    );
    assert_eq!(
        narrative.citations[0].evidence_id,
        "calibration_phantom:ownership"
    );
    assert!(!narrative.degraded);

    let latest = engine
        .latest_mirror_report()
        .unwrap()
        .expect("stored report");
    assert_eq!(latest.id, report.id);
    assert!(latest.narrative.is_some());
}

#[test]
fn p06d_invalid_narrative_citation_degrades_to_raw_report() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);

    let report = engine
        .run_mirror_report_with_static_narrative(
            r#"{
                "text": "这段叙事不能引用不存在的断言。",
                "citations": [{
                    "evidence_id": "calibration_phantom:missing",
                    "quote": "你的自信持续高于实际表现"
                }]
            }"#,
        )
        .unwrap();

    assert!(report.narrative.is_none());
    assert!(!report.assertions.is_empty());
    assert!(report
        .assertions
        .iter()
        .any(|item| item.id == "calibration_phantom:ownership"));
}

#[test]
fn p06d_original_evidence_quote_is_rejected_even_with_report_item_id() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);
    let raw_quote = "原始作答里的一段可引用文本";
    engine
        .conn()
        .execute(
            "INSERT INTO evidence_items(id, session_id, source, content_type, text, created_at)
             VALUES ('raw-p06d-evidence', 's1', 'user', 'text', ?1,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            [raw_quote],
        )
        .unwrap();

    let response = format!(
        r#"{{
            "text": "这段叙事试图绕过报告 item claim。",
            "citations": [{{
                "evidence_id": "calibration_phantom:ownership",
                "quote": "{raw_quote}"
            }}]
        }}"#
    );

    let report = engine
        .run_mirror_report_with_static_narrative(&response)
        .unwrap();

    assert!(report.narrative.is_none());
    assert!(!report.assertions.is_empty());
}

#[test]
fn p06d_empty_or_malformed_static_narrative_degrades_without_losing_assertions() {
    let engine = seeded_engine();
    seed_phantom_concept(&engine, "ownership", 4);

    let report = engine
        .run_mirror_report_with_static_narrative(r#"{"text":"缺引用","citations":[]}"#)
        .unwrap();

    assert!(report.narrative.is_none());
    assert!(!report.assertions.is_empty());

    let malformed = engine
        .run_mirror_report_with_static_narrative("{not json")
        .unwrap();
    assert!(malformed.narrative.is_none());
    assert!(!malformed.assertions.is_empty());
}

#[test]
fn p06d_narrative_serializes_with_report() {
    let narrative = MirrorReportNarrative {
        text: "本周报告叙事".to_owned(),
        citations: Vec::new(),
        degraded: false,
    };

    let json = serde_json::to_string(&narrative).unwrap();

    assert!(json.contains("本周报告叙事"));
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn seed_phantom_concept(engine: &Engine, concept_id: &str, attempt_count: usize) {
    for idx in 0..attempt_count {
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                      final_score, created_at, graded_at)
                 VALUES (?1, 's1', ?2, 'recall', 5, 0.2,
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'),
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'))",
                (format!("{concept_id}-p06d-phantom-{idx}"), concept_id),
            )
            .unwrap();
    }
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO mastery_states(concept_id, p_known, calib_gap, attempt_count, updated_at)
             VALUES (?1, 0.30, 0.40, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            (concept_id, attempt_count as i64),
        )
        .unwrap();
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
