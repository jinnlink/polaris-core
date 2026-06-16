const SAMPLE_URL = "./data/sample.json";

const fallbackData = {
  meta: {
    project: "polaris-core",
    fixture: true,
    synthetic: true,
    privacy_note: "Embedded fallback mirrors data/sample.json shape and contains only artificial values.",
  },
  generated_at: "2026-06-16T09:30:00Z",
  confidence_curve: [
    { attempt_id: "attempt.synthetic.001", created_at: "2026-06-03T09:10:00Z", concept_id: "ownership_transfer", concept_label: "Ownership transfer", confidence: 0.72, actual_score: 0.58, is_final: true },
    { attempt_id: "attempt.synthetic.002", created_at: "2026-06-04T13:42:00Z", concept_id: "borrow_checker", concept_label: "Borrow checker", confidence: 0.63, actual_score: 0.67, is_final: true },
    { attempt_id: "attempt.synthetic.003", created_at: "2026-06-05T20:05:00Z", concept_id: "iterator_adapters", concept_label: "Iterator adapters", confidence: 0.55, actual_score: 0.74, is_final: true },
    { attempt_id: "attempt.synthetic.004", created_at: "2026-06-07T10:18:00Z", concept_id: "lifetimes_in_structs", concept_label: "Lifetimes in structs", confidence: 0.78, actual_score: 0.51, is_final: true },
    { attempt_id: "attempt.synthetic.005", created_at: "2026-06-09T16:24:00Z", concept_id: "error_propagation", concept_label: "Error propagation", confidence: 0.69, actual_score: 0.81, is_final: true },
    { attempt_id: "attempt.synthetic.006", created_at: "2026-06-10T21:02:00Z", concept_id: "trait_bounds", concept_label: "Trait bounds", confidence: 0.82, actual_score: 0.62, is_final: true },
    { attempt_id: "attempt.synthetic.007", created_at: "2026-06-12T08:46:00Z", concept_id: "async_ownership", concept_label: "Async ownership", confidence: 0.47, actual_score: 0.66, is_final: false },
    { attempt_id: "attempt.synthetic.008", created_at: "2026-06-14T19:35:00Z", concept_id: "module_boundaries", concept_label: "Module boundaries", confidence: 0.74, actual_score: 0.79, is_final: false },
  ],
  phase_distribution: [
    { phase: "undetermined", label: "还看不清", summary: "才试了几次，证据还不够，系统会先补探针任务。", count: 5 },
    { phase: "phantom", label: "看起来懂", summary: "自信高但实际表现不稳，需要用更硬的题确认。", count: 4 },
    { phase: "fluctuation", label: "刚上路", summary: "表现起伏明显，结果还不结实。", count: 7 },
    { phase: "settling", label: "刚扎根", summary: "原场景中渐稳，新场景还卡。", count: 3 },
    { phase: "solidification", label: "稳了但僵", summary: "熟练但迁移受限，需要用变式题松动。", count: 6 },
    { phase: "transfer", label: "能迁移", summary: "能在新情境使用。", count: 9 },
    { phase: "generation", label: "能创造", summary: "能独立产出，且迁移表现更快更稳。", count: 2 },
    { phase: "regression", label: "退步了", summary: "之前会但近期又脱档，需要回到证据补缺。", count: 1 },
  ],
  recent_assertions: [
    { id: "assertion.synthetic.001", kind: "calibration_gap", claim: "Trait bounds 题组出现高自信低得分，可能存在 fluency illusion。", confidence: 0.81, suggested_action: "安排一题要求写出约束失败原因的反例解释。" },
    { id: "assertion.synthetic.002", kind: "phase_shift", claim: "Iterator adapters 从脆弱转向活跃，连续两次解释与应用均达标。", confidence: 0.77, suggested_action: "下一轮给低提示的组合题，避免停留在模板熟练。" },
    { id: "assertion.synthetic.003", kind: "evidence_gap", claim: "Async ownership 缺少 final_score，当前曲线点仍是 provisional。", confidence: 0.69, suggested_action: "等待评分回填前只展示趋势，不把该点计作最终掌握。" },
  ],
};

const phaseColors = {
  undetermined: "#8d9692",
  phantom: "#ba5544",
  fluctuation: "#b88625",
  settling: "#7b6ba8",
  solidification: "#477f9d",
  transfer: "#1f8f82",
  generation: "#176d64",
  regression: "#a85f7b",
};

const refs = {
  generatedAt: document.querySelector("#generated-at"),
  dataSource: document.querySelector("#data-source"),
  learnerWindow: document.querySelector("#learner-window"),
  learnerName: document.querySelector("#learner-name"),
  attemptCount: document.querySelector("#attempt-count"),
  finalizedCount: document.querySelector("#finalized-count"),
  calibrationSignal: document.querySelector("#calibration-signal"),
  calibrationCaption: document.querySelector("#calibration-caption"),
  curveChart: document.querySelector("#curve-chart"),
  phaseBars: document.querySelector("#phase-bars"),
  phaseTotal: document.querySelector("#phase-total"),
  assertionList: document.querySelector("#assertion-list"),
  actionList: document.querySelector("#action-list"),
};

async function loadData() {
  try {
    const response = await fetch(SAMPLE_URL, { cache: "no-store" });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const data = await response.json();
    refs.dataSource.textContent = "data/sample.json";
    return data;
  } catch (error) {
    refs.dataSource.textContent = "embedded fixture";
    refs.dataSource.classList.add("load-error");
    return fallbackData;
  }
}

function clamp01(value) {
  return Math.max(0, Math.min(1, Number(value)));
}

function percent(value) {
  return `${Math.round(clamp01(value) * 100)}%`;
}

function formatDate(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function computeCalibration(points) {
  if (!points.length) {
    return { label: "--", caption: "--" };
  }
  const avgGap = points.reduce((sum, point) => sum + (point.confidence - point.actual_score), 0) / points.length;
  if (avgGap > 0.12) {
    return { label: "偏自信", caption: `平均高估 ${Math.round(avgGap * 100)} 个百分点` };
  }
  if (avgGap < -0.12) {
    return { label: "偏保守", caption: `平均低估 ${Math.round(Math.abs(avgGap) * 100)} 个百分点` };
  }
  return { label: "接近校准", caption: `平均差 ${Math.round(avgGap * 100)} 个百分点` };
}

function renderSummary(data) {
  const points = data.confidence_curve || [];
  const learner = data.learner || {};
  const calibration = computeCalibration(points);
  refs.generatedAt.textContent = formatDate(data.generated_at);
  refs.learnerWindow.textContent = learner.window || "latest 30 scored attempts";
  refs.learnerName.textContent = learner.display_name
    ? `${learner.display_name} · ${learner.cohort_label || "local"}`
    : "local Tier 0 snapshot";
  refs.attemptCount.textContent = learner.attempt_count ?? points.length;
  refs.finalizedCount.textContent = `${learner.finalized_count ?? points.filter((point) => point.is_final).length} finalized`;
  refs.calibrationSignal.textContent = calibration.label;
  refs.calibrationCaption.textContent = calibration.caption;
}

function pointPath(points, key, width, height, pad) {
  return points
    .map((point, index) => {
      const x = pad.left + (index / Math.max(1, points.length - 1)) * (width - pad.left - pad.right);
      const y = pad.top + (1 - clamp01(point[key])) * (height - pad.top - pad.bottom);
      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
}

function renderCurve(points) {
  const width = 920;
  const height = 330;
  const pad = { top: 22, right: 24, bottom: 54, left: 48 };
  const yTicks = [1, 0.75, 0.5, 0.25, 0];
  const confidencePath = pointPath(points, "confidence", width, height, pad);
  const actualPath = pointPath(points, "actual_score", width, height, pad);
  const plotWidth = width - pad.left - pad.right;
  const plotHeight = height - pad.top - pad.bottom;

  const ticks = yTicks
    .map((tick) => {
      const y = pad.top + (1 - tick) * plotHeight;
      return `
        <line x1="${pad.left}" y1="${y}" x2="${width - pad.right}" y2="${y}" stroke="rgba(36,48,44,0.09)" />
        <text class="axis-text" x="${pad.left - 12}" y="${y + 3}" text-anchor="end">${Math.round(tick * 100)}</text>
      `;
    })
    .join("");

  const labels = points
    .map((point, index) => {
      const x = pad.left + (index / Math.max(1, points.length - 1)) * plotWidth;
      const y = height - pad.bottom + 24;
      return `<text class="axis-text" x="${x}" y="${y}" text-anchor="middle">${point.label}</text>`;
    })
    .join("");

  const markers = points
    .map((point, index) => {
      const x = pad.left + (index / Math.max(1, points.length - 1)) * plotWidth;
      const yConfidence = pad.top + (1 - clamp01(point.confidence)) * plotHeight;
      const yActual = pad.top + (1 - clamp01(point.actual_score)) * plotHeight;
      const finalClass = point.is_final ? "" : " provisional";
      return `
        <circle class="curve-point confidence" cx="${x}" cy="${yConfidence}" r="4">
          <title>${conceptLabel(point)}: self ${percent(point.confidence)}</title>
        </circle>
        <circle class="curve-point actual${finalClass}" cx="${x}" cy="${yActual}" r="${point.is_final ? 4 : 5}">
          <title>${conceptLabel(point)}: actual ${percent(point.actual_score)}${point.is_final ? "" : " provisional"}</title>
        </circle>
      `;
    })
    .join("");

  refs.curveChart.innerHTML = `
    <svg viewBox="0 0 ${width} ${height}" preserveAspectRatio="none" aria-hidden="true">
      ${ticks}
      <line x1="${pad.left}" y1="${height - pad.bottom}" x2="${width - pad.right}" y2="${height - pad.bottom}" stroke="rgba(36,48,44,0.18)" />
      <path class="curve-line confidence" d="${confidencePath}" />
      <path class="curve-line actual" d="${actualPath}" />
      ${markers}
      ${labels}
    </svg>
  `;
}

function normalizePoint(point) {
  const createdAt = point.created_at || point.at || "";
  return {
    ...point,
    created_at: createdAt,
    label: point.label || shortDate(createdAt),
    concept_label: point.concept_label || point.concept || point.concept_id || point.attempt_id,
    confidence: Number(point.confidence ?? point.self_confidence ?? 0),
  };
}

function conceptLabel(point) {
  return point.concept_label || point.concept_id || point.attempt_id || "attempt";
}

function shortDate(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "--";
  return new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit" }).format(date);
}

function renderPhases(phases) {
  const total = phases.reduce((sum, phase) => sum + phase.count, 0);
  refs.phaseTotal.textContent = total;
  refs.phaseBars.innerHTML = phases
    .map((phase) => {
      const width = total > 0 ? Math.max(3, (phase.count / total) * 100) : 0;
      const color = phaseColors[phase.phase] || "#1f8f82";
      return `
        <div class="phase-row">
          <div class="phase-name" title="${phase.phase}">${phase.label}</div>
          <div class="phase-track" aria-hidden="true">
            <div class="phase-fill" style="width:${width}%; --phase-color:${color}"></div>
          </div>
          <div class="phase-count">${phase.count}</div>
          <div class="phase-summary">${phase.summary}</div>
        </div>
      `;
    })
    .join("");
}

function renderAssertions(assertions) {
  if (!assertions.length) {
    refs.assertionList.innerHTML = `<article class="assertion-card"><p>暂无近期断言。先积累更多证据，镜像报告不会凭空生成结论。</p></article>`;
    return;
  }
  refs.assertionList.innerHTML = assertions
    .map((assertion) => `
      <article class="assertion-card">
        <div class="assertion-top">
          <span class="pill">${assertion.kind}</span>
          <span class="pill gold">${percent(assertion.confidence)}</span>
        </div>
        <p>${assertion.claim}</p>
        <footer>${assertion.suggested_action || "暂无行动提示"}</footer>
      </article>
    `)
    .join("");
}

function actionPrompts(data) {
  if (Array.isArray(data.action_prompts) && data.action_prompts.length) {
    return data.action_prompts;
  }
  return (data.recent_assertions || [])
    .filter((assertion) => assertion.suggested_action)
    .slice(0, 3)
    .map((assertion, index) => ({
      id: assertion.id,
      priority: index === 0 ? "now" : "next",
      title: assertion.kind.replaceAll("_", " "),
      body: assertion.suggested_action,
    }));
}

function renderActions(actions) {
  if (!actions.length) {
    refs.actionList.innerHTML = `<article class="action-card"><p>暂无行动提示。没有证据时，界面保持沉默。</p></article>`;
    return;
  }
  refs.actionList.innerHTML = actions
    .map((action) => {
      const tone = action.priority === "now" ? "rose" : action.priority === "next" ? "" : "gold";
      return `
        <article class="action-card">
          <div class="action-top">
            <h3>${action.title}</h3>
            <span class="pill ${tone}">${action.priority}</span>
          </div>
          <p>${action.body}</p>
          <footer>${action.id}</footer>
        </article>
      `;
    })
    .join("");
}

function render(data) {
  const normalized = {
    ...data,
    confidence_curve: (data.confidence_curve || []).map(normalizePoint),
    phase_distribution: data.phase_distribution || [],
    recent_assertions: data.recent_assertions || [],
  };
  renderSummary(normalized);
  renderCurve(normalized.confidence_curve);
  renderPhases(normalized.phase_distribution);
  renderAssertions(normalized.recent_assertions);
  renderActions(actionPrompts(normalized));
}

loadData().then(render).catch((error) => {
  refs.curveChart.innerHTML = `<div class="load-error">Learner mirror render failed: ${error.message}</div>`;
});
