use std::path::{Path, PathBuf};

use polaris_core::db::{migrate, CURRENT_SCHEMA_VERSION};
use polaris_core::engine::Engine;
use polaris_core::goals::{GoalDimensionInput, GoalInput, GoalMilestoneInput, GoalScope};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn schema_v9_adds_generic_goal_scope_without_losing_legacy_goals() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    assert_eq!(CURRENT_SCHEMA_VERSION, 10);
    assert!(columns(&conn, "goals").contains(&"scope_json".to_owned()));

    conn.execute(
        "INSERT INTO goals(id, title, description, created_at, updated_at)
         VALUES ('legacy', 'Legacy goal', NULL, '2026-01-01', '2026-01-01')",
        [],
    )
    .unwrap();
    let goal = Engine::new(conn).goal_snapshot("legacy").unwrap().unwrap();
    assert!(goal.scope.is_empty());
}

#[test]
fn schema_v9_failure_rolls_back_scope_column_and_version() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE goals(
             id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
             created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
             status TEXT NOT NULL DEFAULT 'active', deadline TEXT, pace TEXT,
             priority INTEGER NOT NULL DEFAULT 50, parent_goal_id TEXT,
             completion_summary TEXT
         );
         CREATE TABLE schema_migrations(
             version INTEGER PRIMARY KEY,
             name TEXT NOT NULL, applied_at TEXT NOT NULL
         );
         CREATE TRIGGER reject_goal_scope_migration
         BEFORE INSERT ON schema_migrations WHEN NEW.version=9
         BEGIN SELECT RAISE(ABORT, 'blocked v9 migration'); END;
         PRAGMA user_version=8;",
    )
    .unwrap();

    let error = migrate(&conn).unwrap_err().to_string();

    assert!(error.contains("blocked v9 migration"), "{error}");
    assert!(!columns(&conn, "goals").contains(&"scope_json".to_owned()));
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 8);
}

#[test]
fn stable_crud_preserves_weights_and_archive_is_distinct_from_delete() {
    let engine = template_engine();
    let mut input = scoped_goal(
        "goal-crud",
        GoalScope {
            pack_ids: vec!["template".to_owned()],
            ..GoalScope::default()
        },
    );
    input.dimensions = vec![
        GoalDimensionInput::new("mastered", "mastered", "Mastered", "count", 3.0).with_weight(2.0),
        GoalDimensionInput::new("mastery", "mastery", "Mastery percent", "score", 80.0),
    ];
    let created = engine.create_goal(input.clone()).unwrap();
    assert_eq!(created.scope.pack_ids, ["template"]);
    assert_eq!(created.dimensions[0].weight, 2.0);
    assert_eq!(engine.list_goals(Some("active")).unwrap().len(), 1);

    input.title = "Updated goal".to_owned();
    input.dimensions[0].weight = 4.0;
    let updated = engine.update_goal(input).unwrap();
    assert_eq!(updated.title, "Updated goal");
    assert_eq!(updated.dimensions[0].weight, 4.0);

    let archived = engine.archive_goal("goal-crud").unwrap();
    assert_eq!(archived.status, "archived");
    assert!(engine.list_goals(Some("active")).unwrap().is_empty());
    assert_eq!(engine.list_goals(Some("archived")).unwrap().len(), 1);
    engine.delete_goal("goal-crud").unwrap();
    assert!(engine.goal_snapshot("goal-crud").unwrap().is_none());
}

#[test]
fn progress_refresh_uses_mastery_evidence_and_reaches_weighted_milestones() {
    let engine = template_engine();
    let mut input = scoped_goal(
        "progress",
        GoalScope {
            concept_ids: vec![
                "template_core_terms".to_owned(),
                "template_boundary_cases".to_owned(),
            ],
            ..GoalScope::default()
        },
    );
    input.dimensions = vec![
        GoalDimensionInput::new("d1", "mastered", "Mastered", "count", 2.0).with_weight(3.0),
        GoalDimensionInput::new("d2", "attempts", "Attempts", "graded_attempts", 4.0),
    ];
    input.milestones = vec![GoalMilestoneInput::new(
        "m1",
        "First mastered concept",
        "dimension_threshold",
        json!({"dimension_key": "mastered", "operator": ">=", "value": 1.0}),
        1,
    )];
    engine.create_goal(input).unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO mastery_states(concept_id, p_known, phase, updated_at)
             VALUES ('template_core_terms', 0.90, 'solidification', '2026-08-09'),
                    ('template_boundary_cases', 0.50, 'fluctuation', '2026-08-09')",
            [],
        )
        .unwrap();
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, concept_id, final_score, created_at)
             VALUES ('a1', 'template_core_terms', 0.9, '2026-08-09'),
                    ('a2', 'template_boundary_cases', 0.5, '2026-08-09')",
            [],
        )
        .unwrap();
    let before = mastery_signature(&engine);

    let workspace = engine.goal_workspace(Some("progress")).unwrap();
    assert_eq!(
        workspace.progress.as_ref().unwrap().dimensions[0].current_value,
        1.0
    );
    assert_eq!(
        engine
            .goal_snapshot("progress")
            .unwrap()
            .unwrap()
            .dimensions[0]
            .current_value,
        0.0,
        "GET-style workspace derivation must not persist progress"
    );

    let progress = engine.refresh_goal_progress("progress").unwrap();
    assert_eq!(progress.dimensions[0].current_value, 1.0);
    assert_eq!(progress.dimensions[1].current_value, 2.0);
    assert!((progress.overall_progress - 0.5).abs() < 1e-12);
    assert_eq!(progress.milestones[0].status, "reached");
    assert_eq!(mastery_signature(&engine), before);
}

#[test]
fn invalid_pack_concept_dimension_and_empty_intersection_are_rejected() {
    let engine = template_engine();
    for (id, scope) in [
        (
            "bad-pack",
            GoalScope {
                pack_ids: vec!["missing".to_owned()],
                ..GoalScope::default()
            },
        ),
        (
            "bad-concept",
            GoalScope {
                concept_ids: vec!["missing".to_owned()],
                ..GoalScope::default()
            },
        ),
        (
            "bad-dimension",
            GoalScope {
                dimension_keys: vec!["missing".to_owned()],
                ..GoalScope::default()
            },
        ),
        (
            "empty-intersection",
            GoalScope {
                pack_ids: vec!["template".to_owned()],
                concept_ids: vec!["ownership".to_owned()],
                ..GoalScope::default()
            },
        ),
    ] {
        let error = engine.create_goal(scoped_goal(id, scope)).unwrap_err();
        assert!(error.to_string().contains("goal.scope"), "{id}: {error}");
    }
}

#[test]
fn selected_goal_works_across_pack_switch_and_keeps_prerequisite_closure() {
    let mut engine = template_engine();
    engine.init_pack(workspace_path("packs/rust")).unwrap();
    engine.switch_pack("rust", None).unwrap();
    engine
        .create_goal(scoped_goal(
            "template-transfer",
            GoalScope {
                concept_ids: vec!["template_transfer_context".to_owned()],
                ..GoalScope::default()
            },
        ))
        .unwrap();

    let workspace = engine.goal_workspace(Some("template-transfer")).unwrap();
    assert_eq!(workspace.actions.len(), 3);
    assert!(workspace
        .actions
        .iter()
        .all(|action| action.concept_id.starts_with("template_")));
    assert!(workspace
        .actions
        .iter()
        .any(|action| action.concept_id == "template_core_terms"));
    engine
        .create_goal(scoped_goal(
            "template-dimension",
            GoalScope {
                dimension_keys: vec!["pack:template".to_owned()],
                ..GoalScope::default()
            },
        ))
        .unwrap();
    let dimension_workspace = engine.goal_workspace(Some("template-dimension")).unwrap();
    assert!(dimension_workspace
        .actions
        .iter()
        .all(|action| action.concept_id.starts_with("template_")));
    let active_pack = engine.list_packs().unwrap();
    assert!(active_pack
        .iter()
        .any(|pack| pack.id == "rust" && pack.active));
}

#[test]
fn no_goal_workspace_matches_existing_batch_and_goal_returns_two_or_three_actions() {
    let engine = template_engine();
    engine
        .conn()
        .execute("UPDATE meta SET value='0' WHERE key='mrt.epsilon'", [])
        .unwrap();
    let existing = engine.get_interleaved_batch(3).unwrap();
    let no_goal = engine.goal_workspace(None).unwrap();
    assert_eq!(no_goal.goal, None);
    assert_eq!(no_goal.actions, existing);

    engine
        .create_goal(scoped_goal(
            "template-pack",
            GoalScope {
                pack_ids: vec!["template".to_owned()],
                ..GoalScope::default()
            },
        ))
        .unwrap();
    let scoped = engine.goal_workspace(Some("template-pack")).unwrap();
    assert!((2..=3).contains(&scoped.actions.len()));
    assert_eq!(scoped.model_version, "goal-workspace-v1");
}

fn template_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_path("packs/template")).unwrap();
    engine
}

fn scoped_goal(id: &str, scope: GoalScope) -> GoalInput {
    let mut goal = GoalInput::new(id, format!("Goal {id}"));
    goal.scope = scope;
    goal
}

fn mastery_signature(engine: &Engine) -> Vec<(String, f64, String)> {
    engine
        .conn()
        .prepare("SELECT concept_id, p_known, phase FROM mastery_states ORDER BY concept_id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn columns(conn: &Connection, table: &str) -> Vec<String> {
    conn.prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
