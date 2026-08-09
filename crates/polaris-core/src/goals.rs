use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::TaskAssignment;
use crate::error::{PolarisError, Result};
use crate::mirt::decode_vector;

const ACTIVE_STATUS: &str = "active";
const PENDING_STATUS: &str = "pending";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GoalScope {
    pub pack_ids: Vec<String>,
    pub dimension_keys: Vec<String>,
    pub concept_ids: Vec<String>,
}

impl GoalScope {
    pub fn is_empty(&self) -> bool {
        self.pack_ids.is_empty() && self.dimension_keys.is_empty() && self.concept_ids.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GoalInput {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub deadline: Option<String>,
    pub pace: Option<String>,
    pub priority: i64,
    pub parent_goal_id: Option<String>,
    pub completion_summary: Option<String>,
    #[serde(default)]
    pub scope: GoalScope,
    pub dimensions: Vec<GoalDimensionInput>,
    pub milestones: Vec<GoalMilestoneInput>,
}

impl Default for GoalInput {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl GoalInput {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            status: ACTIVE_STATUS.to_owned(),
            deadline: None,
            pace: None,
            priority: 50,
            parent_goal_id: None,
            completion_summary: None,
            scope: GoalScope::default(),
            dimensions: Vec::new(),
            milestones: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalDimensionInput {
    pub id: String,
    pub dimension_key: String,
    pub display_name: String,
    pub metric_type: String,
    pub target_value: f64,
    pub target_label: Option<String>,
    pub weight: f64,
    pub current_value: f64,
    pub query_sql: Option<String>,
    pub query_hint: Option<String>,
}

impl GoalDimensionInput {
    pub fn new(
        id: impl Into<String>,
        dimension_key: impl Into<String>,
        display_name: impl Into<String>,
        metric_type: impl Into<String>,
        target_value: f64,
    ) -> Self {
        Self {
            id: id.into(),
            dimension_key: dimension_key.into(),
            display_name: display_name.into(),
            metric_type: metric_type.into(),
            target_value,
            target_label: None,
            weight: 1.0,
            current_value: 0.0,
            query_sql: None,
            query_hint: None,
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalMilestoneInput {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: Value,
    pub status: String,
    pub reached_at: Option<String>,
    pub sort_order: i64,
}

impl GoalMilestoneInput {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        trigger_type: impl Into<String>,
        trigger_config: Value,
        sort_order: i64,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            trigger_type: trigger_type.into(),
            trigger_config,
            status: PENDING_STATUS.to_owned(),
            reached_at: None,
            sort_order,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalRecord {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub deadline: Option<String>,
    pub pace: Option<String>,
    pub priority: i64,
    pub parent_goal_id: Option<String>,
    pub completion_summary: Option<String>,
    pub scope: GoalScope,
    pub dimensions: Vec<GoalDimensionRecord>,
    pub milestones: Vec<GoalMilestoneRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalDimensionRecord {
    pub id: String,
    pub goal_id: String,
    pub dimension_key: String,
    pub display_name: String,
    pub metric_type: String,
    pub target_value: f64,
    pub target_label: Option<String>,
    pub weight: f64,
    pub current_value: f64,
    pub current_updated_at: Option<String>,
    pub query_sql: Option<String>,
    pub query_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalMilestoneRecord {
    pub id: String,
    pub goal_id: String,
    pub title: String,
    pub description: Option<String>,
    pub trigger_type: String,
    pub trigger_config: Value,
    pub status: String,
    pub reached_at: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalProgressReport {
    pub goal_id: String,
    pub status: String,
    pub overall_progress: f64,
    pub dimensions: Vec<GoalDimensionProgress>,
    pub milestones: Vec<GoalMilestoneProgress>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalWorkspaceSnapshot {
    pub model_version: String,
    pub goal: Option<GoalRecord>,
    pub progress: Option<GoalProgressReport>,
    pub scope: GoalScope,
    pub actions: Vec<TaskAssignment>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalDimensionProgress {
    pub dimension_key: String,
    pub display_name: String,
    pub metric_type: String,
    pub current_value: f64,
    pub target_value: f64,
    pub weight: f64,
    pub progress: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalMilestoneProgress {
    pub id: String,
    pub title: String,
    pub status: String,
    pub reached_at: Option<String>,
    pub sort_order: i64,
}

pub fn create_goal(conn: &Connection, input: GoalInput) -> Result<GoalRecord> {
    validate_goal_input(&input)?;
    validate_goal_scope(conn, &input.scope)?;
    let goal_id = input.id.clone();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO goals(
            id, title, description, created_at, updated_at, status, deadline, pace,
            priority, parent_goal_id, completion_summary, scope_json
         )
         VALUES (
            ?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ','now'),
            strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?4, ?5, ?6, ?7, ?8, ?9, ?10
         )",
        params![
            input.id,
            input.title,
            input.description,
            input.status,
            input.deadline,
            input.pace,
            input.priority,
            input.parent_goal_id,
            input.completion_summary,
            serde_json::to_string(&input.scope)?
        ],
    )?;

    for dimension in input.dimensions {
        tx.execute(
            "INSERT INTO goal_dimensions(
                id, goal_id, dimension_key, display_name, metric_type, target_value,
                target_label, weight, current_value, current_updated_at, query_sql, query_hint
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)",
            params![
                dimension.id,
                goal_id,
                dimension.dimension_key,
                dimension.display_name,
                dimension.metric_type,
                dimension.target_value,
                dimension.target_label,
                dimension.weight,
                dimension.current_value,
                dimension.query_sql,
                dimension.query_hint
            ],
        )?;
    }

    for milestone in input.milestones {
        tx.execute(
            "INSERT INTO goal_milestones(
                id, goal_id, title, description, trigger_type, trigger_config,
                status, reached_at, sort_order
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                milestone.id,
                goal_id,
                milestone.title,
                milestone.description,
                milestone.trigger_type,
                serde_json::to_string(&milestone.trigger_config)?,
                milestone.status,
                milestone.reached_at,
                milestone.sort_order
            ],
        )?;
    }

    tx.commit()?;
    goal_snapshot(conn, &goal_id)?.ok_or_else(|| PolarisError::MissingGoal(goal_id))
}

pub fn goal_snapshot(conn: &Connection, goal_id: &str) -> Result<Option<GoalRecord>> {
    let Some(mut goal) = conn
        .query_row(
            "SELECT
                id, title, description, created_at, updated_at, status, deadline, pace,
                priority, parent_goal_id, completion_summary, scope_json
             FROM goals
             WHERE id=?1",
            [goal_id],
            |row| {
                let scope_json: String = row.get(11)?;
                let scope = serde_json::from_str(&scope_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        11,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(GoalRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    status: row.get(5)?,
                    deadline: row.get(6)?,
                    pace: row.get(7)?,
                    priority: row.get(8)?,
                    parent_goal_id: row.get(9)?,
                    completion_summary: row.get(10)?,
                    scope,
                    dimensions: Vec::new(),
                    milestones: Vec::new(),
                })
            },
        )
        .optional()?
    else {
        return Ok(None);
    };

    goal.dimensions = load_dimensions(conn, goal_id)?;
    goal.milestones = load_milestones(conn, goal_id)?;
    Ok(Some(goal))
}

pub fn list_goals(conn: &Connection, status: Option<&str>) -> Result<Vec<GoalRecord>> {
    if let Some(status) = status {
        validate_status(
            "goal.status",
            status,
            &["active", "paused", "completed", "abandoned", "archived"],
        )?;
    }
    let mut statement = conn.prepare(
        "SELECT id FROM goals
         WHERE (?1 IS NULL OR status=?1)
         ORDER BY priority DESC, updated_at DESC, id ASC",
    )?;
    let ids = statement
        .query_map([status], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| goal_snapshot(conn, &id)?.ok_or_else(|| PolarisError::MissingGoal(id)))
        .collect()
}

pub fn update_goal(conn: &Connection, input: GoalInput) -> Result<GoalRecord> {
    validate_goal_input(&input)?;
    validate_goal_scope(conn, &input.scope)?;
    let goal_id = input.id.clone();
    let tx = conn.unchecked_transaction()?;
    let updated = tx.execute(
        "UPDATE goals SET title=?2, description=?3, status=?4, deadline=?5, pace=?6,
             priority=?7, parent_goal_id=?8, completion_summary=?9, scope_json=?10,
             updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?1",
        params![
            input.id,
            input.title,
            input.description,
            input.status,
            input.deadline,
            input.pace,
            input.priority,
            input.parent_goal_id,
            input.completion_summary,
            serde_json::to_string(&input.scope)?
        ],
    )?;
    if updated == 0 {
        return Err(PolarisError::MissingGoal(goal_id));
    }
    tx.execute("DELETE FROM goal_dimensions WHERE goal_id=?1", [&goal_id])?;
    tx.execute("DELETE FROM goal_milestones WHERE goal_id=?1", [&goal_id])?;
    insert_goal_children(&tx, &goal_id, input.dimensions, input.milestones)?;
    tx.commit()?;
    goal_snapshot(conn, &goal_id)?.ok_or_else(|| PolarisError::MissingGoal(goal_id))
}

pub fn archive_goal(conn: &Connection, goal_id: &str) -> Result<GoalRecord> {
    let updated = conn.execute(
        "UPDATE goals SET status='archived', updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?1",
        [goal_id],
    )?;
    if updated == 0 {
        return Err(PolarisError::MissingGoal(goal_id.to_owned()));
    }
    goal_snapshot(conn, goal_id)?.ok_or_else(|| PolarisError::MissingGoal(goal_id.to_owned()))
}

pub fn delete_goal(conn: &Connection, goal_id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM goal_milestones WHERE goal_id=?1", [goal_id])?;
    tx.execute("DELETE FROM goal_dimensions WHERE goal_id=?1", [goal_id])?;
    let deleted = tx.execute("DELETE FROM goals WHERE id=?1", [goal_id])?;
    if deleted == 0 {
        return Err(PolarisError::MissingGoal(goal_id.to_owned()));
    }
    tx.commit()?;
    Ok(())
}

pub fn update_goal_dimension_value(
    conn: &Connection,
    goal_id: &str,
    dimension_key: &str,
    current_value: f64,
) -> Result<()> {
    if !current_value.is_finite() {
        return Err(invalid("current_value", current_value.to_string()));
    }
    let updated = conn.execute(
        "UPDATE goal_dimensions
         SET current_value=?1, current_updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE goal_id=?2 AND dimension_key=?3",
        params![current_value, goal_id, dimension_key],
    )?;
    if updated == 0 {
        return Err(PolarisError::MissingGoal(goal_id.to_owned()));
    }
    conn.execute(
        "UPDATE goals
         SET updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=?1",
        [goal_id],
    )?;
    Ok(())
}

pub fn goal_progress(conn: &Connection, goal_id: &str) -> Result<GoalProgressReport> {
    let goal = goal_snapshot(conn, goal_id)?
        .ok_or_else(|| PolarisError::MissingGoal(goal_id.to_owned()))?;
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    let dimensions = goal
        .dimensions
        .iter()
        .map(|dimension| {
            let progress = dimension_progress(dimension.current_value, dimension.target_value);
            if dimension.weight > 0.0 {
                weighted_sum += progress * dimension.weight;
                weight_sum += dimension.weight;
            }
            GoalDimensionProgress {
                dimension_key: dimension.dimension_key.clone(),
                display_name: dimension.display_name.clone(),
                metric_type: dimension.metric_type.clone(),
                current_value: dimension.current_value,
                target_value: dimension.target_value,
                weight: dimension.weight,
                progress,
            }
        })
        .collect();

    let milestones = goal
        .milestones
        .iter()
        .map(|milestone| GoalMilestoneProgress {
            id: milestone.id.clone(),
            title: milestone.title.clone(),
            status: milestone.status.clone(),
            reached_at: milestone.reached_at.clone(),
            sort_order: milestone.sort_order,
        })
        .collect();

    Ok(GoalProgressReport {
        goal_id: goal.id,
        status: goal.status,
        overall_progress: if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            0.0
        },
        dimensions,
        milestones,
    })
}

pub fn refresh_goal_milestones(
    conn: &Connection,
    goal_id: &str,
) -> Result<Vec<GoalMilestoneRecord>> {
    let dimensions = load_dimensions(conn, goal_id)?;
    if dimensions.is_empty() && goal_snapshot(conn, goal_id)?.is_none() {
        return Err(PolarisError::MissingGoal(goal_id.to_owned()));
    }
    let current_values = dimensions
        .into_iter()
        .map(|dimension| (dimension.dimension_key, dimension.current_value))
        .collect::<BTreeMap<_, _>>();

    for milestone in load_milestones(conn, goal_id)? {
        if milestone.status != PENDING_STATUS {
            continue;
        }
        if milestone_reached(&milestone, &current_values) {
            conn.execute(
                "UPDATE goal_milestones
                 SET status='reached', reached_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
                 WHERE id=?1",
                [milestone.id.as_str()],
            )?;
        }
    }

    load_milestones(conn, goal_id)
}

pub fn refresh_goal_progress(conn: &Connection, goal_id: &str) -> Result<GoalProgressReport> {
    let goal = goal_snapshot(conn, goal_id)?
        .ok_or_else(|| PolarisError::MissingGoal(goal_id.to_owned()))?;
    let derived = derive_goal_progress_for(conn, &goal)?;
    let values = derived
        .dimensions
        .iter()
        .map(|dimension| (dimension.dimension_key.as_str(), dimension.current_value))
        .collect::<BTreeMap<_, _>>();
    let tx = conn.unchecked_transaction()?;
    for dimension in &goal.dimensions {
        let value = values
            .get(dimension.dimension_key.as_str())
            .copied()
            .ok_or_else(|| invalid("goal.dimension", &dimension.dimension_key))?;
        tx.execute(
            "UPDATE goal_dimensions
             SET current_value=?1, current_updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id=?2",
            params![value, dimension.id],
        )?;
    }
    tx.execute(
        "UPDATE goals SET updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?1",
        [goal_id],
    )?;
    tx.commit()?;
    refresh_goal_milestones(conn, goal_id)?;
    goal_progress(conn, goal_id)
}

pub fn derive_goal_progress(conn: &Connection, goal_id: &str) -> Result<GoalProgressReport> {
    let goal = goal_snapshot(conn, goal_id)?
        .ok_or_else(|| PolarisError::MissingGoal(goal_id.to_owned()))?;
    derive_goal_progress_for(conn, &goal)
}

fn derive_goal_progress_for(conn: &Connection, goal: &GoalRecord) -> Result<GoalProgressReport> {
    let concept_ids = concept_ids_for_goal_scope(conn, &goal.scope)?;
    if concept_ids.is_empty() {
        return Err(invalid("goal.scope", "scope resolves to no concepts"));
    }
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    let mut current_values = BTreeMap::new();
    let mut dimensions = Vec::new();
    for dimension in &goal.dimensions {
        let current_value = derived_dimension_value(conn, dimension, &concept_ids)?;
        let progress = dimension_progress(current_value, dimension.target_value);
        weighted_sum += progress * dimension.weight;
        weight_sum += dimension.weight;
        current_values.insert(dimension.dimension_key.clone(), current_value);
        dimensions.push(GoalDimensionProgress {
            dimension_key: dimension.dimension_key.clone(),
            display_name: dimension.display_name.clone(),
            metric_type: dimension.metric_type.clone(),
            current_value,
            target_value: dimension.target_value,
            weight: dimension.weight,
            progress,
        });
    }
    let milestones = goal
        .milestones
        .iter()
        .map(|milestone| GoalMilestoneProgress {
            id: milestone.id.clone(),
            title: milestone.title.clone(),
            status: if milestone.status == PENDING_STATUS
                && milestone_reached(milestone, &current_values)
            {
                "reached".to_owned()
            } else {
                milestone.status.clone()
            },
            reached_at: milestone.reached_at.clone(),
            sort_order: milestone.sort_order,
        })
        .collect();
    Ok(GoalProgressReport {
        goal_id: goal.id.clone(),
        status: goal.status.clone(),
        overall_progress: if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            0.0
        },
        dimensions,
        milestones,
    })
}

pub fn concept_ids_for_goal_scope(conn: &Connection, scope: &GoalScope) -> Result<Vec<String>> {
    let default_pack = if scope.is_empty() {
        crate::pack_state::active_pack(conn)?
    } else {
        None
    };
    let latent_dimensions: Vec<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='latent.dims'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_default();
    let selected_dimension_indexes = scope
        .dimension_keys
        .iter()
        .filter_map(|key| {
            latent_dimensions
                .iter()
                .position(|dimension| dimension == key)
        })
        .collect::<Vec<_>>();
    let mut statement =
        conn.prepare("SELECT id, pack, q FROM concepts ORDER BY seed_order ASC, id ASC")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<Vec<u8>>>(2)?,
        ))
    })?;
    let mut concept_ids = Vec::new();
    for row in rows {
        let (id, pack, q) = row?;
        if !scope.pack_ids.is_empty() && !scope.pack_ids.contains(&pack) {
            continue;
        }
        if default_pack.as_deref().is_some_and(|active| active != pack) {
            continue;
        }
        if !scope.concept_ids.is_empty() && !scope.concept_ids.contains(&id) {
            continue;
        }
        if !selected_dimension_indexes.is_empty() {
            let Some(q) = q else {
                continue;
            };
            let vector = decode_vector(&q)?;
            if !selected_dimension_indexes.iter().any(|index| {
                vector
                    .get(*index)
                    .is_some_and(|value| value.abs() > f64::EPSILON)
            }) {
                continue;
            }
        }
        concept_ids.push(id);
    }
    if concept_ids.is_empty() {
        return Ok(concept_ids);
    }
    let selected_json = serde_json::to_string(&concept_ids)?;
    let mut closure = conn.prepare(
        "WITH RECURSIVE required(id) AS (
             SELECT value FROM json_each(?1)
             UNION
             SELECT e.src FROM edges e JOIN required r ON e.dst=r.id
             WHERE e.type='prerequisite'
         )
         SELECT c.id FROM concepts c JOIN required r ON r.id=c.id
         ORDER BY c.seed_order ASC, c.id ASC",
    )?;
    let ids = closure
        .query_map([selected_json], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids)
}

fn derived_dimension_value(
    conn: &Connection,
    dimension: &GoalDimensionRecord,
    concept_ids: &[String],
) -> Result<f64> {
    let ids_json = serde_json::to_string(concept_ids)?;
    match dimension.metric_type.as_str() {
        "mastery_mean" => conn
            .query_row(
                "SELECT COALESCE(AVG(COALESCE(ms.p_known, c.p_init,
                         CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL))), 0.0)
                 FROM concepts c LEFT JOIN mastery_states ms ON ms.concept_id=c.id
                 WHERE EXISTS(SELECT 1 FROM json_each(?1) ids WHERE ids.value=c.id)",
                [ids_json],
                |row| row.get(0),
            )
            .map_err(Into::into),
        "score" | "mastery_percent" => conn
            .query_row(
                "SELECT COALESCE(AVG(COALESCE(ms.p_known, c.p_init,
                         CAST((SELECT value FROM meta WHERE key='bkt.p_init') AS REAL))), 0.0) * 100.0
                 FROM concepts c LEFT JOIN mastery_states ms ON ms.concept_id=c.id
                 WHERE EXISTS(SELECT 1 FROM json_each(?1) ids WHERE ids.value=c.id)",
                [ids_json],
                |row| row.get(0),
            )
            .map_err(Into::into),
        "count" | "mastered_concepts" => {
            let cut: f64 = conn.query_row(
                "SELECT CAST(value AS REAL) FROM meta WHERE key='bkt.cut_hi'",
                [],
                |row| row.get(0),
            )?;
            conn.query_row(
                "SELECT COUNT(*) FROM mastery_states ms
                 WHERE ms.p_known>=?2
                   AND EXISTS(SELECT 1 FROM json_each(?1) ids WHERE ids.value=ms.concept_id)",
                params![ids_json, cut],
                |row| row.get::<_, i64>(0).map(|value| value as f64),
            )
            .map_err(Into::into)
        }
        "graded_attempts" => conn
            .query_row(
                "SELECT COUNT(*) FROM attempts a
                 WHERE a.final_score IS NOT NULL
                   AND EXISTS(SELECT 1 FROM json_each(?1) ids WHERE ids.value=a.concept_id)",
                [ids_json],
                |row| row.get::<_, i64>(0).map(|value| value as f64),
            )
            .map_err(Into::into),
        "evidence_count" => conn
            .query_row(
                "SELECT COUNT(DISTINCT e.id) FROM evidence_items e
                 WHERE EXISTS(
                     SELECT 1 FROM json_each(COALESCE(e.concept_ids_json, '[]')) concepts
                     WHERE EXISTS(SELECT 1 FROM json_each(?1) ids WHERE ids.value=concepts.value)
                 )",
                [ids_json],
                |row| row.get::<_, i64>(0).map(|value| value as f64),
            )
            .map_err(Into::into),
        other => Err(invalid("goal.dimension.metric_type", other)),
    }
}

fn load_dimensions(conn: &Connection, goal_id: &str) -> Result<Vec<GoalDimensionRecord>> {
    let mut stmt = conn.prepare(
        "SELECT
            id, goal_id, dimension_key, display_name, metric_type, target_value,
            target_label, weight, current_value, current_updated_at, query_sql, query_hint
         FROM goal_dimensions
         WHERE goal_id=?1
         ORDER BY rowid ASC",
    )?;
    let rows = stmt.query_map([goal_id], |row| {
        Ok(GoalDimensionRecord {
            id: row.get(0)?,
            goal_id: row.get(1)?,
            dimension_key: row.get(2)?,
            display_name: row.get(3)?,
            metric_type: row.get(4)?,
            target_value: row.get(5)?,
            target_label: row.get(6)?,
            weight: row.get(7)?,
            current_value: row.get(8)?,
            current_updated_at: row.get(9)?,
            query_sql: row.get(10)?,
            query_hint: row.get(11)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn insert_goal_children(
    conn: &Connection,
    goal_id: &str,
    dimensions: Vec<GoalDimensionInput>,
    milestones: Vec<GoalMilestoneInput>,
) -> Result<()> {
    for dimension in dimensions {
        conn.execute(
            "INSERT INTO goal_dimensions(
                id, goal_id, dimension_key, display_name, metric_type, target_value,
                target_label, weight, current_value, current_updated_at, query_sql, query_hint
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)",
            params![
                dimension.id,
                goal_id,
                dimension.dimension_key,
                dimension.display_name,
                dimension.metric_type,
                dimension.target_value,
                dimension.target_label,
                dimension.weight,
                dimension.current_value,
                dimension.query_sql,
                dimension.query_hint
            ],
        )?;
    }
    for milestone in milestones {
        conn.execute(
            "INSERT INTO goal_milestones(
                id, goal_id, title, description, trigger_type, trigger_config,
                status, reached_at, sort_order
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                milestone.id,
                goal_id,
                milestone.title,
                milestone.description,
                milestone.trigger_type,
                serde_json::to_string(&milestone.trigger_config)?,
                milestone.status,
                milestone.reached_at,
                milestone.sort_order
            ],
        )?;
    }
    Ok(())
}

fn load_milestones(conn: &Connection, goal_id: &str) -> Result<Vec<GoalMilestoneRecord>> {
    let mut stmt = conn.prepare(
        "SELECT
            id, goal_id, title, description, trigger_type, trigger_config,
            status, reached_at, sort_order
         FROM goal_milestones
         WHERE goal_id=?1
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([goal_id], |row| {
        let trigger_config: String = row.get(5)?;
        let trigger_config = serde_json::from_str(&trigger_config).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(GoalMilestoneRecord {
            id: row.get(0)?,
            goal_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            trigger_type: row.get(4)?,
            trigger_config,
            status: row.get(6)?,
            reached_at: row.get(7)?,
            sort_order: row.get(8)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn validate_goal_input(input: &GoalInput) -> Result<()> {
    if input.id.trim().is_empty() || input.title.trim().is_empty() {
        return Err(invalid("goal.identity", "id and title must be non-empty"));
    }
    validate_status(
        "status",
        &input.status,
        &["active", "paused", "completed", "abandoned", "archived"],
    )?;
    if !(0..=100).contains(&input.priority) {
        return Err(invalid("priority", input.priority.to_string()));
    }
    let mut dimension_ids = BTreeSet::new();
    let mut dimension_keys = BTreeSet::new();
    for dimension in &input.dimensions {
        if dimension.id.trim().is_empty()
            || dimension.dimension_key.trim().is_empty()
            || dimension.display_name.trim().is_empty()
            || dimension.metric_type.trim().is_empty()
            || !dimension_ids.insert(dimension.id.as_str())
            || !dimension_keys.insert(dimension.dimension_key.as_str())
        {
            return Err(invalid(
                "goal.dimension",
                "UNIQUE constraint: blank or duplicate dimension",
            ));
        }
        if !dimension.target_value.is_finite() || dimension.target_value <= 0.0 {
            return Err(invalid("target_value", dimension.target_value.to_string()));
        }
        if !dimension.weight.is_finite() || dimension.weight <= 0.0 {
            return Err(invalid("weight", dimension.weight.to_string()));
        }
        if !dimension.current_value.is_finite() {
            return Err(invalid(
                "current_value",
                dimension.current_value.to_string(),
            ));
        }
    }
    let mut milestone_ids = BTreeSet::new();
    let mut milestone_orders = BTreeSet::new();
    for milestone in &input.milestones {
        if milestone.id.trim().is_empty()
            || milestone.title.trim().is_empty()
            || !milestone_ids.insert(milestone.id.as_str())
            || !milestone_orders.insert(milestone.sort_order)
        {
            return Err(invalid(
                "goal.milestone",
                "UNIQUE constraint: blank or duplicate milestone",
            ));
        }
        validate_status(
            "milestone.status",
            &milestone.status,
            &["pending", "reached", "skipped"],
        )?;
        if milestone.trigger_config.is_null() {
            return Err(invalid("trigger_config", "null"));
        }
    }
    Ok(())
}

fn validate_goal_scope(conn: &Connection, scope: &GoalScope) -> Result<()> {
    validate_scope_values("goal.scope.pack_ids", &scope.pack_ids)?;
    validate_scope_values("goal.scope.dimension_keys", &scope.dimension_keys)?;
    validate_scope_values("goal.scope.concept_ids", &scope.concept_ids)?;

    for pack_id in &scope.pack_ids {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM concepts WHERE pack=?1)",
            [pack_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(invalid("goal.scope.pack_ids", pack_id));
        }
    }
    for concept_id in &scope.concept_ids {
        let pack: Option<String> = conn
            .query_row(
                "SELECT pack FROM concepts WHERE id=?1",
                [concept_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(pack) = pack else {
            return Err(invalid("goal.scope.concept_ids", concept_id));
        };
        if !scope.pack_ids.is_empty() && !scope.pack_ids.contains(&pack) {
            return Err(invalid(
                "goal.scope.concept_ids",
                format!("{concept_id} is outside selected packs"),
            ));
        }
    }
    if !scope.dimension_keys.is_empty() {
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key='latent.dims'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let dimensions = raw
            .as_deref()
            .map(serde_json::from_str::<Vec<String>>)
            .transpose()?
            .unwrap_or_default();
        for key in &scope.dimension_keys {
            if !dimensions.contains(key) {
                return Err(invalid("goal.scope.dimension_keys", key));
            }
        }
    }
    if !scope.is_empty() && concept_ids_for_goal_scope(conn, scope)?.is_empty() {
        return Err(invalid("goal.scope", "scope resolves to no concepts"));
    }
    Ok(())
}

fn validate_scope_values(key: &str, values: &[String]) -> Result<()> {
    let mut unique = BTreeSet::new();
    if values
        .iter()
        .any(|value| value.trim().is_empty() || !unique.insert(value.as_str()))
    {
        return Err(invalid(key, "values must be non-empty and unique"));
    }
    Ok(())
}

fn validate_status(key: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(invalid(key, value.to_owned()))
    }
}

fn invalid(key: &str, value: impl Into<String>) -> PolarisError {
    PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: value.into(),
    }
}

fn dimension_progress(current_value: f64, target_value: f64) -> f64 {
    (current_value / target_value).clamp(0.0, 1.0)
}

fn milestone_reached(
    milestone: &GoalMilestoneRecord,
    current_values: &BTreeMap<String, f64>,
) -> bool {
    if milestone.trigger_type != "dimension_threshold" {
        return false;
    }
    let Some(dimension_key) = milestone
        .trigger_config
        .get("dimension_key")
        .and_then(Value::as_str)
    else {
        return false;
    };
    let Some(operator) = milestone
        .trigger_config
        .get("operator")
        .and_then(Value::as_str)
    else {
        return false;
    };
    let Some(target) = milestone
        .trigger_config
        .get("value")
        .and_then(Value::as_f64)
    else {
        return false;
    };
    let Some(current) = current_values.get(dimension_key).copied() else {
        return false;
    };
    match operator {
        ">=" => current >= target,
        ">" => current > target,
        "<=" => current <= target,
        "<" => current < target,
        "==" | "=" => (current - target).abs() < f64::EPSILON,
        _ => false,
    }
}
