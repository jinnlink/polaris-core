use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::{schema_version, CURRENT_SCHEMA_VERSION};
use crate::error::{PolarisError, Result};

const PROFILE_EVENT_TYPE: &str = "profile_measurement";
const PROFILE_EXPORT_SCHEMA_VERSION: i64 = 1;
const REGISTRY_JSON: &str = include_str!("../resources/profile/instruments.json");

pub const DELETE_ALL_CONFIRMATION: &str = "DELETE ALL POLARIS LEARNING DATA";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileLicense {
    pub name: String,
    pub url: String,
    pub redistributable: bool,
    pub attribution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileInstrumentItem {
    pub id: String,
    pub locale: String,
    pub dimension: String,
    pub prompt: String,
    pub keyed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileScoring {
    pub method: String,
    pub response_min: i64,
    pub response_max: i64,
    pub reverse_formula: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileInstrument {
    pub id: String,
    pub title: String,
    pub version: String,
    pub citation: String,
    pub source_url: String,
    pub license: ProfileLicense,
    pub locales: Vec<String>,
    pub items: Vec<ProfileInstrumentItem>,
    pub scoring: ProfileScoring,
    pub admin_modes: Vec<String>,
    pub interpretation_notice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSettings {
    pub version: i64,
    pub enabled: bool,
    pub disclosure_required: bool,
    pub disclosure_acknowledged_at: Option<String>,
    pub summary_sharing_enabled: bool,
    pub paused_until: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSettingsUpdate {
    pub enabled: Option<bool>,
    pub acknowledge_disclosure: bool,
    pub summary_sharing_enabled: Option<bool>,
    pub paused_until: Option<String>,
    pub clear_pause: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMeasurementStatus {
    Recorded,
    Disabled,
    DisclosureRequired,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMeasurementInput {
    pub session_id: String,
    pub instrument_id: String,
    pub instrument_version: String,
    pub item_id: String,
    pub locale: String,
    pub admin_mode: String,
    pub response: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMeasurementReceipt {
    pub status: ProfileMeasurementStatus,
    pub event_id: Option<String>,
    pub recorded_at: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileScope {
    Global,
    Pack,
    Goal,
}

impl ProfileScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Pack => "pack",
            Self::Goal => "goal",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "global" => Ok(Self::Global),
            "pack" => Ok(Self::Pack),
            "goal" => Ok(Self::Goal),
            _ => Err(invalid("profile.scope", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileGateStatus {
    Unfit,
    Shadow,
    Active,
    Suspended,
}

impl ProfileGateStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unfit => "unfit",
            Self::Shadow => "shadow",
            Self::Active => "active",
            Self::Suspended => "suspended",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "unfit" => Ok(Self::Unfit),
            "shadow" => Ok(Self::Shadow),
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            _ => Err(invalid("profile.gate_status", value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileDimensionInput {
    pub scope: ProfileScope,
    pub scope_id: Option<String>,
    pub dimension_key: String,
    pub mean: f64,
    pub variance: f64,
    pub evidence_count: i64,
    pub model_version: String,
    pub gate_status: ProfileGateStatus,
    pub provenance: Value,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileDimension {
    pub scope: ProfileScope,
    pub scope_id: Option<String>,
    pub dimension_key: String,
    pub mean: f64,
    pub variance: f64,
    pub evidence_count: i64,
    pub model_version: String,
    pub gate_status: ProfileGateStatus,
    pub provenance: Value,
    pub evidence_ids: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileValidationRunInput {
    pub id: String,
    pub scope: ProfileScope,
    pub scope_id: Option<String>,
    pub dimension_key: String,
    pub model_version: String,
    pub status: ProfileGateStatus,
    pub metrics: Value,
    pub provenance: Value,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileValidationRun {
    pub id: String,
    pub scope: ProfileScope,
    pub scope_id: Option<String>,
    pub dimension_key: String,
    pub model_version: String,
    pub status: ProfileGateStatus,
    pub metrics: Value,
    pub provenance: Value,
    pub evidence_ids: Vec<String>,
    pub ran_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileMeasurementExport {
    pub event_id: String,
    pub session_id: Option<String>,
    pub recorded_at: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDataAction {
    pub id: String,
    pub action: String,
    pub measurements_deleted: i64,
    pub dimensions_deleted: i64,
    pub validation_runs_deleted: i64,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalProfileExport {
    pub schema_version: i64,
    pub exported_at: String,
    pub settings: ProfileSettings,
    pub instruments: Vec<ProfileInstrument>,
    pub measurements: Vec<ProfileMeasurementExport>,
    pub dimensions: Vec<ProfileDimension>,
    pub validation_runs: Vec<ProfileValidationRun>,
    pub data_actions: Vec<ProfileDataAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalProfileIntegrationSummary {
    pub generated_at: String,
    pub dimensions: Vec<ProfileDimension>,
    pub notice: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalProfileOverview {
    pub generated_at: String,
    pub settings: ProfileSettings,
    pub instrument_count: usize,
    pub measurement_count: i64,
    pub dimensions: Vec<ProfileDimension>,
    pub validation_runs: Vec<ProfileValidationRun>,
    pub data_actions: Vec<ProfileDataAction>,
    pub notice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileResetReceipt {
    pub action_id: String,
    pub at: String,
    pub measurements_deleted: i64,
    pub dimensions_deleted: i64,
    pub validation_runs_deleted: i64,
    pub learning_attempts_preserved: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullDataDeletionRequest {
    pub database_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub confirmation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullDataDeletionReceipt {
    pub deleted_at: String,
    pub database_path: String,
    pub backup_path: Option<String>,
    pub files_deleted: usize,
    pub local_secrets_deleted: usize,
    pub empty_database_created: bool,
}

pub fn profile_instruments() -> Result<Vec<ProfileInstrument>> {
    validate_profile_registry_json(REGISTRY_JSON)
}

pub fn validate_profile_registry_json(raw: &str) -> Result<Vec<ProfileInstrument>> {
    let instruments: Vec<ProfileInstrument> = serde_json::from_str(raw)?;
    if instruments.is_empty() {
        return Err(invalid("profile.registry", "must contain instruments"));
    }

    let mut instrument_ids = BTreeSet::new();
    for instrument in &instruments {
        require_nonempty("profile.registry.instrument.id", &instrument.id)?;
        require_nonempty("profile.registry.instrument.title", &instrument.title)?;
        require_nonempty("profile.registry.instrument.version", &instrument.version)?;
        require_nonempty("profile.registry.instrument.citation", &instrument.citation)?;
        require_https(
            "profile.registry.instrument.source_url",
            &instrument.source_url,
        )?;
        if !instrument.license.redistributable {
            return Err(invalid(
                "profile.registry.instrument.license.redistributable",
                format!("{} must be redistributable", instrument.id),
            ));
        }
        if !matches!(
            instrument.license.name.as_str(),
            "Public Domain" | "CC BY 4.0"
        ) {
            return Err(invalid(
                "profile.registry.instrument.license.name",
                format!(
                    "{} is not an approved bundled license",
                    instrument.license.name
                ),
            ));
        }
        require_https(
            "profile.registry.instrument.license.url",
            &instrument.license.url,
        )?;
        require_nonempty(
            "profile.registry.instrument.license.attribution",
            &instrument.license.attribution,
        )?;
        if instrument.id.to_ascii_lowercase().contains("pals")
            || instrument.title.to_ascii_lowercase().contains("pals")
        {
            return Err(invalid(
                "profile.registry.instrument",
                "PALS cannot be bundled without a recorded redistribution license",
            ));
        }
        if !instrument_ids.insert(instrument.id.as_str()) {
            return Err(invalid(
                "profile.registry.instrument.id",
                format!("duplicate {}", instrument.id),
            ));
        }
        if instrument.locales.is_empty() || instrument.admin_modes.is_empty() {
            return Err(invalid(
                "profile.registry.instrument",
                format!("{} requires locale and admin_mode metadata", instrument.id),
            ));
        }
        if instrument.items.is_empty()
            || instrument.scoring.response_min >= instrument.scoring.response_max
        {
            return Err(invalid(
                "profile.registry.instrument.scoring",
                format!(
                    "{} requires items and a valid response range",
                    instrument.id
                ),
            ));
        }
        require_nonempty(
            "profile.registry.instrument.scoring.method",
            &instrument.scoring.method,
        )?;
        require_nonempty(
            "profile.registry.instrument.interpretation_notice",
            &instrument.interpretation_notice,
        )?;

        let mut locales = BTreeSet::new();
        for locale in &instrument.locales {
            require_nonempty("profile.registry.instrument.locale", locale)?;
            if !locales.insert(locale.as_str()) {
                return Err(invalid(
                    "profile.registry.instrument.locale",
                    format!("duplicate {locale} in {}", instrument.id),
                ));
            }
        }
        let mut admin_modes = BTreeSet::new();
        for admin_mode in &instrument.admin_modes {
            if !matches!(admin_mode.as_str(), "full_scale" | "ema_single_item") {
                return Err(invalid(
                    "profile.registry.instrument.admin_mode",
                    format!("unsupported {admin_mode} in {}", instrument.id),
                ));
            }
            if !admin_modes.insert(admin_mode.as_str()) {
                return Err(invalid(
                    "profile.registry.instrument.admin_mode",
                    format!("duplicate {admin_mode} in {}", instrument.id),
                ));
            }
        }

        let mut item_ids = BTreeSet::new();
        let mut has_negative_item = false;
        for item in &instrument.items {
            require_nonempty("profile.registry.item.id", &item.id)?;
            require_nonempty("profile.registry.item.dimension", &item.dimension)?;
            require_nonempty("profile.registry.item.prompt", &item.prompt)?;
            if !instrument.locales.contains(&item.locale) {
                return Err(invalid(
                    "profile.registry.item.locale",
                    format!("{} is not registered by {}", item.locale, instrument.id),
                ));
            }
            if !matches!(item.keyed.as_str(), "positive" | "negative") {
                return Err(invalid("profile.registry.item.keyed", &item.keyed));
            }
            has_negative_item |= item.keyed == "negative";
            if !item_ids.insert(item.id.as_str()) {
                return Err(invalid(
                    "profile.registry.item.id",
                    format!("duplicate {} in {}", item.id, instrument.id),
                ));
            }
        }
        if has_negative_item
            && instrument
                .scoring
                .reverse_formula
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(invalid(
                "profile.registry.instrument.scoring.reverse_formula",
                format!("{} has negative-keyed items", instrument.id),
            ));
        }
    }
    Ok(instruments)
}

pub fn global_profile_settings(conn: &Connection) -> Result<ProfileSettings> {
    conn.query_row(
        "SELECT enabled, disclosure_acknowledged_at, summary_sharing_enabled,
                paused_until, created_at, updated_at
         FROM profile_settings WHERE id=1",
        [],
        |row| {
            let enabled: i64 = row.get(0)?;
            let disclosure_acknowledged_at: Option<String> = row.get(1)?;
            let summary_sharing_enabled: i64 = row.get(2)?;
            Ok(ProfileSettings {
                version: 1,
                enabled: enabled != 0,
                disclosure_required: disclosure_acknowledged_at.is_none(),
                disclosure_acknowledged_at,
                summary_sharing_enabled: summary_sharing_enabled != 0,
                paused_until: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    )
    .map_err(Into::into)
}

pub fn update_global_profile_settings(
    conn: &Connection,
    update: ProfileSettingsUpdate,
) -> Result<ProfileSettings> {
    if update.clear_pause && update.paused_until.is_some() {
        return Err(invalid(
            "profile.settings.paused_until",
            "cannot set and clear pause in one update",
        ));
    }
    if let Some(value) = update.paused_until.as_deref() {
        validate_utc_timestamp("profile.settings.paused_until", value)?;
    }
    if update.summary_sharing_enabled == Some(true)
        && global_profile_settings(conn)?.disclosure_required
        && !update.acknowledge_disclosure
    {
        return Err(invalid(
            "profile.settings.summary_sharing_enabled",
            "acknowledge the local profile disclosure before enabling integration sharing",
        ));
    }
    conn.execute(
        "UPDATE profile_settings
         SET enabled=COALESCE(?1, enabled),
             disclosure_acknowledged_at=CASE
                 WHEN ?2=1 THEN COALESCE(
                     disclosure_acknowledged_at,
                     strftime('%Y-%m-%dT%H:%M:%SZ','now')
                 )
                 ELSE disclosure_acknowledged_at
             END,
             summary_sharing_enabled=CASE
                 WHEN ?1=0 THEN 0
                 ELSE COALESCE(?3, summary_sharing_enabled)
             END,
             paused_until=CASE
                 WHEN ?5=1 THEN NULL
                 WHEN ?4 IS NOT NULL THEN ?4
                 ELSE paused_until
             END,
             updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE id=1",
        params![
            update.enabled.map(i64::from),
            i64::from(update.acknowledge_disclosure),
            update.summary_sharing_enabled.map(i64::from),
            update.paused_until,
            i64::from(update.clear_pause),
        ],
    )?;
    global_profile_settings(conn)
}

pub fn record_profile_measurement(
    conn: &Connection,
    input: ProfileMeasurementInput,
) -> Result<ProfileMeasurementReceipt> {
    let settings = global_profile_settings(conn)?;
    if !settings.enabled {
        return Ok(skipped_measurement(
            ProfileMeasurementStatus::Disabled,
            "画像已关闭；未记录回答。",
        ));
    }
    if settings.disclosure_required {
        return Ok(skipped_measurement(
            ProfileMeasurementStatus::DisclosureRequired,
            "请先确认本地画像说明；未记录回答。",
        ));
    }
    if let Some(paused_until) = settings.paused_until.as_deref() {
        let paused: i64 = conn.query_row(
            "SELECT CASE WHEN ?1 > strftime('%Y-%m-%dT%H:%M:%SZ','now') THEN 1 ELSE 0 END",
            [paused_until],
            |row| row.get(0),
        )?;
        if paused != 0 {
            return Ok(skipped_measurement(
                ProfileMeasurementStatus::Paused,
                "画像提问已暂停；未记录回答。",
            ));
        }
    }

    require_nonempty("profile.measurement.session_id", &input.session_id)?;
    let instruments = profile_instruments()?;
    let instrument = instruments
        .iter()
        .find(|instrument| instrument.id == input.instrument_id)
        .ok_or_else(|| invalid("profile.measurement.instrument_id", &input.instrument_id))?;
    if instrument.version != input.instrument_version {
        return Err(invalid(
            "profile.measurement.instrument_version",
            format!(
                "{}; expected {}",
                input.instrument_version, instrument.version
            ),
        ));
    }
    if !instrument.locales.contains(&input.locale) {
        return Err(invalid("profile.measurement.locale", &input.locale));
    }
    if !instrument.admin_modes.contains(&input.admin_mode) {
        return Err(invalid("profile.measurement.admin_mode", &input.admin_mode));
    }
    let item = instrument
        .items
        .iter()
        .find(|item| item.id == input.item_id && item.locale == input.locale)
        .ok_or_else(|| invalid("profile.measurement.item_id", &input.item_id))?;
    if !(instrument.scoring.response_min..=instrument.scoring.response_max)
        .contains(&input.response)
    {
        return Err(invalid(
            "profile.measurement.response",
            format!(
                "{}; expected {}..={}",
                input.response, instrument.scoring.response_min, instrument.scoring.response_max
            ),
        ));
    }

    let event_id = Uuid::new_v4().to_string();
    let recorded_at = now_iso(conn)?;
    let payload = serde_json::json!({
        "schema_version": 1,
        "instrument_id": instrument.id,
        "instrument_version": instrument.version,
        "item_id": item.id,
        "locale": item.locale,
        "admin_mode": input.admin_mode,
        "dimension": item.dimension,
        "keyed": item.keyed,
        "response": input.response,
        "license": instrument.license.name,
        "effect": "recorded_only"
    });
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
        params![
            event_id,
            input.session_id,
            recorded_at,
            PROFILE_EVENT_TYPE,
            serde_json::to_string(&payload)?,
        ],
    )?;

    Ok(ProfileMeasurementReceipt {
        status: ProfileMeasurementStatus::Recorded,
        event_id: Some(event_id),
        recorded_at: Some(recorded_at),
        message: "回答仅保存在本机画像事件中，不改变掌握度。".to_owned(),
    })
}

pub fn store_profile_dimension(
    conn: &Connection,
    input: ProfileDimensionInput,
) -> Result<ProfileDimension> {
    let scope_id = validate_dimension_input(&input)?;
    let updated_at = now_iso(conn)?;
    conn.execute(
        "INSERT INTO profile_dimensions(
             scope, scope_id, dimension_key, mean, variance, evidence_count,
             model_version, gate_status, provenance_json, evidence_ids_json, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(scope, scope_id, dimension_key) DO UPDATE SET
             mean=excluded.mean,
             variance=excluded.variance,
             evidence_count=excluded.evidence_count,
             model_version=excluded.model_version,
             gate_status=excluded.gate_status,
             provenance_json=excluded.provenance_json,
             evidence_ids_json=excluded.evidence_ids_json,
             updated_at=excluded.updated_at",
        params![
            input.scope.as_str(),
            scope_id,
            input.dimension_key,
            input.mean,
            input.variance,
            input.evidence_count,
            input.model_version,
            input.gate_status.as_str(),
            serde_json::to_string(&input.provenance)?,
            serde_json::to_string(&input.evidence_ids)?,
            updated_at,
        ],
    )?;
    Ok(ProfileDimension {
        scope: input.scope,
        scope_id: empty_to_none(scope_id),
        dimension_key: input.dimension_key,
        mean: input.mean,
        variance: input.variance,
        evidence_count: input.evidence_count,
        model_version: input.model_version,
        gate_status: input.gate_status,
        provenance: input.provenance,
        evidence_ids: input.evidence_ids,
        updated_at,
    })
}

pub fn record_profile_validation_run(
    conn: &Connection,
    input: ProfileValidationRunInput,
) -> Result<ProfileValidationRun> {
    require_nonempty("profile.validation.id", &input.id)?;
    require_nonempty("profile.validation.dimension_key", &input.dimension_key)?;
    require_nonempty("profile.validation.model_version", &input.model_version)?;
    require_json_object("profile.validation.metrics", &input.metrics)?;
    require_json_object("profile.validation.provenance", &input.provenance)?;
    reject_raw_answer_metadata("profile.validation.metrics", &input.metrics)?;
    reject_raw_answer_metadata("profile.validation.provenance", &input.provenance)?;
    validate_evidence_ids("profile.validation.evidence_ids", &input.evidence_ids)?;
    let scope_id = normalized_scope_id(input.scope, input.scope_id.as_deref())?;
    let ran_at = now_iso(conn)?;
    conn.execute(
        "INSERT INTO profile_validation_runs(
             id, scope, scope_id, dimension_key, model_version, status,
             metrics_json, provenance_json, evidence_ids_json, ran_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            input.id,
            input.scope.as_str(),
            scope_id,
            input.dimension_key,
            input.model_version,
            input.status.as_str(),
            serde_json::to_string(&input.metrics)?,
            serde_json::to_string(&input.provenance)?,
            serde_json::to_string(&input.evidence_ids)?,
            ran_at,
        ],
    )?;
    Ok(ProfileValidationRun {
        id: input.id,
        scope: input.scope,
        scope_id: empty_to_none(scope_id),
        dimension_key: input.dimension_key,
        model_version: input.model_version,
        status: input.status,
        metrics: input.metrics,
        provenance: input.provenance,
        evidence_ids: input.evidence_ids,
        ran_at,
    })
}

pub fn export_global_profile(conn: &Connection) -> Result<GlobalProfileExport> {
    Ok(GlobalProfileExport {
        schema_version: PROFILE_EXPORT_SCHEMA_VERSION,
        exported_at: now_iso(conn)?,
        settings: global_profile_settings(conn)?,
        instruments: profile_instruments()?,
        measurements: load_profile_measurements(conn)?,
        dimensions: load_profile_dimensions(conn)?,
        validation_runs: load_profile_validation_runs(conn)?,
        data_actions: load_profile_data_actions(conn)?,
    })
}

pub fn global_profile_overview(conn: &Connection) -> Result<GlobalProfileOverview> {
    let measurement_count = conn.query_row(
        "SELECT COUNT(*) FROM behavior_events WHERE type=?1",
        [PROFILE_EVENT_TYPE],
        |row| row.get(0),
    )?;
    Ok(GlobalProfileOverview {
        generated_at: now_iso(conn)?,
        settings: global_profile_settings(conn)?,
        instrument_count: profile_instruments()?.len(),
        measurement_count,
        dimensions: load_profile_dimensions(conn)?,
        validation_runs: load_profile_validation_runs(conn)?,
        data_actions: load_profile_data_actions(conn)?,
        notice: "原始回答只在用户显式导出时读取；概览不含回答内容。".to_owned(),
    })
}

pub fn global_profile_integration_summary(
    conn: &Connection,
) -> Result<GlobalProfileIntegrationSummary> {
    let settings = global_profile_settings(conn)?;
    if !settings.enabled || !settings.summary_sharing_enabled {
        return Err(PolarisError::ProfileSummarySharingDisabled);
    }
    Ok(GlobalProfileIntegrationSummary {
        generated_at: now_iso(conn)?,
        dimensions: load_profile_dimensions(conn)?,
        notice: "本摘要不含原始回答；非 active 维度不得用于调度或干预。".to_owned(),
    })
}

pub fn reset_global_profile(conn: &Connection) -> Result<ProfileResetReceipt> {
    let tx = conn.unchecked_transaction()?;
    let measurements_deleted = tx.execute(
        "DELETE FROM behavior_events WHERE type=?1",
        [PROFILE_EVENT_TYPE],
    )? as i64;
    let dimensions_deleted = tx.execute("DELETE FROM profile_dimensions", [])? as i64;
    let validation_runs_deleted = tx.execute("DELETE FROM profile_validation_runs", [])? as i64;
    let learning_attempts_preserved: i64 =
        tx.query_row("SELECT COUNT(*) FROM attempts", [], |row| row.get(0))?;
    let action_id = Uuid::new_v4().to_string();
    let at = now_iso(&tx)?;
    tx.execute(
        "INSERT INTO profile_data_actions(
             id, action, measurements_deleted, dimensions_deleted,
             validation_runs_deleted, at
         ) VALUES (?1, 'profile_reset', ?2, ?3, ?4, ?5)",
        params![
            action_id,
            measurements_deleted,
            dimensions_deleted,
            validation_runs_deleted,
            at,
        ],
    )?;
    tx.commit()?;
    Ok(ProfileResetReceipt {
        action_id,
        at,
        measurements_deleted,
        dimensions_deleted,
        validation_runs_deleted,
        learning_attempts_preserved,
    })
}

pub fn delete_all_learning_data<F>(
    request: FullDataDeletionRequest,
    clear_local_secrets: F,
) -> Result<FullDataDeletionReceipt>
where
    F: FnOnce() -> Result<usize>,
{
    delete_all_learning_data_with_remover(request, clear_local_secrets, |path| {
        std::fs::remove_file(path)
    })
}

fn delete_all_learning_data_with_remover<F, R>(
    request: FullDataDeletionRequest,
    clear_local_secrets: F,
    mut remove_old_file: R,
) -> Result<FullDataDeletionReceipt>
where
    F: FnOnce() -> Result<usize>,
    R: FnMut(&Path) -> std::io::Result<()>,
{
    if request.confirmation != DELETE_ALL_CONFIRMATION {
        return Err(invalid(
            "profile.delete_all.confirmation",
            format!("must exactly match {DELETE_ALL_CONFIRMATION}"),
        ));
    }
    if !request.database_path.is_file() {
        return Err(invalid(
            "profile.delete_all.database_path",
            format!(
                "database does not exist: {}",
                request.database_path.display()
            ),
        ));
    }
    let deleted_at = sqlite_now()?;
    if let Some(backup_path) = request.backup_path.as_deref() {
        if backup_path.exists() {
            return Err(invalid(
                "profile.delete_all.backup_path",
                format!(
                    "backup target must be a new path: {}",
                    backup_path.display()
                ),
            ));
        }
        validate_backup_path(&request.database_path, backup_path)?;
        backup_database_file(&request.database_path, backup_path)?;
    }

    let recovery_database_path = recovery_database_path(&request.database_path);
    if let Err(error) = backup_database_file(&request.database_path, &recovery_database_path) {
        cleanup_sqlite_family(&recovery_database_path);
        return Err(error);
    }
    let fresh_database_path = fresh_database_path(&request.database_path);
    let fresh_conn = match crate::db::open_database(&fresh_database_path) {
        Ok(conn) => conn,
        Err(error) => {
            cleanup_sqlite_family(&recovery_database_path);
            return Err(error);
        }
    };
    drop(fresh_conn);

    let mut targets = vec![request.database_path.clone()];
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = sqlite_sidecar_path(&request.database_path, suffix);
        if path.is_file() {
            targets.push(path);
        }
    }
    let mut quarantined = Vec::with_capacity(targets.len());
    for (index, original) in targets.iter().enumerate() {
        let quarantine = quarantine_path(original, index);
        if let Err(error) = std::fs::rename(original, &quarantine) {
            rollback_quarantine(&quarantined);
            cleanup_sqlite_family(&fresh_database_path);
            cleanup_sqlite_family(&recovery_database_path);
            return Err(error.into());
        }
        quarantined.push((original.clone(), quarantine));
    }

    let local_secrets_deleted = match clear_local_secrets() {
        Ok(count) => count,
        Err(error) => {
            rollback_quarantine(&quarantined);
            cleanup_sqlite_family(&fresh_database_path);
            cleanup_sqlite_family(&recovery_database_path);
            return Err(error);
        }
    };

    if let Err(error) = std::fs::rename(&fresh_database_path, &request.database_path) {
        rollback_quarantine(&quarantined);
        cleanup_sqlite_family(&fresh_database_path);
        cleanup_sqlite_family(&recovery_database_path);
        return Err(error.into());
    }

    let files_deleted = quarantined.len();
    quarantined.sort_by_key(|(original, _)| usize::from(original == &request.database_path));
    for (_, quarantine) in &quarantined {
        if let Err(error) = remove_old_file(quarantine) {
            let restore = restore_recovery_database(
                &request.database_path,
                &fresh_database_path,
                &recovery_database_path,
            );
            for (_, leftover) in &quarantined {
                let _ = std::fs::remove_file(leftover);
            }
            return match restore {
                Ok(()) => Err(error.into()),
                Err(restore_error) => Err(invalid(
                    "profile.delete_all.recovery",
                    format!(
                        "old-file cleanup failed: {error}; logical database restore failed: \
                         {restore_error}; recovery snapshot: {}",
                        recovery_database_path.display()
                    ),
                )),
            };
        }
    }
    if let Err(error) = std::fs::remove_file(&recovery_database_path) {
        let restore = restore_recovery_database(
            &request.database_path,
            &fresh_database_path,
            &recovery_database_path,
        );
        return match restore {
            Ok(()) => Err(error.into()),
            Err(restore_error) => Err(invalid(
                "profile.delete_all.recovery",
                format!(
                    "recovery snapshot cleanup failed: {error}; logical database restore failed: \
                     {restore_error}; recovery snapshot: {}",
                    recovery_database_path.display()
                ),
            )),
        };
    }

    Ok(FullDataDeletionReceipt {
        deleted_at,
        database_path: request.database_path.display().to_string(),
        backup_path: request.backup_path.map(|path| path.display().to_string()),
        files_deleted,
        local_secrets_deleted,
        empty_database_created: true,
    })
}

fn validate_dimension_input(input: &ProfileDimensionInput) -> Result<String> {
    require_nonempty("profile.dimension.dimension_key", &input.dimension_key)?;
    require_nonempty("profile.dimension.model_version", &input.model_version)?;
    if !input.mean.is_finite() {
        return Err(invalid("profile.dimension.mean", input.mean));
    }
    if !input.variance.is_finite() || input.variance < 0.0 {
        return Err(invalid("profile.dimension.variance", input.variance));
    }
    if input.evidence_count < 0 {
        return Err(invalid(
            "profile.dimension.evidence_count",
            input.evidence_count,
        ));
    }
    require_json_object("profile.dimension.provenance", &input.provenance)?;
    reject_raw_answer_metadata("profile.dimension.provenance", &input.provenance)?;
    validate_evidence_ids("profile.dimension.evidence_ids", &input.evidence_ids)?;
    normalized_scope_id(input.scope, input.scope_id.as_deref())
}

fn normalized_scope_id(scope: ProfileScope, scope_id: Option<&str>) -> Result<String> {
    let scope_id = scope_id.unwrap_or_default().trim();
    match scope {
        ProfileScope::Global if scope_id.is_empty() => Ok(String::new()),
        ProfileScope::Global => Err(invalid(
            "profile.scope_id",
            "global scope must not have scope_id",
        )),
        ProfileScope::Pack | ProfileScope::Goal if !scope_id.is_empty() => Ok(scope_id.to_owned()),
        ProfileScope::Pack | ProfileScope::Goal => Err(invalid(
            "profile.scope_id",
            "pack and goal scopes require scope_id",
        )),
    }
}

fn load_profile_measurements(conn: &Connection) -> Result<Vec<ProfileMeasurementExport>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, at, payload_json
         FROM behavior_events WHERE type=?1 ORDER BY at, id",
    )?;
    let rows = stmt
        .query_map([PROFILE_EVENT_TYPE], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(event_id, session_id, recorded_at, payload)| {
            Ok(ProfileMeasurementExport {
                event_id,
                session_id,
                recorded_at,
                payload: serde_json::from_str(&payload)?,
            })
        })
        .collect()
}

fn load_profile_dimensions(conn: &Connection) -> Result<Vec<ProfileDimension>> {
    let mut stmt = conn.prepare(
        "SELECT scope, scope_id, dimension_key, mean, variance, evidence_count,
                model_version, gate_status, provenance_json, evidence_ids_json, updated_at
         FROM profile_dimensions ORDER BY scope, scope_id, dimension_key",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(
                scope,
                scope_id,
                dimension_key,
                mean,
                variance,
                evidence_count,
                model_version,
                gate_status,
                provenance,
                evidence_ids,
                updated_at,
            )| {
                Ok(ProfileDimension {
                    scope: ProfileScope::parse(&scope)?,
                    scope_id: empty_to_none(scope_id),
                    dimension_key,
                    mean,
                    variance,
                    evidence_count,
                    model_version,
                    gate_status: ProfileGateStatus::parse(&gate_status)?,
                    provenance: serde_json::from_str(&provenance)?,
                    evidence_ids: serde_json::from_str(&evidence_ids)?,
                    updated_at,
                })
            },
        )
        .collect()
}

fn load_profile_validation_runs(conn: &Connection) -> Result<Vec<ProfileValidationRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, scope, scope_id, dimension_key, model_version, status,
                metrics_json, provenance_json, evidence_ids_json, ran_at
         FROM profile_validation_runs ORDER BY ran_at, id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(
                id,
                scope,
                scope_id,
                dimension_key,
                model_version,
                status,
                metrics,
                provenance,
                evidence_ids,
                ran_at,
            )| {
                Ok(ProfileValidationRun {
                    id,
                    scope: ProfileScope::parse(&scope)?,
                    scope_id: empty_to_none(scope_id),
                    dimension_key,
                    model_version,
                    status: ProfileGateStatus::parse(&status)?,
                    metrics: serde_json::from_str(&metrics)?,
                    provenance: serde_json::from_str(&provenance)?,
                    evidence_ids: serde_json::from_str(&evidence_ids)?,
                    ran_at,
                })
            },
        )
        .collect()
}

fn load_profile_data_actions(conn: &Connection) -> Result<Vec<ProfileDataAction>> {
    let mut stmt = conn.prepare(
        "SELECT id, action, measurements_deleted, dimensions_deleted,
                validation_runs_deleted, at
         FROM profile_data_actions ORDER BY at, id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ProfileDataAction {
                id: row.get(0)?,
                action: row.get(1)?,
                measurements_deleted: row.get(2)?,
                dimensions_deleted: row.get(3)?,
                validation_runs_deleted: row.get(4)?,
                at: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn skipped_measurement(
    status: ProfileMeasurementStatus,
    message: &str,
) -> ProfileMeasurementReceipt {
    ProfileMeasurementReceipt {
        status,
        event_id: None,
        recorded_at: None,
        message: message.to_owned(),
    }
}

fn require_nonempty(key: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(invalid(key, "must not be blank"))
    } else {
        Ok(())
    }
}

fn require_https(key: &str, value: &str) -> Result<()> {
    if value.starts_with("https://") {
        Ok(())
    } else {
        Err(invalid(key, "must use https"))
    }
}

fn require_json_object(key: &str, value: &Value) -> Result<()> {
    if value.is_object() {
        Ok(())
    } else {
        Err(invalid(key, "must be a JSON object"))
    }
}

fn reject_raw_answer_metadata(key: &str, value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (field, child) in object {
                let normalized = field.to_ascii_lowercase().replace('-', "_");
                if matches!(
                    normalized.as_str(),
                    "answer"
                        | "raw_answer"
                        | "raw_response"
                        | "response"
                        | "response_text"
                        | "item_prompt"
                ) {
                    return Err(invalid(
                        key,
                        format!("raw answer field is forbidden: {field}"),
                    ));
                }
                reject_raw_answer_metadata(key, child)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for child in values {
                reject_raw_answer_metadata(key, child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_evidence_ids(key: &str, evidence_ids: &[String]) -> Result<()> {
    for evidence_id in evidence_ids {
        let valid = !evidence_id.is_empty()
            && evidence_id.len() <= 128
            && evidence_id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            });
        if !valid {
            return Err(invalid(key, format!("invalid evidence id: {evidence_id}")));
        }
    }
    Ok(())
}

fn validate_utc_timestamp(key: &str, value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
    let looks_valid = bytes.len() == 20
        && bytes[19] == b'Z'
        && separators
            .iter()
            .all(|(index, expected)| bytes[*index] == *expected)
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        });
    if looks_valid {
        Ok(())
    } else {
        Err(invalid(key, "expected UTC timestamp YYYY-MM-DDTHH:MM:SSZ"))
    }
}

fn now_iso(conn: &Connection) -> Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn sqlite_now() -> Result<String> {
    let conn = Connection::open_in_memory()?;
    now_iso(&conn)
}

fn backup_database_file(database_path: &Path, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let found = schema_version(&conn)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(PolarisError::UnsupportedSchemaVersion {
            found,
            current: CURRENT_SCHEMA_VERSION,
        });
    }
    if let Err(error) = conn.execute("VACUUM INTO ?1", [output.to_string_lossy().to_string()]) {
        cleanup_sqlite_family(output);
        return Err(error.into());
    }
    Ok(())
}

fn validate_backup_path(database_path: &Path, backup_path: &Path) -> Result<()> {
    let database_path = std::path::absolute(database_path)?;
    let backup_path = std::path::absolute(backup_path)?;
    let conflicts = std::iter::once(database_path.clone()).chain(
        ["-wal", "-shm", "-journal"]
            .into_iter()
            .map(|suffix| sqlite_sidecar_path(&database_path, suffix)),
    );
    if conflicts.into_iter().any(|path| path == backup_path) {
        return Err(invalid(
            "profile.delete_all.backup_path",
            "backup target must not overlap the SQLite database family",
        ));
    }
    Ok(())
}

fn sqlite_sidecar_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut value = database_path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn quarantine_path(original: &Path, index: usize) -> PathBuf {
    let mut value = original.as_os_str().to_os_string();
    value.push(format!(".polaris-delete-{}-{index}", Uuid::new_v4()));
    PathBuf::from(value)
}

fn fresh_database_path(database_path: &Path) -> PathBuf {
    let mut value = database_path.as_os_str().to_os_string();
    value.push(format!(".polaris-empty-{}", Uuid::new_v4()));
    PathBuf::from(value)
}

fn recovery_database_path(database_path: &Path) -> PathBuf {
    let mut value = database_path.as_os_str().to_os_string();
    value.push(format!(".polaris-recovery-{}", Uuid::new_v4()));
    PathBuf::from(value)
}

fn restore_recovery_database(
    database_path: &Path,
    fresh_database_path: &Path,
    recovery_database_path: &Path,
) -> Result<()> {
    cleanup_sqlite_family(fresh_database_path);
    if database_path.exists() {
        std::fs::rename(database_path, fresh_database_path)?;
    }
    if let Err(error) = std::fs::rename(recovery_database_path, database_path) {
        if fresh_database_path.exists() && !database_path.exists() {
            let _ = std::fs::rename(fresh_database_path, database_path);
        }
        return Err(error.into());
    }
    cleanup_sqlite_family(fresh_database_path);
    Ok(())
}

fn cleanup_sqlite_family(database_path: &Path) {
    let _ = std::fs::remove_file(database_path);
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(sqlite_sidecar_path(database_path, suffix));
    }
}

fn rollback_quarantine(paths: &[(PathBuf, PathBuf)]) {
    for (original, quarantine) in paths.iter().rev() {
        if quarantine.exists() && !original.exists() {
            let _ = std::fs::rename(quarantine, original);
        }
    }
}

fn empty_to_none(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn invalid(key: &str, value: impl ToString) -> PolarisError {
    PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_file_cleanup_failure_restores_logical_database_from_recovery_snapshot() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let database_path =
            std::env::temp_dir().join(format!("polaris-profile-delete-recovery-{suffix}.sqlite"));
        let conn = crate::db::open_database(&database_path).unwrap();
        conn.execute("UPDATE meta SET value='0.42' WHERE key='bkt.p_init'", [])
            .unwrap();
        drop(conn);

        let mut first_remove = true;
        let error = delete_all_learning_data_with_remover(
            FullDataDeletionRequest {
                database_path: database_path.clone(),
                backup_path: None,
                confirmation: DELETE_ALL_CONFIRMATION.to_owned(),
            },
            || Ok(0),
            |path| {
                if first_remove {
                    first_remove = false;
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "simulated old-file cleanup failure",
                    ))
                } else {
                    std::fs::remove_file(path)
                }
            },
        )
        .unwrap_err();

        assert!(matches!(error, PolarisError::Io(_)));
        let restored = crate::db::open_database(&database_path).unwrap();
        let p_init: String = restored
            .query_row("SELECT value FROM meta WHERE key='bkt.p_init'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(p_init, "0.42");
        drop(restored);
        cleanup_sqlite_family(&database_path);
    }
}
