use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::Result;

const SUCCESS_THRESHOLD: f64 = 0.75;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialPerformanceReport {
    pub by_material: Vec<MaterialPerformanceSummary>,
    pub by_level: Vec<LevelPerformanceSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialPerformanceSummary {
    pub material_id: String,
    pub pack: String,
    pub kind: String,
    pub level: String,
    pub title: String,
    pub attempt_count: usize,
    pub average_final_score: Option<f64>,
    pub first_success_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LevelPerformanceSummary {
    pub pack: String,
    pub level: String,
    pub level_order: usize,
    pub attempt_count: usize,
    pub average_final_score: Option<f64>,
    pub first_success_rate: Option<f64>,
}

#[derive(Default)]
struct Aggregate {
    attempts: usize,
    final_sum: f64,
    final_count: usize,
    first_seen: HashSet<String>,
    first_successes: usize,
}

impl Aggregate {
    fn observe(&mut self, concept_id: &str, final_score: Option<f64>) {
        self.attempts += 1;
        if let Some(score) = final_score {
            self.final_sum += score;
            self.final_count += 1;
            if self.first_seen.insert(concept_id.to_owned()) && score >= SUCCESS_THRESHOLD {
                self.first_successes += 1;
            }
        }
    }

    fn average(&self) -> Option<f64> {
        (self.final_count > 0).then(|| self.final_sum / self.final_count as f64)
    }

    fn first_success_rate(&self) -> Option<f64> {
        (!self.first_seen.is_empty())
            .then(|| self.first_successes as f64 / self.first_seen.len() as f64)
    }
}

pub fn material_performance(
    conn: &Connection,
    pack_filter: Option<&str>,
) -> Result<MaterialPerformanceReport> {
    let mut materials_stmt = conn.prepare(
        "SELECT id, pack, kind, level, title FROM materials
         WHERE (?1 IS NULL OR pack=?1) ORDER BY pack, id",
    )?;
    let materials = materials_stmt
        .query_map([pack_filter], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut material_aggregates: HashMap<String, Aggregate> = HashMap::new();
    let mut level_aggregates: HashMap<(String, String), Aggregate> = HashMap::new();
    let mut attempts_stmt = conn.prepare(
        "SELECT a.material_id, m.pack, m.level, a.concept_id, a.final_score
         FROM attempts a JOIN materials m ON m.id=a.material_id
         WHERE (?1 IS NULL OR m.pack=?1)
         ORDER BY a.created_at, a.id",
    )?;
    let mut rows = attempts_stmt.query([pack_filter])?;
    while let Some(row) = rows.next()? {
        let material_id: String = row.get(0)?;
        let pack: String = row.get(1)?;
        let level: String = row.get(2)?;
        let concept_id: String = row.get(3)?;
        let score: Option<f64> = row.get(4)?;
        material_aggregates
            .entry(material_id)
            .or_default()
            .observe(&concept_id, score);
        level_aggregates
            .entry((pack, level))
            .or_default()
            .observe(&concept_id, score);
    }

    let by_material = materials
        .iter()
        .map(|(id, pack, kind, level, title)| {
            let aggregate = material_aggregates.get(id);
            MaterialPerformanceSummary {
                material_id: id.clone(),
                pack: pack.clone(),
                kind: kind.clone(),
                level: level.clone(),
                title: title.clone(),
                attempt_count: aggregate.map_or(0, |item| item.attempts),
                average_final_score: aggregate.and_then(Aggregate::average),
                first_success_rate: aggregate.and_then(Aggregate::first_success_rate),
            }
        })
        .collect();

    let mut packs: Vec<String> = materials.iter().map(|item| item.1.clone()).collect();
    packs.sort();
    packs.dedup();
    let mut by_level = Vec::new();
    for pack in packs {
        let levels_json: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key=?1",
                [format!("pack.{pack}.material_levels")],
                |row| row.get(0),
            )
            .optional()?;
        let levels: Vec<String> = levels_json
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default();
        for (level_order, level) in levels.into_iter().enumerate() {
            let aggregate = level_aggregates.get(&(pack.clone(), level.clone()));
            by_level.push(LevelPerformanceSummary {
                pack: pack.clone(),
                level,
                level_order,
                attempt_count: aggregate.map_or(0, |item| item.attempts),
                average_final_score: aggregate.and_then(Aggregate::average),
                first_success_rate: aggregate.and_then(Aggregate::first_success_rate),
            });
        }
    }

    Ok(MaterialPerformanceReport {
        by_material,
        by_level,
    })
}
