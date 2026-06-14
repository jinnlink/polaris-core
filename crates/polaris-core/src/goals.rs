use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{PolarisError, Result};

const ACTIVE_STATUS: &str = "active";
const PENDING_STATUS: &str = "pending";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub dimensions: Vec<GoalDimensionInput>,
    pub milestones: Vec<GoalMilestoneInput>,
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
            dimensions: Vec::new(),
            milestones: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    let goal_id = input.id.clone();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO goals(
            id, title, description, created_at, updated_at, status, deadline, pace,
            priority, parent_goal_id, completion_summary
         )
         VALUES (
            ?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ','now'),
            strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?4, ?5, ?6, ?7, ?8, ?9
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
            input.completion_summary
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
                priority, parent_goal_id, completion_summary
             FROM goals
             WHERE id=?1",
            [goal_id],
            |row| {
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
    validate_status(
        "status",
        &input.status,
        &["active", "paused", "completed", "abandoned"],
    )?;
    if !(0..=100).contains(&input.priority) {
        return Err(invalid("priority", input.priority.to_string()));
    }
    for dimension in &input.dimensions {
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
    for milestone in &input.milestones {
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
