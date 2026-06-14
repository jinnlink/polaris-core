use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::goals::{GoalDimensionInput, GoalInput, GoalMilestoneInput};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn migration_creates_goal_engine_tables_and_indexes() {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    for table in ["goals", "goal_dimensions", "goal_milestones"] {
        assert_eq!(
            sqlite_object_count(&conn, "table", table),
            1,
            "missing {table}"
        );
    }

    for column in [
        "id",
        "title",
        "description",
        "created_at",
        "updated_at",
        "status",
        "deadline",
        "pace",
        "priority",
        "parent_goal_id",
        "completion_summary",
    ] {
        assert!(table_columns(&conn, "goals").contains(&column.to_owned()));
    }
    assert_eq!(
        column_default(&conn, "goals", "status"),
        Some("'active'".to_owned())
    );
    assert_eq!(
        column_default(&conn, "goals", "priority"),
        Some("50".to_owned())
    );
    assert!(column_is_not_null(&conn, "goal_dimensions", "target_value"));
    assert!(column_is_not_null(
        &conn,
        "goal_milestones",
        "trigger_config"
    ));
    assert!(unique_index_exists(
        &conn,
        "goal_dimensions",
        &["goal_id", "dimension_key"]
    ));
    assert!(unique_index_exists(
        &conn,
        "goal_milestones",
        &["goal_id", "sort_order"]
    ));

    for index in [
        "idx_goals_updated_at",
        "idx_goals_status",
        "idx_goals_parent",
        "idx_goal_dim_goal",
        "idx_milestone_goal",
    ] {
        assert_eq!(
            sqlite_object_count(&conn, "index", index),
            1,
            "missing {index}"
        );
    }
}

#[test]
fn create_goal_persists_dimensions_and_milestones() {
    let engine = goal_engine();

    let goal = engine.create_goal(sample_goal()).unwrap();

    assert_eq!(goal.id, "rust-transfer");
    assert_eq!(goal.title, "Rust transfer fluency");
    assert_eq!(goal.status, "active");
    assert_eq!(goal.priority, 70);
    assert_eq!(goal.dimensions.len(), 2);
    assert_eq!(goal.dimensions[0].dimension_key, "ownership_transfer");
    assert_eq!(goal.dimensions[0].target_value, 10.0);
    assert_eq!(goal.milestones.len(), 2);
    assert_eq!(goal.milestones[0].title, "First transfer card");
    assert_eq!(goal.milestones[1].sort_order, 20);

    let snapshot = engine
        .goal_snapshot("rust-transfer")
        .unwrap()
        .expect("goal snapshot");
    assert_eq!(snapshot, goal);
}

#[test]
fn create_goal_rolls_back_when_child_rows_violate_constraints() {
    let engine = goal_engine();
    let mut input = sample_goal();
    input.dimensions[1].dimension_key = "ownership_transfer".to_owned();

    let error = engine.create_goal(input).unwrap_err().to_string();

    assert!(error.contains("UNIQUE"));
    let count: i64 = engine
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM goals WHERE id='rust-transfer'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn create_goal_rejects_non_positive_dimension_targets() {
    let engine = goal_engine();
    let mut input = sample_goal();
    input.dimensions[0].target_value = 0.0;

    let error = engine.create_goal(input).unwrap_err().to_string();

    assert!(error.contains("invalid parameter target_value"));
}

#[test]
fn goal_progress_is_weighted_and_clamped() {
    let engine = goal_engine();
    engine.create_goal(sample_goal()).unwrap();

    engine
        .update_goal_dimension_value("rust-transfer", "ownership_transfer", 4.0)
        .unwrap();
    engine
        .update_goal_dimension_value("rust-transfer", "borrow_checker", 99.0)
        .unwrap();

    let progress = engine.goal_progress("rust-transfer").unwrap();

    assert_eq!(progress.goal_id, "rust-transfer");
    assert_eq!(progress.dimensions[0].progress, 0.4);
    assert_eq!(progress.dimensions[1].progress, 1.0);
    assert!((progress.overall_progress - 0.6).abs() < 1e-9);
}

#[test]
fn refresh_goal_milestones_reaches_dimension_thresholds_only() {
    let engine = goal_engine();
    engine.create_goal(sample_goal()).unwrap();

    engine
        .update_goal_dimension_value("rust-transfer", "ownership_transfer", 1.0)
        .unwrap();
    let before = engine.refresh_goal_milestones("rust-transfer").unwrap();
    assert_eq!(before[0].status, "pending");
    assert_eq!(before[1].status, "pending");

    engine
        .update_goal_dimension_value("rust-transfer", "ownership_transfer", 3.0)
        .unwrap();
    let after = engine.refresh_goal_milestones("rust-transfer").unwrap();

    assert_eq!(after[0].status, "reached");
    assert!(after[0].reached_at.is_some());
    assert_eq!(after[1].status, "pending");
}

fn goal_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    Engine::new(conn)
}

fn sample_goal() -> GoalInput {
    let mut input = GoalInput::new("rust-transfer", "Rust transfer fluency");
    input.description = Some("Move ownership knowledge into unfamiliar tasks.".to_owned());
    input.deadline = Some("2026-09-01".to_owned());
    input.pace = Some("steady".to_owned());
    input.priority = 70;
    input.dimensions = vec![
        GoalDimensionInput::new(
            "dim-ownership",
            "ownership_transfer",
            "Ownership transfer cards",
            "count",
            10.0,
        )
        .with_weight(2.0),
        GoalDimensionInput::new(
            "dim-borrow-checker",
            "borrow_checker",
            "Borrow checker debugging",
            "score",
            80.0,
        ),
    ];
    input.milestones = vec![
        GoalMilestoneInput::new(
            "milestone-1",
            "First transfer card",
            "dimension_threshold",
            json!({"dimension_key": "ownership_transfer", "operator": ">=", "value": 3.0}),
            10,
        ),
        GoalMilestoneInput::new(
            "milestone-2",
            "Manual mentor review",
            "manual",
            json!({}),
            20,
        ),
    ];
    input
}

fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}

fn column_default(conn: &Connection, table: &str, column: &str) -> Option<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    let mut rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, Option<String>>(4)?))
        })
        .unwrap();
    rows.find_map(|row| {
        let (name, default_value) = row.unwrap();
        (name == column).then_some(default_value).flatten()
    })
}

fn column_is_not_null(conn: &Connection, table: &str, column: &str) -> bool {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap();
    let mut rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)?))
        })
        .unwrap();
    rows.any(|row| {
        let (name, not_null) = row.unwrap();
        name == column && not_null == 1
    })
}

fn unique_index_exists(conn: &Connection, table: &str, columns: &[&str]) -> bool {
    let mut stmt = conn
        .prepare(&format!("PRAGMA index_list({table})"))
        .unwrap();
    let indexes = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();

    indexes
        .into_iter()
        .any(|(index, unique)| unique == 1 && index_columns(conn, &index) == columns)
}

fn index_columns(conn: &Connection, index: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA index_info({index})"))
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(2))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
}

fn sqlite_object_count(conn: &Connection, object_type: &str, name: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type=?1 AND name=?2",
        (object_type, name),
        |row| row.get(0),
    )
    .unwrap()
}
