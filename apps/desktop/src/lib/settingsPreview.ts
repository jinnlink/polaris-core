import type { AiInteractionProfileUpdate, ProfileMeasurementSubmitInput, ProfileSettingsUpdateInput, SettingsMutationReceipt, SettingsWorkspaceSnapshot } from "../contracts/core";

let profile = { enabled: true, disclosure_required: false, disclosure_acknowledged: true, summary_sharing_enabled: false, paused_until: null as string | null };
let aiProfile = { persona: "balanced_mentor", verbosity: "normal", explanation_depth: "key_steps", proactivity: "stuck_only", intervention_frequency: "normal", correction_style: "guided", custom_notes: null as string | null, guidance: "性格：保持平衡、稳、清楚，像靠谱助教。话量：正常。解释深度：解释关键步骤。" };

export function previewSettingsWorkspace(): SettingsWorkspaceSnapshot {
  return { generated_at: new Date().toISOString(), profile: structuredClone(profile), ai_profile: structuredClone(aiProfile), tier0_only: false, profile_measurement_count: 6, profile_dimension_count: 2, valid_session_count: 18,
    privacy_calls: [
      { id: "llm_grade_attempt", tier: "Tier 1", trigger: "提交回答 / 后台评分", data_sent: ["回答文本", "领域评分规约", "strict-citation 证据提示"], degradation: "启发式临时分 + 评分队列重试", disabled_when_tier0_only: true },
      { id: "llm_mirror_narrative", tier: "Tier 1", trigger: "生成带叙事的镜像报告", data_sent: ["已过门的报告断言、假设与建议"], degradation: "保留结构化本地报告，不生成叙事", disabled_when_tier0_only: true },
      { id: "llm_concept_suggestion", tier: "Tier 1", trigger: "Inbox 分析新知识点", data_sent: ["选中的原始资料与证据编号", "当前学习空间", "已安装概念的编号、名称与类型"], degradation: "原始资料保持不变，不生成候选，也不更新掌握度", disabled_when_tier0_only: true },
      { id: "embed_concept", tier: "Tier 1", trigger: "刷新概念几何嵌入", data_sent: ["概念与图式名称"], degradation: "关闭几何层，符号层与潜因子层继续", disabled_when_tier0_only: true },
    ],
    instruments: [{ id: "gse", title: "General Self-Efficacy Scale", version: "1.0", citation: "Schwarzer & Jerusalem (1995)", source_url: "https://openscales.net/scale.php?code=GSE", response_min: 1, response_max: 4, admin_modes: ["full_scale", "ema_single_item"], interpretation_notice: "单题 EMA 只是分散证据，不能呈现为标准 GSE 总分。", items: [{ id: "gse_01", dimension: "self_efficacy", prompt: "I can always manage to solve difficult problems if I try hard enough.", keyed: "positive" }] }],
  };
}

export function previewUpdateProfileSettings(input: ProfileSettingsUpdateInput): SettingsMutationReceipt { profile = { ...profile, ...(input.enabled === null ? {} : { enabled: input.enabled }), ...(input.summary_sharing_enabled === null ? {} : { summary_sharing_enabled: input.summary_sharing_enabled }), ...(input.paused_until === null ? {} : { paused_until: input.paused_until }), ...(input.clear_pause ? { paused_until: null } : {}), ...(input.acknowledge_disclosure ? { disclosure_required: false, disclosure_acknowledged: true } : {}) }; if (!profile.enabled) profile.summary_sharing_enabled = false; return { effect: "profile_settings_updated", message: "画像设置已保存。" }; }
export function previewUpdateAiProfile(input: AiInteractionProfileUpdate): SettingsMutationReceipt { aiProfile = { ...aiProfile, ...Object.fromEntries(Object.entries(input).filter(([, value]) => value !== null)) }; return { effect: "ai_profile_updated", message: "AI 互动偏好已保存。" }; }
export function previewSubmitMeasurement(input: ProfileMeasurementSubmitInput): SettingsMutationReceipt { return { effect: "profile_measurement_recorded", message: `${input.item_id} 已作为本地画像证据记录。` }; }
export function previewResetProfile(): SettingsMutationReceipt { return { effect: "profile_reset", message: "画像数据已清除；学习证据保持不变。" }; }
