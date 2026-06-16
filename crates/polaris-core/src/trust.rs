use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use crate::config::{default_registry, meta_f64, meta_i64, meta_value, ParameterClass};
use crate::error::Result;
use crate::ops::{doctor_diagnostics, ActivitySummary};
use crate::pack_state::active_pack;

const TRUST_WINDOW_DAYS: i64 = 7;
const ACTIVE_LIMIT: i64 = 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrustPanel {
    pub generated_at: String,
    pub window_days: i64,
    pub gates: Vec<TrustGateStatus>,
    pub active_breeding_experiments: Vec<TrustBreedingExperiment>,
    pub active_mrt_experiments: Vec<TrustMrtExperiment>,
    pub recent_activity: TrustRecentActivity,
    pub governance: TrustGovernance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustGateStatus {
    pub framework: String,
    pub name: String,
    pub status: String,
    pub gate: String,
    pub metric: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrustBreedingExperiment {
    pub id: String,
    pub candidate_move: String,
    pub incumbent_move: String,
    pub context_hash: String,
    pub task_type: String,
    pub status: String,
    pub posterior_win_prob: f64,
    pub n_candidate: i64,
    pub n_incumbent: i64,
    pub admit_p: f64,
    pub retire_p: f64,
    pub min_n: i64,
    pub main_effect_hypothesis: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustMrtExperiment {
    pub id: String,
    pub at: String,
    pub move_id: String,
    pub randomized: bool,
    pub prereg_id: String,
    pub context_hash: Option<String>,
    pub window: Option<String>,
    pub main_effect_hypothesis: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustRecentActivity {
    pub window_days: i64,
    pub param_tuning_runs: ActivitySummary,
    pub breeding_evaluated_7d: ActivitySummary,
    pub breeding_admitted_7d: ActivitySummary,
    pub breeding_retired_7d: ActivitySummary,
    pub mental_fit_hazard: ActivitySummary,
    pub mental_fit_state_gate: ActivitySummary,
    pub gu_inductions: ActivitySummary,
    pub nightly_consolidation: ActivitySummary,
    pub mirror_reports: ActivitySummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustGovernance {
    pub current_pack_id: Option<String>,
    pub breeding_admit_p: TrustParameter,
    pub breeding_retire_p: TrustParameter,
    pub breeding_min_n: TrustParameter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustParameter {
    pub key: String,
    pub current_value: String,
    pub default_value: String,
    pub class: String,
    pub bounds: Option<String>,
    pub tuning_route: String,
    pub is_governance_gate: bool,
}

pub fn trust_panel(conn: &Connection) -> Result<TrustPanel> {
    let generated_at =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
            row.get(0)
        })?;
    let diagnostics = doctor_diagnostics(conn, TRUST_WINDOW_DAYS)?;

    Ok(TrustPanel {
        generated_at,
        window_days: TRUST_WINDOW_DAYS,
        gates: gate_statuses(conn)?,
        active_breeding_experiments: active_breeding_experiments(conn)?,
        active_mrt_experiments: active_mrt_experiments(conn)?,
        recent_activity: TrustRecentActivity {
            window_days: diagnostics.window_days,
            param_tuning_runs: diagnostics.param_tuning_runs,
            breeding_evaluated_7d: diagnostics.breeding_evaluated_7d,
            breeding_admitted_7d: diagnostics.breeding_admitted_7d,
            breeding_retired_7d: diagnostics.breeding_retired_7d,
            mental_fit_hazard: diagnostics.mental_fit_hazard,
            mental_fit_state_gate: diagnostics.mental_fit_state_gate,
            gu_inductions: diagnostics.gu_inductions,
            nightly_consolidation: diagnostics.consolidation_runs,
            mirror_reports: diagnostics.mirror_reports,
        },
        governance: governance(conn)?,
    })
}

fn gate_statuses(conn: &Connection) -> Result<Vec<TrustGateStatus>> {
    Ok(vec![
        f1_signature_gate(conn)?,
        f2_phase_gate(conn)?,
        f3_friction_gate(conn)?,
        f4_gu_gate(conn)?,
        f5_breeding_gate(conn)?,
    ])
}

fn f1_signature_gate(conn: &Connection) -> Result<TrustGateStatus> {
    let (effect_rows, effect_samples) = move_effect_stats(conn)?;
    let mrt_preregistrations = teaching_mrt_preregistration_count(conn)?;
    let metric = Some(format!(
        "effect_samples={effect_samples}, effect_rows={effect_rows}, mrt_preregistrations={mrt_preregistrations}"
    ));
    if effect_samples > 0 {
        Ok(TrustGateStatus {
            framework: "F1".to_owned(),
            name: "pedagogy_signature".to_owned(),
            status: "fitted".to_owned(),
            gate: "evidence_visible".to_owned(),
            metric,
            reason: "move effects and MRT preregistrations are auditable; no separate holdout pass is claimed".to_owned(),
        })
    } else if mrt_preregistrations > 0 {
        Ok(TrustGateStatus {
            framework: "F1".to_owned(),
            name: "pedagogy_signature".to_owned(),
            status: "running".to_owned(),
            gate: "evidence_visible".to_owned(),
            metric,
            reason:
                "teaching MRT preregistrations are visible, but no move-effect samples exist yet"
                    .to_owned(),
        })
    } else {
        Ok(no_data_gate(
            "F1",
            "pedagogy_signature",
            "no moves_effects or MRT preregistrations yet",
        ))
    }
}

fn f2_phase_gate(conn: &Connection) -> Result<TrustGateStatus> {
    let concept_count = count_where(conn, "concepts", "1=1")?;
    Ok(TrustGateStatus {
        framework: "F2".to_owned(),
        name: "phase_diagram".to_owned(),
        status: "available".to_owned(),
        gate: "deterministic_rule".to_owned(),
        metric: Some(format!("concepts={concept_count}")),
        reason:
            "Tier 0 phase classification is deterministic and visible; no validation AUC is fabricated"
                .to_owned(),
    })
}

fn f3_friction_gate(conn: &Connection) -> Result<TrustGateStatus> {
    let friction_effect_rows =
        count_where(conn, "moves_effects", "context_hash LIKE 'state:%|phase:%'")?;
    let signature_mrt_rows = count_where(
        conn,
        "mrt_log",
        "json_valid(context_json)
           AND json_extract(context_json, '$.kind')='preregistration'
           AND json_extract(context_json, '$.selected_by')='signature_friction'",
    )?;
    if friction_effect_rows > 0 {
        Ok(TrustGateStatus {
            framework: "F3".to_owned(),
            name: "friction_curve".to_owned(),
            status: "fitted".to_owned(),
            gate: "evidence_visible".to_owned(),
            metric: Some(format!(
                "friction_effect_rows={friction_effect_rows}, signature_mrt_rows={signature_mrt_rows}"
            )),
            reason: "friction-aware contexts are present in auditable effect or MRT data".to_owned(),
        })
    } else if signature_mrt_rows > 0 {
        Ok(TrustGateStatus {
            framework: "F3".to_owned(),
            name: "friction_curve".to_owned(),
            status: "running".to_owned(),
            gate: "evidence_visible".to_owned(),
            metric: Some(format!(
                "friction_effect_rows={friction_effect_rows}, signature_mrt_rows={signature_mrt_rows}"
            )),
            reason:
                "signature-friction MRT preregistrations are visible, but no friction effect rows exist yet"
                    .to_owned(),
        })
    } else {
        Ok(no_data_gate(
            "F3",
            "friction_curve",
            "no friction-context move effects or signature MRT rows yet",
        ))
    }
}

fn f4_gu_gate(conn: &Connection) -> Result<TrustGateStatus> {
    let active = count_where(conn, "gu_rules", "status='active'")?;
    let validated = count_where(conn, "gu_rules", "status='validated'")?;
    let candidate = count_where(conn, "gu_rules", "status='candidate'")?;
    if active + validated > 0 {
        Ok(TrustGateStatus {
            framework: "F4".to_owned(),
            name: "g_u_rules".to_owned(),
            status: "active".to_owned(),
            gate: "rules_visible".to_owned(),
            metric: Some(format!(
                "active={active}, validated={validated}, candidate={candidate}"
            )),
            reason: "validated or active learner-specific G_u rules are visible".to_owned(),
        })
    } else if candidate > 0 {
        Ok(TrustGateStatus {
            framework: "F4".to_owned(),
            name: "g_u_rules".to_owned(),
            status: "running".to_owned(),
            gate: "not_passed".to_owned(),
            metric: Some(format!(
                "active={active}, validated={validated}, candidate={candidate}"
            )),
            reason: "candidate G_u rules exist but none are validated or active".to_owned(),
        })
    } else {
        Ok(no_data_gate(
            "F4",
            "g_u_rules",
            "no candidate, validated, or active G_u rules yet",
        ))
    }
}

fn f5_breeding_gate(conn: &Connection) -> Result<TrustGateStatus> {
    let preregistered = count_where(conn, "bred_moves", "status='preregistered'")?;
    let admitted = count_where(conn, "bred_moves", "status='admitted'")?;
    let retired = count_where(conn, "bred_moves", "status='retired'")?;
    if admitted > 0 {
        Ok(TrustGateStatus {
            framework: "F5".to_owned(),
            name: "breeding".to_owned(),
            status: "passed".to_owned(),
            gate: "admitted".to_owned(),
            metric: Some(format!(
                "preregistered={preregistered}, admitted={admitted}, retired={retired}"
            )),
            reason: format!(
                "admitted={admitted}; preregistered={preregistered}; retired={retired}"
            ),
        })
    } else if preregistered > 0 {
        Ok(TrustGateStatus {
            framework: "F5".to_owned(),
            name: "breeding".to_owned(),
            status: "running".to_owned(),
            gate: "not_passed".to_owned(),
            metric: Some(format!(
                "preregistered={preregistered}, admitted={admitted}, retired={retired}"
            )),
            reason: format!(
                "preregistered={preregistered}; admitted={admitted}; retired={retired}"
            ),
        })
    } else if retired > 0 {
        Ok(TrustGateStatus {
            framework: "F5".to_owned(),
            name: "breeding".to_owned(),
            status: "retired".to_owned(),
            gate: "not_passed".to_owned(),
            metric: Some(format!(
                "preregistered={preregistered}, admitted={admitted}, retired={retired}"
            )),
            reason: format!("retired={retired}; no admitted bred moves remain visible"),
        })
    } else {
        Ok(no_data_gate(
            "F5",
            "breeding",
            "no bred move preregistrations yet",
        ))
    }
}

fn active_breeding_experiments(conn: &Connection) -> Result<Vec<TrustBreedingExperiment>> {
    let fallback_admit_p = meta_f64(conn, "breeding.admit_p")?;
    let fallback_retire_p = meta_f64(conn, "breeding.retire_p")?;
    let fallback_min_n = meta_i64(conn, "breeding.min_n")?;
    let mut stmt = conn.prepare(
        "SELECT id, candidate_move, incumbent_move, context_hash, task_type, status,
                posterior_win_prob, n_candidate, n_incumbent, main_effect_hypothesis,
                prereg_json, created_at, updated_at
         FROM bred_moves
         WHERE status='preregistered'
         ORDER BY julianday(updated_at) DESC, id DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([ACTIVE_LIMIT], |row| {
        let prereg_json: String = row.get(10)?;
        let prereg = parse_json(&prereg_json);
        Ok(TrustBreedingExperiment {
            id: row.get(0)?,
            candidate_move: row.get(1)?,
            incumbent_move: row.get(2)?,
            context_hash: row.get(3)?,
            task_type: row.get(4)?,
            status: row.get(5)?,
            posterior_win_prob: row.get(6)?,
            n_candidate: row.get(7)?,
            n_incumbent: row.get(8)?,
            main_effect_hypothesis: row.get(9)?,
            admit_p: json_f64(prereg.as_ref(), "admit_p").unwrap_or(fallback_admit_p),
            retire_p: json_f64(prereg.as_ref(), "retire_p").unwrap_or(fallback_retire_p),
            min_n: json_i64(prereg.as_ref(), "min_n").unwrap_or(fallback_min_n),
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn active_mrt_experiments(conn: &Connection) -> Result<Vec<TrustMrtExperiment>> {
    let mut stmt = conn.prepare(
        "SELECT id, at, context_json, randomized, move, prereg_id
         FROM mrt_log
         WHERE prereg_id IS NOT NULL
           AND json_valid(context_json)
           AND json_extract(context_json, '$.kind')='preregistration'
           AND julianday(at) >= julianday('now') - ?1
         ORDER BY julianday(at) DESC, id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map([TRUST_WINDOW_DAYS, ACTIVE_LIMIT], |row| {
        let context_json: String = row.get(2)?;
        let context = parse_json(&context_json);
        let randomized: i64 = row.get(3)?;
        Ok(TrustMrtExperiment {
            id: row.get(0)?,
            at: row.get(1)?,
            move_id: row.get(4)?,
            randomized: randomized != 0,
            prereg_id: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            context_hash: json_string(context.as_ref(), "context_hash"),
            window: json_string(context.as_ref(), "window"),
            main_effect_hypothesis: json_string(context.as_ref(), "main_effect_hypothesis"),
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn governance(conn: &Connection) -> Result<TrustGovernance> {
    Ok(TrustGovernance {
        current_pack_id: active_pack(conn)?,
        breeding_admit_p: trust_parameter(conn, "breeding.admit_p")?,
        breeding_retire_p: trust_parameter(conn, "breeding.retire_p")?,
        breeding_min_n: trust_parameter(conn, "breeding.min_n")?,
    })
}

fn trust_parameter(conn: &Connection, key: &str) -> Result<TrustParameter> {
    let registry = default_registry();
    let spec = registry
        .get(key)
        .expect("trust parameters must be registered");
    Ok(TrustParameter {
        key: spec.key.to_owned(),
        current_value: meta_value(conn, key)?,
        default_value: spec.default_value.to_owned(),
        class: spec.class.as_str().to_owned(),
        bounds: spec.bounds.map(str::to_owned),
        tuning_route: spec.tuning_route.as_str().to_owned(),
        is_governance_gate: spec.class == ParameterClass::A,
    })
}

fn move_effect_stats(conn: &Connection) -> Result<(i64, i64)> {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(n), 0) FROM moves_effects",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(Into::into)
}

fn count_where(conn: &Connection, table: &str, where_clause: &str) -> Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {where_clause}");
    conn.query_row(&sql, [], |row| row.get(0))
        .map_err(Into::into)
}

fn teaching_mrt_preregistration_count(conn: &Connection) -> Result<i64> {
    count_where(
        conn,
        "mrt_log",
        "json_valid(context_json)
           AND json_extract(context_json, '$.kind')='preregistration'",
    )
}

fn no_data_gate(framework: &str, name: &str, reason: &str) -> TrustGateStatus {
    TrustGateStatus {
        framework: framework.to_owned(),
        name: name.to_owned(),
        status: "unfit".to_owned(),
        gate: "no_data".to_owned(),
        metric: None,
        reason: reason.to_owned(),
    }
}

fn parse_json(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}

fn json_string(value: Option<&Value>, key: &str) -> Option<String> {
    value?.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn json_i64(value: Option<&Value>, key: &str) -> Option<i64> {
    value?.get(key).and_then(Value::as_i64)
}

fn json_f64(value: Option<&Value>, key: &str) -> Option<f64> {
    value?.get(key).and_then(Value::as_f64)
}
