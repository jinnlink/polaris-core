import type { ReportFeedbackInput, ReportMutationReceipt, ReportsWorkspaceSnapshot, TrustWorkspaceSnapshot } from "../contracts/core";

const generatedAt = "2026-08-12T10:30:00Z";

const report = {
  id: "report-2026-w33", week: "2026-W33", generated_at: generatedAt, window_days: 7,
  items: [
    { id: "calibration-dijkstra", category: "assertion", kind: "calibration", subject: "dijkstra", claim: "你对 Dijkstra 边界题的把握度比实际表现高 18 个百分点。", confidence: 0.84, evidence_ids: ["attempt-d1", "attempt-d2", "attempt-d3"], suggested_action: "下一题先写出算法失效条件，再给出实现。" },
    { id: "hint-abandon", category: "hypothesis", kind: "behavior", subject: "session", claim: "连续两次提示后，中断风险可能上升。当前证据只够作为待验证假设。", confidence: 0.61, evidence_ids: ["session-18", "session-21"], suggested_action: "出现第二次提示时改用 worked example，并继续收集对照证据。" },
    { id: "cut-hi", category: "suggestion", kind: "parameter", subject: "calibration.cut_hi", claim: "当前高把握阈值可能偏松；建议观察更多迁移题后再决定是否调整。", confidence: 0.72, evidence_ids: ["attempt-d1", "attempt-b4"], suggested_action: "保持参数不变，积累到最小样本门。" },
  ],
  top_signal: { id: "calibration-dijkstra", category: "assertion", kind: "calibration", subject: "dijkstra", claim: "你对 Dijkstra 边界题的把握度比实际表现高 18 个百分点。", confidence: 0.84, evidence_ids: ["attempt-d1", "attempt-d2", "attempt-d3"], suggested_action: "下一题先写出算法失效条件，再给出实现。" },
  skipped: [{ id: "hazard-evening", kind: "hazard", reason: "validation_auc_below_gate" }],
  hazard_participates: false, hazard_reason: "留出 AUC 尚未达到 0.70，风险模型不参与调度或确定性报告。", hazard_validation_auc: 0.64,
  reflection_prompts: ["本周哪个概念的实际表现最出乎你的意料？为什么？", "上面哪条断言和你的自我感觉不符？", "下周你优先补哪个缺口？"],
  narrative: { text: "本周最清楚的信号不是做题速度，而是边界题上的把握度偏高。它值得验证，但不应被解释为固定特质。", citations: [{ evidence_id: "calibration-dijkstra", quote: "你对 Dijkstra 边界题的把握度比实际表现高 18 个百分点。" }], degraded: false },
  citation_status: "verified",
};

export function previewReportsWorkspace(): ReportsWorkspaceSnapshot {
  return {
    generated_at: generatedAt,
    confidence_curve: [
      ["a1", 0.55, 0.62], ["a2", 0.7, 0.58], ["a3", 0.62, 0.68], ["a4", 0.82, 0.61], ["a5", 0.76, 0.72], ["a6", 0.88, 0.7],
    ].map(([id, confidence, actual], index) => ({ attempt_id: String(id), concept_id: index > 2 ? "dijkstra" : "binary-search", created_at: `2026-08-${String(index + 5).padStart(2, "0")}T20:00:00Z`, confidence: Number(confidence), actual_score: Number(actual), is_final: true })),
    phase_distribution: [
      { phase: "consolidation", label: "巩固", summary: "能复现，迁移仍需验证。", count: 7 },
      { phase: "transfer", label: "迁移", summary: "正在跨情境验证。", count: 4 },
      { phase: "acquisition", label: "习得", summary: "刚形成可用表征。", count: 3 },
    ],
    report: structuredClone(report),
  };
}

export function previewRunReport(): ReportMutationReceipt {
  return { report_id: report.id, effect: "generated", message: "本周报告已从本地证据重新生成。" };
}

export function previewReportFeedback(input: ReportFeedbackInput): ReportMutationReceipt {
  return { report_id: input.report_id, effect: `feedback_${input.verdict}`, message: "反馈已记录；不准会进入后续报告的抑制与校正。" };
}

export function previewTrustWorkspace(): TrustWorkspaceSnapshot {
  return {
    generated_at: generatedAt, window_days: 7,
    gates: [
      { framework: "F1", name: "pedagogy_signature", status: "running", gate: "evidence_visible", metric: "effect_samples=42, mrt_preregistrations=3", reason: "签名效应可审计，但尚未宣称留出优于固定教法。" },
      { framework: "F2", name: "phase_diagram", status: "available", gate: "deterministic_rule", metric: "concepts=14", reason: "Tier 0 相分类确定性可见，不伪造验证 AUC。" },
      { framework: "F3", name: "friction_curve", status: "running", gate: "not_passed", metric: "signature_mrt_rows=3", reason: "已有预注册，尚无足够摩擦效应样本。" },
      { framework: "F4", name: "g_u_rules", status: "unfit", gate: "no_data", metric: null, reason: "尚无通过门的个人误解规则。" },
      { framework: "F5", name: "breeding", status: "running", gate: "not_passed", metric: "preregistered=1, admitted=0", reason: "候选仍在预注册实验中，不能进入默认教学。" },
    ],
    breeding_experiments: [{ id: "breed-1", kind: "breeding", title: "contrastive_case 对照 worked_example", status: "preregistered", metric: 0.73, sample_summary: "候选 n=9 · 在位 n=11 · 准入 p≥0.8 · 最少 n=20", hypothesis: "边界概念上对比案例能降低下一题校准差。", at: generatedAt }],
    mrt_experiments: [{ id: "mrt-1", kind: "mrt", title: "worked_example", status: "randomized", metric: null, sample_summary: "预注册 prereg-33 · 窗口 next_attempt", hypothesis: "第二次提示后切换例题能降低中断率。", at: generatedAt }],
    recent_activity: [
      { id: "param_tuning", label: "参数调优", count_7d: 1, last_at: generatedAt, last_status: "held" },
      { id: "breeding_evaluated", label: "育种评估", count_7d: 4, last_at: generatedAt, last_status: "preregistered" },
      { id: "mental_hazard", label: "放弃风险拟合", count_7d: 1, last_at: generatedAt, last_status: "below_gate" },
      { id: "consolidation", label: "夜间巩固", count_7d: 6, last_at: generatedAt, last_status: "passed" },
      { id: "mirror_reports", label: "镜像报告", count_7d: 1, last_at: generatedAt, last_status: "generated" },
    ],
    current_pack_id: "algorithms",
    governance: [
      { key: "breeding.admit_p", current_value: "0.8", default_value: "0.8", class: "A", bounds: "[0.5, 0.99]", tuning_route: "manual", is_governance_gate: true },
      { key: "breeding.retire_p", current_value: "0.2", default_value: "0.2", class: "A", bounds: "[0.01, 0.5]", tuning_route: "manual", is_governance_gate: true },
      { key: "breeding.min_n", current_value: "20", default_value: "20", class: "A", bounds: "[5, 1000]", tuning_route: "manual", is_governance_gate: true },
    ],
  };
}
