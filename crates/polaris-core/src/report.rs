use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::calibration::{
    calibration_samples, posterior_from_samples, prob_beta_greater, prob_beta_greater_half,
    regularized_incomplete_beta,
};
use crate::citation::{validate_citations_with_policy, Citation, CitationPolicy, EvidenceText};
use crate::config::{meta_f64, meta_i64, meta_value};
use crate::consolidation::iso_week_label;
use crate::error::{PolarisError, Result};
use crate::grader::LlmConfig;

pub const REPORT_SCHEMA_VERSION: i64 = 1;

const TEN_MINUTES_DAYS: f64 = 10.0 / 1440.0;
const EVIDENCE_CAP: usize = 20;
const CALIBRATION_EVIDENCE_CAP: usize = 12;
const CALIBRATION_CONCEPT_CAP: usize = 5;
const HYPOTHESIS_CAP: usize = 3;
const SUGGESTION_LOOKBACK_DAYS: f64 = 90.0;
const SUGGESTION_SAMPLE_CAP: usize = 200;

const REFLECTION_PROMPTS: [&str; 3] = [
    "本周哪个概念的实际表现最出乎你的意料？为什么？",
    "上面哪条断言和你的自我感觉不符？标记「不准」——这本身就是校正数据。",
    "下周你优先补哪个缺口？打算用什么方式验证自己真的补上了？",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportItem {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub claim: String,
    pub confidence: f64,
    pub evidence_ids: Vec<String>,
    pub stats: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkippedCandidate {
    pub id: String,
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HazardGateStatus {
    pub participates: bool,
    pub reason: String,
    pub validation_auc: Option<f64>,
    pub auc_gate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MirrorReportNarrative {
    pub text: String,
    pub citations: Vec<Citation>,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MirrorReport {
    pub schema_version: i64,
    pub id: String,
    pub week: String,
    pub generated_at: String,
    pub window_days: i64,
    pub assertions: Vec<ReportItem>,
    pub hypotheses: Vec<ReportItem>,
    pub suggestions: Vec<ReportItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_signal: Option<ReportItem>,
    pub skipped: Vec<SkippedCandidate>,
    pub hazard_gate: HazardGateStatus,
    pub reflection_prompts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrative: Option<MirrorReportNarrative>,
}

#[derive(Debug, Clone)]
struct Candidate {
    item: ReportItem,
    sample_n: usize,
}

#[derive(Debug, Deserialize)]
struct RawNarrative {
    text: String,
    #[serde(default)]
    citations: Vec<Citation>,
}

enum Admission {
    Kept(ReportItem),
    Skipped(SkippedCandidate),
}

pub fn run_mirror_report(conn: &Connection) -> Result<MirrorReport> {
    run_mirror_report_inner(conn, NarrativeSource::None)
}

pub fn run_mirror_report_with_config(conn: &Connection, config: LlmConfig) -> Result<MirrorReport> {
    run_mirror_report_inner(conn, NarrativeSource::Config(config))
}

pub fn run_mirror_report_with_static_narrative(
    conn: &Connection,
    response_json: &str,
) -> Result<MirrorReport> {
    run_mirror_report_inner(conn, NarrativeSource::Static(response_json))
}

enum NarrativeSource<'a> {
    None,
    Config(LlmConfig),
    Static(&'a str),
}

fn run_mirror_report_inner(
    conn: &Connection,
    narrative: NarrativeSource<'_>,
) -> Result<MirrorReport> {
    let window_days = meta_i64(conn, "report.window_days")?.max(1);
    let min_evidence = meta_i64(conn, "report.min_evidence")?.max(1) as usize;
    let confidence_floor = meta_f64(conn, "report.confidence_floor")?;
    let suppress_days = meta_i64(conn, "report.feedback_suppress_days")?.max(0);

    let mut assertions = Vec::new();
    let mut skipped = Vec::new();
    let hazard_gate = hazard_gate_status(conn)?;

    let mut assertion_candidates = Vec::new();
    assertion_candidates.extend(calibration_phantom_candidates(conn)?);
    assertion_candidates.extend(hint_abandon_candidate(conn, window_days)?);
    assertion_candidates.extend(abandon_time_contrast_candidate(
        conn,
        window_days,
        min_evidence,
    )?);
    assertion_candidates.extend(gu_pattern_candidates(conn)?);
    assertion_candidates.extend(hazard_risk_candidate(conn, window_days, &hazard_gate)?);
    for candidate in assertion_candidates {
        match admit_assertion(
            conn,
            candidate,
            min_evidence,
            confidence_floor,
            suppress_days,
        )? {
            Admission::Kept(item) => assertions.push(item),
            Admission::Skipped(skip) => skipped.push(skip),
        }
    }

    let mut hypotheses = Vec::new();
    for candidate in consolidation_hypotheses(conn)? {
        match admit_hypothesis(conn, candidate, suppress_days)? {
            Admission::Kept(item) => hypotheses.push(item),
            Admission::Skipped(skip) => skipped.push(skip),
        }
    }

    let mut suggestions = Vec::new();
    for candidate in param_suggestions(conn)? {
        match admit_assertion(
            conn,
            candidate,
            min_evidence,
            confidence_floor,
            suppress_days,
        )? {
            Admission::Kept(item) => suggestions.push(item),
            Admission::Skipped(skip) => skipped.push(skip),
        }
    }

    sort_items(&mut assertions);
    sort_items(&mut hypotheses);
    sort_items(&mut suggestions);
    let top_signal = select_top_signal(&assertions, &hypotheses, &suggestions);
    skipped.sort_by(|left, right| left.id.cmp(&right.id));

    let generated_at = now_iso(conn)?;
    let report = MirrorReport {
        schema_version: REPORT_SCHEMA_VERSION,
        id: Uuid::new_v4().to_string(),
        week: iso_week_label(&generated_at)?,
        generated_at,
        window_days,
        assertions,
        hypotheses,
        suggestions,
        top_signal,
        skipped,
        hazard_gate,
        reflection_prompts: REFLECTION_PROMPTS
            .iter()
            .map(|prompt| (*prompt).to_owned())
            .collect(),
        narrative: None,
    };
    let mut report = report;
    report.narrative = match narrative {
        NarrativeSource::None => None,
        NarrativeSource::Static(response_json) => {
            parse_narrative_response(conn, &report, response_json).ok()
        }
        NarrativeSource::Config(config) => generate_narrative_with_config(conn, &report, config)?,
    };
    persist_report(conn, &report)?;
    Ok(report)
}

pub fn latest_mirror_report(conn: &Connection) -> Result<Option<MirrorReport>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT report_json FROM mirror_reports
             ORDER BY julianday(generated_at) DESC, id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    json.as_deref()
        .map(|payload| serde_json::from_str(payload).map_err(Into::into))
        .transpose()
}

fn generate_narrative_with_config(
    conn: &Connection,
    report: &MirrorReport,
    config: LlmConfig,
) -> Result<Option<MirrorReportNarrative>> {
    let evidence = narrative_evidence(report);
    if evidence.is_empty() {
        return Ok(None);
    }
    let LlmConfig::OpenAiCompatible {
        base_url,
        model,
        api_key,
    } = config
    else {
        return Ok(None);
    };

    for _ in 0..2 {
        let response = call_openai_compatible_for_narrative(report, &base_url, &model, &api_key);
        if let Ok(response_json) = response {
            if let Ok(narrative) = parse_narrative_response(conn, report, &response_json) {
                return Ok(Some(narrative));
            }
        }
    }
    Ok(None)
}

fn parse_narrative_response(
    conn: &Connection,
    report: &MirrorReport,
    response_json: &str,
) -> std::result::Result<MirrorReportNarrative, String> {
    let raw: RawNarrative =
        serde_json::from_str(response_json).map_err(|error| error.to_string())?;
    let text = raw.text.trim();
    if text.is_empty() {
        return Err("empty narrative text".to_owned());
    }
    if raw.citations.is_empty() {
        return Err("missing narrative citation".to_owned());
    }
    let evidence = narrative_evidence(report);
    let policy = CitationPolicy::from_conn(conn).map_err(|error| error.to_string())?;
    validate_citations_with_policy(&raw.citations, &evidence, policy)
        .map_err(|error| error.to_string())?;
    Ok(MirrorReportNarrative {
        text: text.to_owned(),
        citations: raw.citations,
        degraded: false,
    })
}

fn narrative_evidence(report: &MirrorReport) -> Vec<EvidenceText> {
    report
        .assertions
        .iter()
        .chain(report.hypotheses.iter())
        .chain(report.suggestions.iter())
        .map(|item| EvidenceText {
            id: item.id.clone(),
            text: item.claim.clone(),
        })
        .collect()
}

fn call_openai_compatible_for_narrative(
    report: &MirrorReport,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<String> {
    #[derive(Deserialize)]
    struct ChatResponse {
        choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(Deserialize)]
    struct Message {
        content: String,
    }

    let endpoint = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "temperature": 0,
        "response_format": {"type": "json_object"},
        "messages": [
            {
                "role": "system",
                "content": "You are Polaris Tier 1 report polisher. Return only JSON with text and citations. Every citation evidence_id must be one report item id, and every quote must be an exact substring of that item's claim. Do not add new facts."
            },
            {
                "role": "user",
                "content": narrative_prompt(report)
            }
        ]
    });

    let response: ChatResponse = reqwest::blocking::Client::new()
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()?
        .error_for_status()?
        .json()?;

    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| PolarisError::InvalidGraderResponse("empty choices".to_owned()))
}

fn narrative_prompt(report: &MirrorReport) -> String {
    let items = report
        .assertions
        .iter()
        .chain(report.hypotheses.iter())
        .chain(report.suggestions.iter())
        .map(|item| {
            format!(
                "id: {}\nkind: {}\nconfidence: {:.3}\nclaim: {}",
                item.id, item.kind, item.confidence, item.claim
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "Week: {}\nWindow days: {}\nAllowed report items:\n{}\n\nReturn JSON: {{\"text\":\"中文周报叙事，不新增事实\", \"citations\":[{{\"evidence_id\":\"item id\", \"quote\":\"claim 原文子串\"}}]}}",
        report.week, report.window_days, items
    )
}

pub fn record_report_feedback(
    conn: &Connection,
    report_id: Option<&str>,
    assertion_id: &str,
    verdict: &str,
) -> Result<String> {
    let verdict = verdict.trim().to_ascii_lowercase();
    if !matches!(verdict.as_str(), "accurate" | "inaccurate") {
        return Err(PolarisError::InvalidParameter {
            key: "report.feedback_verdict".to_owned(),
            value: verdict.to_owned(),
        });
    }

    let report = match report_id {
        Some(id) => load_report(conn, id)?,
        None => latest_mirror_report(conn)?,
    }
    .ok_or_else(|| PolarisError::InvalidParameter {
        key: "report.feedback_report_id".to_owned(),
        value: report_id.unwrap_or("<latest missing>").to_owned(),
    })?;

    let known = report
        .assertions
        .iter()
        .chain(report.hypotheses.iter())
        .chain(report.suggestions.iter())
        .any(|item| item.id == assertion_id);
    if !known {
        return Err(PolarisError::InvalidParameter {
            key: "report.feedback_assertion_id".to_owned(),
            value: assertion_id.to_owned(),
        });
    }

    let payload = serde_json::json!({
        "report_id": report.id,
        "assertion_id": assertion_id,
        "verdict": verdict,
    })
    .to_string();
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (?1, 'report', strftime('%Y-%m-%dT%H:%M:%SZ','now'), 'report_feedback', NULL, ?2)",
        params![Uuid::new_v4().to_string(), payload],
    )?;
    Ok(report.id)
}

fn load_report(conn: &Connection, report_id: &str) -> Result<Option<MirrorReport>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT report_json FROM mirror_reports WHERE id=?1",
            [report_id],
            |row| row.get(0),
        )
        .optional()?;
    json.as_deref()
        .map(|payload| serde_json::from_str(payload).map_err(Into::into))
        .transpose()
}

fn persist_report(conn: &Connection, report: &MirrorReport) -> Result<()> {
    conn.execute(
        "INSERT INTO mirror_reports(id, week, generated_at, report_json, assertion_count, skipped_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            report.id,
            report.week,
            report.generated_at,
            serde_json::to_string(report)?,
            report.assertions.len() as i64,
            report.skipped.len() as i64,
        ],
    )?;
    conn.execute(
        "INSERT INTO behavior_events(id, session_id, at, type, concept_id, payload_json)
         VALUES (?1, 'report', ?2, 'mirror_report', NULL, ?3)",
        params![
            Uuid::new_v4().to_string(),
            report.generated_at,
            serde_json::json!({
                "report_id": report.id,
                "week": report.week,
                "assertions": report.assertions.len(),
                "hypotheses": report.hypotheses.len(),
                "suggestions": report.suggestions.len(),
                "skipped": report.skipped.len(),
            })
            .to_string(),
        ],
    )?;
    Ok(())
}

fn admit_assertion(
    conn: &Connection,
    candidate: Candidate,
    min_evidence: usize,
    confidence_floor: f64,
    suppress_days: i64,
) -> Result<Admission> {
    let Candidate { item, sample_n } = candidate;
    if item.evidence_ids.is_empty() {
        return Ok(Admission::Skipped(skip(&item, "no_evidence")));
    }
    if sample_n < min_evidence {
        return Ok(Admission::Skipped(skip(&item, "insufficient_evidence")));
    }
    if item.confidence < confidence_floor {
        return Ok(Admission::Skipped(skip(&item, "below_confidence_floor")));
    }
    if feedback_suppressed(conn, &item.id, suppress_days)? {
        return Ok(Admission::Skipped(skip(&item, "user_marked_inaccurate")));
    }
    Ok(Admission::Kept(item))
}

fn admit_hypothesis(
    conn: &Connection,
    candidate: Candidate,
    suppress_days: i64,
) -> Result<Admission> {
    let Candidate { item, .. } = candidate;
    if item.evidence_ids.is_empty() {
        return Ok(Admission::Skipped(skip(&item, "no_evidence")));
    }
    if feedback_suppressed(conn, &item.id, suppress_days)? {
        return Ok(Admission::Skipped(skip(&item, "user_marked_inaccurate")));
    }
    Ok(Admission::Kept(item))
}

fn skip(item: &ReportItem, reason: &str) -> SkippedCandidate {
    SkippedCandidate {
        id: item.id.clone(),
        kind: item.kind.clone(),
        reason: reason.to_owned(),
    }
}

fn feedback_suppressed(conn: &Connection, assertion_id: &str, suppress_days: i64) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM behavior_events
         WHERE type='report_feedback'
           AND json_extract(payload_json, '$.assertion_id')=?1
           AND json_extract(payload_json, '$.verdict')='inaccurate'
           AND julianday(at) >= julianday('now') - ?2",
        params![assertion_id, suppress_days as f64],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn sort_items(items: &mut [ReportItem]) {
    items.sort_by(|left, right| left.id.cmp(&right.id));
}

fn select_top_signal(
    assertions: &[ReportItem],
    hypotheses: &[ReportItem],
    suggestions: &[ReportItem],
) -> Option<ReportItem> {
    let mut candidates = assertions
        .iter()
        .chain(suggestions.iter())
        .chain(hypotheses.iter())
        .filter(|item| item.kind != "param_suggestion")
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        top_signal_score(right)
            .partial_cmp(&top_signal_score(left))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.id.cmp(&right.id))
    });
    candidates.first().map(|item| (*item).clone())
}

fn top_signal_score(item: &ReportItem) -> f64 {
    let confidence = if item.confidence.is_finite() {
        item.confidence
    } else {
        0.0
    };
    confidence * top_signal_kind_weight(&item.kind)
}

fn top_signal_kind_weight(kind: &str) -> f64 {
    match kind {
        "calibration_phantom" => 1.30,
        "gu_pattern" => 1.20,
        "abandon_time_contrast" => 1.10,
        "hazard_risk_summary" => 1.05,
        "hint_abandon_conditional" => 1.00,
        "consolidation_hypothesis" => 0.60,
        _ => 0.0,
    }
}

fn suggested_action_for_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "calibration_phantom" => Some("可以为该概念挑一道更高深度的验证题（迁移 / 自由解释）。"),
        "hint_abandon_conditional" => {
            Some("下次连续求提示时，不妨先停下复述你对边界的理解，再看提示。")
        }
        "abandon_time_contrast" => Some("考虑避开高放弃率时段，或把该时段改为纯复习任务。"),
        "gu_pattern" => Some("针对该错误模式做一道反例 / 边界题，看能否独立识别。"),
        "consolidation_hypothesis" => Some("这是引擎提出的待验证假设，暂当参考、不必当结论。"),
        "param_suggestion" => Some("给开发者的参数复核建议，不影响你的今天。"),
        "hazard_risk_summary" => Some("今天可以适当降低任务强度，或把高摩擦任务往后挪。"),
        _ => None,
    }
}

fn now_iso(conn: &Connection) -> Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// 断言挖掘
// ---------------------------------------------------------------------------

fn calibration_phantom_candidates(conn: &Connection) -> Result<Vec<Candidate>> {
    let phantom_gap = meta_f64(conn, "calib.phantom_gap")?;
    let phantom_p = meta_f64(conn, "calib.phantom_p")?;
    let phantom_n = meta_i64(conn, "calib.phantom_n")?;

    let mut stmt = conn.prepare(
        "SELECT ms.concept_id, COALESCE(c.name, ms.concept_id), ms.calib_gap, ms.p_known
         FROM mastery_states ms
         JOIN concepts c ON c.id = ms.concept_id
         WHERE ms.attempt_count >= ?1 AND ms.calib_gap >= ?2 AND ms.p_known < ?3
         ORDER BY ms.calib_gap DESC, ms.concept_id ASC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(
            params![
                phantom_n,
                phantom_gap,
                phantom_p,
                CALIBRATION_CONCEPT_CAP as i64
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut candidates = Vec::new();
    for (concept_id, name, calib_gap, p_known) in rows {
        let samples = calibration_samples(conn, &concept_id, CALIBRATION_EVIDENCE_CAP)?;
        if samples.is_empty() {
            continue;
        }

        let posterior = posterior_from_samples(&samples);
        let overestimates = posterior.overestimates;
        let n = posterior.total;
        let confidence = posterior.probability_over_half;
        let mut evidence_ids = samples
            .iter()
            .map(|sample| format!("attempt:{}", sample.attempt_id))
            .collect::<Vec<_>>();
        evidence_ids.sort();

        candidates.push(Candidate {
            item: ReportItem {
                id: format!("calibration_phantom:{concept_id}"),
                kind: "calibration_phantom".to_owned(),
                subject: concept_id.clone(),
                claim: format!(
                    "概念「{name}」：你的自信持续高于实际表现（校准差 EWMA {calib_gap:+.2}，近 {n} 次作答中 {overestimates} 次高估）——幻影掌握风险。"
                ),
                confidence,
                evidence_ids,
                stats: serde_json::json!({
                    "calib_gap": calib_gap,
                    "p_known": p_known,
                    "overestimates": overestimates,
                    "n": n,
                    "alpha": posterior.alpha,
                    "beta": posterior.beta,
                    "probability_over_half": posterior.probability_over_half,
                }),
                suggested_action: suggested_action_for_kind("calibration_phantom").map(str::to_owned),
            },
            sample_n: n,
        });
    }
    Ok(candidates)
}

#[derive(Debug, Clone)]
struct TimedEvent {
    id: String,
    session_id: String,
    at_julian: f64,
}

fn hint_abandon_candidate(conn: &Connection, window_days: i64) -> Result<Vec<Candidate>> {
    let hints = timed_events(conn, "hint", window_days)?;
    let abandons = timed_events(conn, "abandon", window_days)?;
    let attempts = timed_attempts(conn, window_days)?;
    if hints.is_empty() {
        return Ok(Vec::new());
    }

    // 索引事件：同会话内 10 分钟里的第二次 hint；索引事件间隔 ≥ 10 分钟避免重叠计数。
    let mut episodes = Vec::new();
    let mut last_hint: std::collections::BTreeMap<&str, f64> = Default::default();
    let mut last_index: std::collections::BTreeMap<&str, f64> = Default::default();
    for hint in &hints {
        let session = hint.session_id.as_str();
        let is_streak = last_hint
            .get(session)
            .map(|previous| hint.at_julian - previous <= TEN_MINUTES_DAYS)
            .unwrap_or(false);
        let far_from_last_index = last_index
            .get(session)
            .map(|previous| hint.at_julian - previous > TEN_MINUTES_DAYS)
            .unwrap_or(true);
        if is_streak && far_from_last_index {
            episodes.push(hint.clone());
            last_index.insert(session, hint.at_julian);
        }
        last_hint.insert(session, hint.at_julian);
    }
    if episodes.is_empty() {
        return Ok(Vec::new());
    }

    let abandon_after = |session: &str, from: f64| -> Option<String> {
        abandons
            .iter()
            .find(|event| {
                event.session_id == session
                    && event.at_julian > from
                    && event.at_julian - from <= TEN_MINUTES_DAYS
            })
            .map(|event| event.id.clone())
    };

    let mut cond_success = 0usize;
    let mut evidence_ids = Vec::new();
    for episode in &episodes {
        evidence_ids.push(format!("behavior:{}", episode.id));
        if let Some(abandon_id) = abandon_after(&episode.session_id, episode.at_julian) {
            cond_success += 1;
            evidence_ids.push(format!("behavior:{abandon_id}"));
        }
    }
    let cond_n = episodes.len();

    // 基线：不在任何索引事件 10 分钟窗内的 attempt，其后 10 分钟内是否放弃。
    let in_episode_window = |session: &str, at: f64| -> bool {
        episodes.iter().any(|episode| {
            episode.session_id == session
                && at >= episode.at_julian
                && at - episode.at_julian <= TEN_MINUTES_DAYS
        })
    };
    let mut base_success = 0usize;
    let mut base_n = 0usize;
    for attempt in &attempts {
        if in_episode_window(&attempt.session_id, attempt.at_julian) {
            continue;
        }
        base_n += 1;
        if abandon_after(&attempt.session_id, attempt.at_julian).is_some() {
            base_success += 1;
        }
    }
    if base_n == 0 {
        return Ok(Vec::new());
    }

    let cond_rate = cond_success as f64 / cond_n as f64;
    let base_rate = base_success as f64 / base_n as f64;
    let confidence = prob_beta_greater(
        (cond_success + 1) as f64,
        (cond_n - cond_success + 1) as f64,
        (base_success + 1) as f64,
        (base_n - base_success + 1) as f64,
    );
    evidence_ids.sort();
    evidence_ids.truncate(EVIDENCE_CAP);

    Ok(vec![Candidate {
        item: ReportItem {
            id: "hint_abandon_conditional:hint_streak_2".to_owned(),
            kind: "hint_abandon_conditional".to_owned(),
            subject: "hint_streak_2".to_owned(),
            claim: format!(
                "连续两次提示后 10 分钟内，你 {cond_success}/{cond_n} 次放弃了会话（{:.0}%）；其余时刻的基线放弃率为 {:.0}%（{base_success}/{base_n}）。",
                cond_rate * 100.0,
                base_rate * 100.0,
            ),
            confidence,
            evidence_ids,
            stats: serde_json::json!({
                "cond_success": cond_success,
                "cond_n": cond_n,
                "base_success": base_success,
                "base_n": base_n,
            }),
            suggested_action: suggested_action_for_kind("hint_abandon_conditional")
                .map(str::to_owned),
        },
        sample_n: cond_n,
    }])
}

const BUCKET_LABELS: [&str; 4] = [
    "凌晨（UTC 0-6 点）",
    "上午（UTC 6-12 点）",
    "下午（UTC 12-18 点）",
    "晚上（UTC 18-24 点）",
];

fn abandon_time_contrast_candidate(
    conn: &Connection,
    window_days: i64,
    min_evidence: usize,
) -> Result<Vec<Candidate>> {
    let abandons = bucketed_events(conn, "abandon", window_days)?;
    let attempts = bucketed_attempts(conn, window_days)?;

    let mut activity = [0usize; 4];
    let mut abandon_counts = [0usize; 4];
    let mut abandon_ids: [Vec<String>; 4] = Default::default();
    for (bucket, id) in abandons {
        abandon_counts[bucket] += 1;
        activity[bucket] += 1;
        abandon_ids[bucket].push(format!("behavior:{id}"));
    }
    for bucket in attempts {
        activity[bucket] += 1;
    }

    let eligible = (0..4)
        .filter(|&bucket| activity[bucket] >= min_evidence)
        .collect::<Vec<_>>();
    if eligible.len() < 2 {
        return Ok(Vec::new());
    }
    let rate = |bucket: usize| abandon_counts[bucket] as f64 / activity[bucket] as f64;
    let hi = eligible
        .iter()
        .copied()
        .max_by(|&left, &right| {
            rate(left)
                .partial_cmp(&rate(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(right.cmp(&left))
        })
        .expect("eligible non-empty");
    let lo = eligible
        .iter()
        .copied()
        .min_by(|&left, &right| {
            rate(left)
                .partial_cmp(&rate(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.cmp(&right))
        })
        .expect("eligible non-empty");
    if hi == lo || abandon_counts[hi] == 0 || rate(hi) <= rate(lo) {
        return Ok(Vec::new());
    }

    let confidence = prob_beta_greater(
        (abandon_counts[hi] + 1) as f64,
        (activity[hi] - abandon_counts[hi] + 1) as f64,
        (abandon_counts[lo] + 1) as f64,
        (activity[lo] - abandon_counts[lo] + 1) as f64,
    );
    let claim = if abandon_counts[lo] > 0 {
        format!(
            "你在{}的放弃频率约为{}的 {:.1} 倍（{}/{} vs {}/{}，按每次活动计）。",
            BUCKET_LABELS[hi],
            BUCKET_LABELS[lo],
            rate(hi) / rate(lo),
            abandon_counts[hi],
            activity[hi],
            abandon_counts[lo],
            activity[lo],
        )
    } else {
        format!(
            "你在{}的放弃频率明显高于{}（{}/{} vs 0/{}，按每次活动计）。",
            BUCKET_LABELS[hi], BUCKET_LABELS[lo], abandon_counts[hi], activity[hi], activity[lo],
        )
    };
    let mut evidence_ids = abandon_ids[hi].clone();
    evidence_ids.sort();
    evidence_ids.truncate(EVIDENCE_CAP);

    Ok(vec![Candidate {
        item: ReportItem {
            id: format!("abandon_time_contrast:bucket{hi}_vs_bucket{lo}"),
            kind: "abandon_time_contrast".to_owned(),
            subject: format!("bucket{hi}_vs_bucket{lo}"),
            claim,
            confidence,
            evidence_ids,
            stats: serde_json::json!({
                "hi_bucket": hi,
                "lo_bucket": lo,
                "hi_abandons": abandon_counts[hi],
                "hi_activity": activity[hi],
                "lo_abandons": abandon_counts[lo],
                "lo_activity": activity[lo],
            }),
            suggested_action: suggested_action_for_kind("abandon_time_contrast").map(str::to_owned),
        },
        sample_n: abandon_counts[hi] + abandon_counts[lo],
    }])
}

fn gu_pattern_candidates(conn: &Connection) -> Result<Vec<Candidate>> {
    let retire_p = meta_f64(conn, "gu.retire_p")?;
    let mut stmt = conn.prepare(
        "SELECT id, pattern, concept_ids_json, attempt_ids_json, count, alpha, beta, status
         FROM gu_rules
         WHERE status IN ('active', 'validated')
         ORDER BY pattern ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut candidates = Vec::new();
    for (rule_id, pattern, concept_ids_json, attempt_ids_json, count, alpha, beta, status) in rows {
        let concept_ids: Vec<String> = serde_json::from_str(&concept_ids_json)?;
        let attempt_ids: Vec<String> = serde_json::from_str(&attempt_ids_json)?;
        let names = concept_names(conn, &concept_ids)?;
        let confidence = 1.0 - regularized_incomplete_beta(retire_p, alpha, beta);
        let mut evidence_ids = attempt_ids
            .iter()
            .map(|id| format!("attempt:{id}"))
            .collect::<Vec<_>>();
        evidence_ids.sort();
        evidence_ids.truncate(EVIDENCE_CAP);

        candidates.push(Candidate {
            sample_n: attempt_ids.len(),
            item: ReportItem {
                id: format!("gu_pattern:{rule_id}"),
                kind: "gu_pattern".to_owned(),
                subject: pattern.clone(),
                claim: format!(
                    "你在概念 {} 上反复出现「{pattern}」错误模式（{count} 次失败触发，规则状态 {status}，预测精度后验 P(precision≥{:.0}%)={:.0}%）。这是行为模式标注，不是个人特质。",
                    names.join("、"),
                    retire_p * 100.0,
                    confidence * 100.0,
                ),
                confidence,
                evidence_ids,
                stats: serde_json::json!({
                    "rule_id": rule_id,
                    "pattern": pattern,
                    "status": status,
                    "count": count,
                    "alpha": alpha,
                    "beta": beta,
                }),
                suggested_action: suggested_action_for_kind("gu_pattern").map(str::to_owned),
            },
        });
    }
    Ok(candidates)
}

fn consolidation_hypotheses(conn: &Connection) -> Result<Vec<Candidate>> {
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT id, proposals_json, status FROM consolidation_runs
             ORDER BY julianday(ran_at) DESC, id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((run_id, proposals_json, status)) = row else {
        return Ok(Vec::new());
    };
    let proposals: Vec<serde_json::Value> = serde_json::from_str(&proposals_json)?;

    let mut candidates = Vec::new();
    for (idx, proposal) in proposals.iter().take(HYPOTHESIS_CAP).enumerate() {
        let concepts = proposal
            .get("concepts")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if concepts.is_empty() {
            continue;
        }
        let names = concept_names(conn, &concepts)?;
        candidates.push(Candidate {
            sample_n: concepts.len(),
            item: ReportItem {
                id: format!("consolidation_hypothesis:{run_id}:{idx}"),
                kind: "consolidation_hypothesis".to_owned(),
                subject: format!("{run_id}:{idx}"),
                claim: format!(
                    "夜间巩固发现概念 {} 的残差按周同步波动，提示候选潜在维度；当前状态：{status}——未过留出验证门，仅为假设，不影响任何调度或评分。",
                    names.join("、"),
                ),
                confidence: 0.5,
                evidence_ids: vec![format!("consolidation:{run_id}")],
                stats: proposal.clone(),
                suggested_action: suggested_action_for_kind("consolidation_hypothesis")
                    .map(str::to_owned),
            },
        });
    }
    Ok(candidates)
}

fn param_suggestions(conn: &Connection) -> Result<Vec<Candidate>> {
    let bias_thresh = meta_f64(conn, "report.suggest_bias_thresh")?;
    let bias_n = meta_i64(conn, "report.suggest_bias_n")?.max(1) as usize;

    let mut stmt = conn.prepare(
        "SELECT id, provisional_score - final_score
         FROM attempts
         WHERE final_score IS NOT NULL AND provisional_score IS NOT NULL
           AND julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) >= julianday('now') - ?1
         ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) DESC, id DESC
         LIMIT ?2",
    )?;
    let samples = stmt
        .query_map(
            params![SUGGESTION_LOOKBACK_DAYS, SUGGESTION_SAMPLE_CAP as i64],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let n = samples.len();
    if n < bias_n {
        return Ok(Vec::new());
    }
    let bias = samples.iter().map(|(_, delta)| delta).sum::<f64>() / n as f64;
    if bias.abs() < bias_thresh {
        return Ok(Vec::new());
    }

    let aligned = samples
        .iter()
        .filter(|(_, delta)| delta.signum() == bias.signum() && *delta != 0.0)
        .count();
    let confidence = prob_beta_greater_half((aligned + 1) as f64, (n - aligned + 1) as f64);
    let base = meta_value(conn, "grade.provisional_base")?;
    let slope = meta_value(conn, "grade.provisional_slope")?;
    let direction = if bias > 0.0 { "高" } else { "低" };
    let mut evidence_ids = samples
        .iter()
        .take(EVIDENCE_CAP)
        .map(|(id, _)| format!("attempt:{id}"))
        .collect::<Vec<_>>();
    evidence_ids.sort();

    Ok(vec![Candidate {
        sample_n: n,
        item: ReportItem {
            id: "param_suggestion:grade.provisional".to_owned(),
            kind: "param_suggestion".to_owned(),
            subject: "grade.provisional".to_owned(),
            claim: format!(
                "乐观落账启发式系统性偏{direction}：provisional 比 final 平均 {bias:+.2}（n={n}）。建议人工复核 grade.provisional_base（当前 {base}）与 grade.provisional_slope（当前 {slope}）。仅建议，引擎不会自行修改。"
            ),
            confidence,
            evidence_ids,
            stats: serde_json::json!({
                "bias": bias,
                "n": n,
                "aligned": aligned,
            }),
            suggested_action: suggested_action_for_kind("param_suggestion").map(str::to_owned),
        },
    }])
}

/// hazard 风险摘要——仅当 hazard 模型过 AUC 门才生成（DATA_MODEL §7）。
fn hazard_risk_candidate(
    conn: &Connection,
    window_days: i64,
    gate: &HazardGateStatus,
) -> Result<Vec<Candidate>> {
    if !gate.participates {
        return Ok(Vec::new());
    }
    let Some(validation_auc) = gate.validation_auc else {
        return Ok(Vec::new());
    };

    let mut stmt = conn.prepare(
        "SELECT id, json_extract(payload_json, '$.hazard.probability')
         FROM behavior_events
         WHERE type='mental_state'
           AND json_extract(payload_json, '$.score_source')='provisional'
           AND json_extract(payload_json, '$.hazard.model_status')='fitted'
           AND julianday(at) >= julianday('now') - ?1
         ORDER BY julianday(at) ASC, rowid ASC",
    )?;
    let rows = stmt
        .query_map(params![window_days as f64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<f64>>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let samples = rows
        .into_iter()
        .filter_map(|(id, probability)| probability.map(|value| (id, value)))
        .filter(|(_, value)| value.is_finite())
        .collect::<Vec<_>>();
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let n = samples.len();
    let mean = samples.iter().map(|(_, value)| value).sum::<f64>() / n as f64;
    let peak = samples
        .iter()
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max);
    let mut evidence_ids = samples
        .iter()
        .map(|(id, _)| format!("behavior:{id}"))
        .collect::<Vec<_>>();
    evidence_ids.sort();
    evidence_ids.truncate(EVIDENCE_CAP);

    Ok(vec![Candidate {
        sample_n: n,
        item: ReportItem {
            id: "hazard_risk_summary:window".to_owned(),
            kind: "hazard_risk_summary".to_owned(),
            subject: "window".to_owned(),
            claim: format!(
                "本窗口 {n} 次作答的即时放弃风险均值 {:.0}%、峰值 {:.0}%（hazard 模型留出 AUC {validation_auc:.2}，已过 {:.2} 门）。",
                mean * 100.0,
                peak * 100.0,
                gate.auc_gate,
            ),
            confidence: validation_auc,
            evidence_ids,
            stats: serde_json::json!({
                "mean": mean,
                "peak": peak,
                "n": n,
                "validation_auc": validation_auc,
            }),
            suggested_action: suggested_action_for_kind("hazard_risk_summary").map(str::to_owned),
        },
    }])
}

fn hazard_gate_status(conn: &Connection) -> Result<HazardGateStatus> {
    let auc_gate = meta_f64(conn, "hazard.auc_gate")?;
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload_json FROM behavior_events
             WHERE type='mental_state'
             ORDER BY julianday(at) DESC, rowid DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Ok(HazardGateStatus {
            participates: false,
            reason: "no_mental_state_data".to_owned(),
            validation_auc: None,
            auc_gate,
        });
    };
    let value: serde_json::Value = serde_json::from_str(&payload)?;
    let validation_auc = value
        .pointer("/hazard/validation_auc")
        .and_then(serde_json::Value::as_f64);
    let (participates, reason) = match validation_auc {
        None => (false, "model_unfit".to_owned()),
        Some(auc) if auc < auc_gate => (false, "auc_below_gate".to_owned()),
        Some(_) => (true, "auc_gate_passed".to_owned()),
    };
    Ok(HazardGateStatus {
        participates,
        reason,
        validation_auc,
        auc_gate,
    })
}

// ---------------------------------------------------------------------------
// 数据装载辅助
// ---------------------------------------------------------------------------

fn timed_events(conn: &Connection, event_type: &str, window_days: i64) -> Result<Vec<TimedEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(session_id, 'default'), julianday(at)
         FROM behavior_events
         WHERE type=?1 AND julianday(at) >= julianday('now') - ?2
         ORDER BY julianday(at) ASC, id ASC",
    )?;
    let rows = stmt
        .query_map(params![event_type, window_days as f64], |row| {
            Ok(TimedEvent {
                id: row.get(0)?,
                session_id: row.get(1)?,
                at_julian: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn timed_attempts(conn: &Connection, window_days: i64) -> Result<Vec<TimedEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(session_id, 'default'), julianday(created_at)
         FROM attempts
         WHERE created_at IS NOT NULL
           AND julianday(created_at) >= julianday('now') - ?1
         ORDER BY julianday(created_at) ASC, id ASC",
    )?;
    let rows = stmt
        .query_map(params![window_days as f64], |row| {
            Ok(TimedEvent {
                id: row.get(0)?,
                session_id: row.get(1)?,
                at_julian: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn bucketed_events(
    conn: &Connection,
    event_type: &str,
    window_days: i64,
) -> Result<Vec<(usize, String)>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', at) AS INTEGER) / 6, id
         FROM behavior_events
         WHERE type=?1 AND julianday(at) >= julianday('now') - ?2
         ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map(params![event_type, window_days as f64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|(bucket, id)| (bucket.clamp(0, 3) as usize, id))
        .collect())
}

fn bucketed_attempts(conn: &Connection, window_days: i64) -> Result<Vec<usize>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', created_at) AS INTEGER) / 6
         FROM attempts
         WHERE created_at IS NOT NULL
           AND julianday(created_at) >= julianday('now') - ?1",
    )?;
    let rows = stmt
        .query_map(params![window_days as f64], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|bucket| bucket.clamp(0, 3) as usize)
        .collect())
}

fn concept_names(conn: &Connection, concept_ids: &[String]) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for concept_id in concept_ids {
        let name: Option<String> = conn
            .query_row(
                "SELECT COALESCE(name, id) FROM concepts WHERE id=?1",
                [concept_id],
                |row| row.get(0),
            )
            .optional()?;
        names.push(name.unwrap_or_else(|| concept_id.clone()));
    }
    Ok(names)
}
