use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{PolarisError, Result};

const META_KEY: &str = "ai.interaction_profile";
const MAX_CUSTOM_NOTES_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiInteractionProfile {
    pub version: i64,
    pub persona: String,
    pub verbosity: String,
    pub explanation_depth: String,
    pub proactivity: String,
    pub intervention_frequency: String,
    pub correction_style: String,
    pub custom_notes: Option<String>,
    pub guidance: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiInteractionProfileInput {
    pub persona: Option<String>,
    pub verbosity: Option<String>,
    pub explanation_depth: Option<String>,
    pub proactivity: Option<String>,
    pub intervention_frequency: Option<String>,
    pub correction_style: Option<String>,
    pub custom_notes: Option<String>,
}

pub fn ai_interaction_profile(conn: &Connection) -> Result<AiInteractionProfile> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key=?1", [META_KEY], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(raw) = raw else {
        return Ok(default_profile());
    };
    let profile: AiInteractionProfile =
        serde_json::from_str(&raw).map_err(|error| PolarisError::InvalidParameter {
            key: META_KEY.to_owned(),
            value: format!("invalid JSON: {error}"),
        })?;
    validate_profile(&profile)?;
    Ok(with_guidance(profile))
}

pub fn update_ai_interaction_profile(
    conn: &Connection,
    input: AiInteractionProfileInput,
) -> Result<AiInteractionProfile> {
    let current = ai_interaction_profile(conn)?;
    let profile = AiInteractionProfile {
        version: 1,
        persona: merge_enum(
            "ai_profile.persona",
            input.persona,
            current.persona,
            PERSONA_VALUES,
        )?,
        verbosity: merge_enum(
            "ai_profile.verbosity",
            input.verbosity,
            current.verbosity,
            VERBOSITY_VALUES,
        )?,
        explanation_depth: merge_enum(
            "ai_profile.explanation_depth",
            input.explanation_depth,
            current.explanation_depth,
            EXPLANATION_DEPTH_VALUES,
        )?,
        proactivity: merge_enum(
            "ai_profile.proactivity",
            input.proactivity,
            current.proactivity,
            PROACTIVITY_VALUES,
        )?,
        intervention_frequency: merge_enum(
            "ai_profile.intervention_frequency",
            input.intervention_frequency,
            current.intervention_frequency,
            INTERVENTION_FREQUENCY_VALUES,
        )?,
        correction_style: merge_enum(
            "ai_profile.correction_style",
            input.correction_style,
            current.correction_style,
            CORRECTION_STYLE_VALUES,
        )?,
        custom_notes: match input.custom_notes {
            Some(notes) => normalized_notes(notes)?,
            None => current.custom_notes,
        },
        guidance: String::new(),
    };
    let profile = with_guidance(profile);
    let profile_json = serde_json::to_string(&profile)?;
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
        [META_KEY, profile_json.as_str()],
    )?;
    Ok(profile)
}

fn default_profile() -> AiInteractionProfile {
    with_guidance(AiInteractionProfile {
        version: 1,
        persona: "balanced_mentor".to_owned(),
        verbosity: "normal".to_owned(),
        explanation_depth: "key_steps".to_owned(),
        proactivity: "stuck_only".to_owned(),
        intervention_frequency: "normal".to_owned(),
        correction_style: "guided".to_owned(),
        custom_notes: None,
        guidance: String::new(),
    })
}

fn with_guidance(mut profile: AiInteractionProfile) -> AiInteractionProfile {
    profile.guidance = build_guidance(&profile);
    profile
}

fn build_guidance(profile: &AiInteractionProfile) -> String {
    let mut parts = vec![
        format!("性格：{}。", persona_guidance(&profile.persona)),
        format!("话量：{}。", verbosity_guidance(&profile.verbosity)),
        format!(
            "解释深度：{}。",
            explanation_depth_guidance(&profile.explanation_depth)
        ),
        format!("主动程度：{}。", proactivity_guidance(&profile.proactivity)),
        format!(
            "介入频率：{}。",
            intervention_frequency_guidance(&profile.intervention_frequency)
        ),
        format!(
            "纠错方式：{}。",
            correction_style_guidance(&profile.correction_style)
        ),
    ];
    if let Some(notes) = &profile.custom_notes {
        parts.push(format!("用户补充：{notes}"));
    }
    parts.join("")
}

fn validate_profile(profile: &AiInteractionProfile) -> Result<()> {
    validate_enum("ai_profile.persona", &profile.persona, PERSONA_VALUES)?;
    validate_enum("ai_profile.verbosity", &profile.verbosity, VERBOSITY_VALUES)?;
    validate_enum(
        "ai_profile.explanation_depth",
        &profile.explanation_depth,
        EXPLANATION_DEPTH_VALUES,
    )?;
    validate_enum(
        "ai_profile.proactivity",
        &profile.proactivity,
        PROACTIVITY_VALUES,
    )?;
    validate_enum(
        "ai_profile.intervention_frequency",
        &profile.intervention_frequency,
        INTERVENTION_FREQUENCY_VALUES,
    )?;
    validate_enum(
        "ai_profile.correction_style",
        &profile.correction_style,
        CORRECTION_STYLE_VALUES,
    )?;
    if let Some(notes) = &profile.custom_notes {
        validate_notes_length(notes)?;
    }
    Ok(())
}

fn merge_enum(
    key: &str,
    incoming: Option<String>,
    current: String,
    allowed: &[&str],
) -> Result<String> {
    let Some(value) = incoming else {
        return Ok(current);
    };
    let value = value.trim().to_owned();
    validate_enum(key, &value, allowed)?;
    Ok(value)
}

fn validate_enum(key: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(PolarisError::InvalidParameter {
        key: key.to_owned(),
        value: format!("{value}; expected one of {}", allowed.join(", ")),
    })
}

fn normalized_notes(notes: String) -> Result<Option<String>> {
    let notes = notes.trim();
    if notes.is_empty() {
        Ok(None)
    } else {
        validate_notes_length(notes)?;
        Ok(Some(notes.to_owned()))
    }
}

fn validate_notes_length(notes: &str) -> Result<()> {
    let len = notes.chars().count();
    if len <= MAX_CUSTOM_NOTES_CHARS {
        return Ok(());
    }
    Err(PolarisError::InvalidParameter {
        key: "ai_profile.custom_notes".to_owned(),
        value: format!("length {len}; max {MAX_CUSTOM_NOTES_CHARS} chars"),
    })
}

fn persona_guidance(value: &str) -> &'static str {
    match value {
        "balanced_mentor" => "保持平衡、稳、清楚，像靠谱助教",
        "socratic_tutor" => "苏格拉底式追问，先让学生想，再给提示",
        "strict_coach" => "严格教练，直接指出漏洞和下一步训练",
        "friendly_companion" => "陪伴型，语气温和，降低焦虑",
        "direct_operator" => "执行型，少铺垫，直接给可行动步骤",
        _ => "保持平衡、稳、清楚",
    }
}

fn verbosity_guidance(value: &str) -> &'static str {
    match value {
        "brief" => "简洁，少话，只说必要信息",
        "normal" => "正常，给足上下文但不铺张",
        "detailed" => "详细，多解释推理过程和取舍",
        _ => "正常",
    }
}

fn explanation_depth_guidance(value: &str) -> &'static str {
    match value {
        "answer_only" => "只给结论和最短理由",
        "key_steps" => "解释关键步骤，不展开所有背景",
        "deep" => "展开原理、边界和常见误区",
        "examples_first" => "优先用例子、反例和类比解释",
        _ => "解释关键步骤",
    }
}

fn proactivity_guidance(value: &str) -> &'static str {
    match value {
        "on_request" => "只在学生明确要求时介入",
        "stuck_only" => "学生卡住、连续失败或信心低时再主动介入",
        "proactive" => "主动提醒、追问和建议下一步",
        _ => "学生卡住时介入",
    }
}

fn intervention_frequency_guidance(value: &str) -> &'static str {
    match value {
        "low" => "低频，尽量不打断学习流",
        "normal" => "中频，在关键节点提醒",
        "high" => "高频，持续检查理解并主动追问",
        _ => "中频",
    }
}

fn correction_style_guidance(value: &str) -> &'static str {
    match value {
        "direct" => "直接指出错误和修正方式",
        "guided" => "先引导学生自查，再给修正",
        "supportive" => "先承认有效思路，再温和纠偏",
        _ => "先引导学生自查",
    }
}

const PERSONA_VALUES: &[&str] = &[
    "balanced_mentor",
    "socratic_tutor",
    "strict_coach",
    "friendly_companion",
    "direct_operator",
];
const VERBOSITY_VALUES: &[&str] = &["brief", "normal", "detailed"];
const EXPLANATION_DEPTH_VALUES: &[&str] = &["answer_only", "key_steps", "deep", "examples_first"];
const PROACTIVITY_VALUES: &[&str] = &["on_request", "stuck_only", "proactive"];
const INTERVENTION_FREQUENCY_VALUES: &[&str] = &["low", "normal", "high"];
const CORRECTION_STYLE_VALUES: &[&str] = &["direct", "guided", "supportive"];
