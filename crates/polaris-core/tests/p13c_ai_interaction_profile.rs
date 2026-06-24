use polaris_core::ai_profile::AiInteractionProfileInput;
use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use rusqlite::Connection;

#[test]
fn default_ai_interaction_profile_is_balanced_and_read_only() {
    let engine = test_engine();

    let profile = engine.ai_interaction_profile().unwrap();

    assert_eq!(profile.version, 1);
    assert_eq!(profile.persona, "balanced_mentor");
    assert_eq!(profile.verbosity, "normal");
    assert_eq!(profile.explanation_depth, "key_steps");
    assert_eq!(profile.proactivity, "stuck_only");
    assert_eq!(profile.intervention_frequency, "normal");
    assert_eq!(profile.correction_style, "guided");
    assert_eq!(profile.custom_notes, None);
    assert!(profile.guidance.contains("平衡"));
    assert!(profile.guidance.contains("卡住"));
    assert_eq!(profile_meta_count(engine.conn()), 0);
}

#[test]
fn update_ai_interaction_profile_persists_student_preferences() {
    let engine = test_engine();

    let profile = engine
        .update_ai_interaction_profile(AiInteractionProfileInput {
            persona: Some("socratic_tutor".to_owned()),
            verbosity: Some("detailed".to_owned()),
            explanation_depth: Some("examples_first".to_owned()),
            proactivity: Some("proactive".to_owned()),
            intervention_frequency: Some("high".to_owned()),
            correction_style: Some("supportive".to_owned()),
            custom_notes: Some("先问我一个小问题，再展开解释。".to_owned()),
        })
        .unwrap();

    assert_eq!(profile.persona, "socratic_tutor");
    assert_eq!(profile.verbosity, "detailed");
    assert_eq!(profile.explanation_depth, "examples_first");
    assert_eq!(profile.proactivity, "proactive");
    assert_eq!(profile.intervention_frequency, "high");
    assert_eq!(profile.correction_style, "supportive");
    assert_eq!(
        profile.custom_notes.as_deref(),
        Some("先问我一个小问题，再展开解释。")
    );
    assert!(profile.guidance.contains("苏格拉底"));
    assert!(profile.guidance.contains("主动"));
    assert_eq!(profile_meta_count(engine.conn()), 1);

    let reloaded = engine.ai_interaction_profile().unwrap();
    assert_eq!(reloaded, profile);
}

#[test]
fn update_ai_interaction_profile_rejects_invalid_values_without_mutation() {
    let engine = test_engine();
    let original = engine.ai_interaction_profile().unwrap();

    let error = engine
        .update_ai_interaction_profile(AiInteractionProfileInput {
            persona: Some("chaotic".to_owned()),
            verbosity: None,
            explanation_depth: None,
            proactivity: None,
            intervention_frequency: None,
            correction_style: None,
            custom_notes: None,
        })
        .unwrap_err();

    assert!(error.to_string().contains("ai_profile.persona"));
    assert_eq!(engine.ai_interaction_profile().unwrap(), original);
    assert_eq!(profile_meta_count(engine.conn()), 0);
}

#[test]
fn update_ai_interaction_profile_trims_blank_custom_notes() {
    let engine = test_engine();

    let profile = engine
        .update_ai_interaction_profile(AiInteractionProfileInput {
            persona: None,
            verbosity: Some("brief".to_owned()),
            explanation_depth: None,
            proactivity: None,
            intervention_frequency: None,
            correction_style: None,
            custom_notes: Some("   ".to_owned()),
        })
        .unwrap();

    assert_eq!(profile.verbosity, "brief");
    assert_eq!(profile.custom_notes, None);
    assert!(profile.guidance.contains("简洁"));
}

#[test]
fn update_ai_interaction_profile_rejects_overlong_custom_notes_without_mutation() {
    let engine = test_engine();
    let original = engine.ai_interaction_profile().unwrap();
    let overlong_notes = "太".repeat(2_001);

    let error = engine
        .update_ai_interaction_profile(AiInteractionProfileInput {
            persona: Some("friendly_companion".to_owned()),
            verbosity: None,
            explanation_depth: None,
            proactivity: None,
            intervention_frequency: None,
            correction_style: None,
            custom_notes: Some(overlong_notes),
        })
        .unwrap_err();

    assert!(error.to_string().contains("ai_profile.custom_notes"));
    assert_eq!(engine.ai_interaction_profile().unwrap(), original);
    assert_eq!(profile_meta_count(engine.conn()), 0);
}

fn test_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn profile_meta_count(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM meta WHERE key='ai.interaction_profile'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}
