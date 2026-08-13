use std::env;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrivacyCallInventory {
    pub tier0_only: bool,
    pub calls: Vec<PrivacyCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrivacyCall {
    pub id: &'static str,
    pub tier: &'static str,
    pub trigger: &'static str,
    pub env_keys: &'static [&'static str],
    pub data_sent: &'static [&'static str],
    pub degradation: &'static str,
    pub disabled_when_tier0_only: bool,
}

impl PrivacyCallInventory {
    pub fn all() -> Self {
        Self {
            tier0_only: tier0_only_enabled(),
            calls: vec![
                PrivacyCall {
                    id: "llm_grade_attempt",
                    tier: "Tier 1",
                    trigger: "submit / grade-pending / MCP evidence grading",
                    env_keys: &[
                        "POLARIS_LLM_FAST_BASE_URL",
                        "POLARIS_LLM_FAST_MODEL",
                        "POLARIS_LLM_FAST_API_KEY",
                        "POLARIS_LLM_STRONG_BASE_URL",
                        "POLARIS_LLM_STRONG_MODEL",
                        "POLARIS_LLM_STRONG_API_KEY",
                    ],
                    data_sent: &[
                        "attempt response text",
                        "domain rubric",
                        "active G_u prompt context",
                        "strict-citation evidence prompt",
                    ],
                    degradation: "heuristic score + grade_queue retry",
                    disabled_when_tier0_only: true,
                },
                PrivacyCall {
                    id: "llm_mirror_narrative",
                    tier: "Tier 1",
                    trigger: "report --narrative / MCP run_mirror_report(narrative=true)",
                    env_keys: &[
                        "POLARIS_LLM_FAST_BASE_URL",
                        "POLARIS_LLM_FAST_MODEL",
                        "POLARIS_LLM_FAST_API_KEY",
                        "POLARIS_LLM_STRONG_BASE_URL",
                        "POLARIS_LLM_STRONG_MODEL",
                        "POLARIS_LLM_STRONG_API_KEY",
                    ],
                    data_sent: &["mirror report assertion/hypothesis/suggestion claims"],
                    degradation: "raw mirror report without narrative",
                    disabled_when_tier0_only: true,
                },
                PrivacyCall {
                    id: "llm_concept_suggestion",
                    tier: "Tier 1",
                    trigger: "Inbox 分析新知识点",
                    env_keys: &[
                        "POLARIS_LLM_FAST_BASE_URL",
                        "POLARIS_LLM_FAST_MODEL",
                        "POLARIS_LLM_FAST_API_KEY",
                    ],
                    data_sent: &[
                        "selected raw capture text and evidence id",
                        "active base pack id",
                        "installed concept ids, names, and kinds",
                    ],
                    degradation: "raw capture remains unchanged; no suggestion or mastery update",
                    disabled_when_tier0_only: true,
                },
                PrivacyCall {
                    id: "embed_concept",
                    tier: "Tier 1",
                    trigger: "geometry embedding refresh",
                    env_keys: &[
                        "POLARIS_EMBED_BASE_URL",
                        "POLARIS_EMBED_MODEL",
                        "POLARIS_EMBED_API_KEY",
                    ],
                    data_sent: &["concept and schema names used for embedding"],
                    degradation: "geometry layer disabled; symbolic and latent layers continue",
                    disabled_when_tier0_only: true,
                },
            ],
        }
    }
}

pub fn tier0_only_enabled() -> bool {
    env::var("POLARIS_TIER0_ONLY")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
