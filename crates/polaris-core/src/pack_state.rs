use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{PolarisError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThetaMode {
    Shared,
    Isolated,
}

impl ThetaMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "shared" => Ok(Self::Shared),
            "isolated" => Ok(Self::Isolated),
            other => Err(PolarisError::InvalidParameter {
                key: "theta_mode".to_owned(),
                value: other.to_owned(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Isolated => "isolated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "desktop-bindings", derive(ts_rs::TS))]
pub struct PackSummary {
    pub id: String,
    pub title: String,
    #[cfg_attr(feature = "desktop-bindings", ts(type = "number"))]
    pub concept_count: i64,
    pub active: bool,
    pub theta_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "desktop-bindings", derive(ts_rs::TS))]
pub struct PackSwitchReceipt {
    pub active_pack: String,
    pub theta_mode: String,
}

pub fn active_pack(conn: &Connection) -> Result<Option<String>> {
    let pack: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key='active_pack'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(pack) = pack else {
        return Ok(None);
    };
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM concepts WHERE pack=?1",
        [pack.as_str()],
        |row| row.get(0),
    )?;
    if count == 0 {
        return Err(PolarisError::InvalidParameter {
            key: "active_pack".to_owned(),
            value: format!("pack not installed: {pack}"),
        });
    }
    Ok(Some(pack))
}

pub fn list_packs(conn: &Connection) -> Result<Vec<PackSummary>> {
    let active = active_pack(conn)?;
    let mut stmt = conn.prepare(
        "SELECT c.pack,
                COALESCE((SELECT value FROM meta WHERE key='pack.' || c.pack || '.title'), c.pack),
                COUNT(*)
         FROM concepts c
         WHERE c.pack IS NOT NULL AND c.pack <> ''
         GROUP BY c.pack
         ORDER BY c.pack ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut packs = Vec::new();
    for row in rows {
        let (id, title, concept_count) = row?;
        let theta_mode = theta_mode_for_pack(conn, &id)?.as_str().to_owned();
        packs.push(PackSummary {
            active: active.as_deref() == Some(id.as_str()),
            id,
            title,
            concept_count,
            theta_mode,
        });
    }
    Ok(packs)
}

pub fn theta_mode_for_pack(conn: &Connection, pack_id: &str) -> Result<ThetaMode> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key=?1",
            [format!("pack.{pack_id}.theta_mode")],
            |row| row.get(0),
        )
        .optional()?;
    value
        .as_deref()
        .map(ThetaMode::parse)
        .unwrap_or(Ok(ThetaMode::Shared))
}

pub fn set_pack_theta_mode(conn: &Connection, pack_id: &str, mode: ThetaMode) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
        params![format!("pack.{pack_id}.theta_mode"), mode.as_str()],
    )?;
    Ok(())
}

pub fn ensure_pack_theta_mode(conn: &Connection, pack_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO meta(key, value) VALUES (?1, 'shared')",
        [format!("pack.{pack_id}.theta_mode")],
    )?;
    Ok(())
}

pub fn switch_pack_metadata(
    conn: &Connection,
    pack_id: &str,
    mode: Option<ThetaMode>,
) -> Result<PackSwitchReceipt> {
    ensure_known_pack(conn, pack_id)?;
    if let Some(mode) = mode {
        set_pack_theta_mode(conn, pack_id, mode)?;
    } else {
        ensure_pack_theta_mode(conn, pack_id)?;
    }

    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES ('active_pack', ?1)",
        [pack_id],
    )?;
    let theta_mode = theta_mode_for_pack(conn, pack_id)?;
    Ok(PackSwitchReceipt {
        active_pack: pack_id.to_owned(),
        theta_mode: theta_mode.as_str().to_owned(),
    })
}

pub fn ensure_known_pack(conn: &Connection, pack_id: &str) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM concepts WHERE pack=?1",
        [pack_id],
        |row| row.get(0),
    )?;
    if count == 0 {
        return Err(PolarisError::InvalidParameter {
            key: "pack".to_owned(),
            value: pack_id.to_owned(),
        });
    }
    Ok(())
}

pub fn concept_pack(conn: &Connection, concept_id: &str) -> Result<Option<String>> {
    let pack: Option<String> = conn
        .query_row(
            "SELECT pack FROM concepts WHERE id=?1",
            [concept_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if pack.is_none() {
        let exists: Option<i64> = conn
            .query_row("SELECT 1 FROM concepts WHERE id=?1", [concept_id], |row| {
                row.get(0)
            })
            .optional()?;
        if exists.is_none() {
            return Err(PolarisError::MissingConcept(concept_id.to_owned()));
        }
    }
    Ok(pack.filter(|value| !value.trim().is_empty()))
}
