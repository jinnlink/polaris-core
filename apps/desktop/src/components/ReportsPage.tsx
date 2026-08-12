import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircleIcon, QuotesIcon, SealCheckIcon, ThumbsDownIcon, ThumbsUpIcon, WarningCircleIcon } from "@phosphor-icons/react";

import type { MirrorCurvePoint, ReportItemView } from "../contracts/core";
import { commandKeys, getReportsWorkspace, normalizeCommandError, runReport, submitReportFeedback } from "../lib/commands";

const chartWidth = 560;
const chartHeight = 176;
const chartInset = 16;

function curvePath(points: MirrorCurvePoint[], key: "confidence" | "actual_score") {
  if (points.length === 0) return "";
  const width = chartWidth - chartInset * 2;
  const height = chartHeight - chartInset * 2;
  return points.map((point, index) => {
    const x = chartInset + (points.length === 1 ? width / 2 : index * width / (points.length - 1));
    const y = chartInset + (1 - point[key]) * height;
    return `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");
}

export function ReportsPage() {
  const client = useQueryClient();
  const [feedback, setFeedback] = useState<Record<string, string>>({});
  const workspace = useQuery({ queryKey: commandKeys.reports, queryFn: getReportsWorkspace });
  const regenerate = useMutation({ mutationFn: runReport, onSuccess: async () => client.invalidateQueries({ queryKey: commandKeys.reports }) });
  const judge = useMutation({
    mutationFn: submitReportFeedback,
    onSuccess: (_receipt, input) => { setFeedback((current) => ({ ...current, [input.assertion_id]: input.verdict })); },
  });
  if (workspace.isPending) return <div className="route-loading" role="status">正在整理有出处的学习信号…</div>;
  if (workspace.error) return <ReportError error={workspace.error} onRetry={() => { void workspace.refetch(); }} />;
  const data = workspace.data;
  const report = data.report;
  const maxPhase = Math.max(1, ...data.phase_distribution.map((item) => item.count));
  const error = regenerate.error ?? judge.error;
  return <section className="reports-workspace" aria-labelledby="reports-title">
    <header className="reports-hero"><div><p className="eyebrow">mirror · evidence before story</p><h1 id="reports-title">看见模式，也看见它的证据边界。</h1><p>断言、假设和建议分开呈现；没有来源的句子不会伪装成结论。</p></div><button className="primary-action" type="button" disabled={regenerate.isPending} onClick={() => { regenerate.mutate(); }}>{regenerate.isPending ? "正在重算…" : "重新生成本周报告"}</button></header>
    {error && <p className="inline-error" role="alert">{normalizeCommandError(error).message}</p>}
    <div className="mirror-dashboard">
      <section className="mirror-chart" aria-labelledby="calibration-title"><header><div><p className="eyebrow">confidence mirror</p><h2 id="calibration-title">把握度与实际表现</h2></div><div className="chart-legend"><span data-series="confidence">把握度</span><span data-series="actual">实际分</span></div></header>{data.confidence_curve.length === 0 ? <p className="empty-evidence">完成带把握度的练习后，这里会出现校准曲线。</p> : <><svg role="img" aria-label="最近练习的把握度与实际表现折线图" viewBox={`0 0 ${String(chartWidth)} ${String(chartHeight)}`}><line x1="16" y1="88" x2="544" y2="88" /><path data-series="confidence" d={curvePath(data.confidence_curve, "confidence")} /><path data-series="actual" d={curvePath(data.confidence_curve, "actual_score")} />{data.confidence_curve.map((point, index) => <circle key={point.attempt_id} cx={chartInset + (data.confidence_curve.length === 1 ? (chartWidth - chartInset * 2) / 2 : index * (chartWidth - chartInset * 2) / (data.confidence_curve.length - 1))} cy={chartInset + (1 - point.actual_score) * (chartHeight - chartInset * 2)} r="3.5"><title>{point.concept_id}：实际 {Math.round(point.actual_score * 100)}%</title></circle>)}</svg><p className="chart-caption">最近 {data.confidence_curve.length} 次可比较回答；虚线 50% 只作读图基线。</p></>}</section>
      <section className="phase-bars" aria-labelledby="phase-title"><p className="eyebrow">knowledge phases</p><h2 id="phase-title">知识相分布</h2>{data.phase_distribution.length === 0 ? <p className="empty-evidence">当前还没有可分相概念。</p> : data.phase_distribution.map((item) => <article key={item.phase}><div><strong>{item.label}</strong><span>{item.count}</span></div><i><b style={{ width: `${String(item.count / maxPhase * 100)}%` }} /></i><p>{item.summary}</p></article>)}</section>
    </div>
    {!report ? <section className="report-empty"><QuotesIcon size={34} /><h2>还没有镜像报告</h2><p>生成只读取本地结构化证据；证据不足的候选会列入跳过原因，不会硬写故事。</p></section> : <>
      <section className="top-signal" aria-labelledby="top-signal-title"><div><span>{report.citation_status === "verified" ? <SealCheckIcon weight="fill" /> : <WarningCircleIcon />} strict-citation · {report.citation_status}</span><p>{report.week} · {report.window_days} 天窗口</p></div><h2 id="top-signal-title">{report.top_signal?.claim ?? "本周没有达到报告门的主信号。"}</h2>{report.top_signal?.suggested_action && <p>{report.top_signal.suggested_action}</p>}</section>
      {report.narrative && <section className="report-narrative"><QuotesIcon size={26} /><div><p>{report.narrative.text}</p><details><summary>核对叙事引用 · {report.narrative.citations.length}</summary>{report.narrative.citations.map((citation) => <blockquote key={`${citation.evidence_id}-${citation.quote}`}><code>{citation.evidence_id}</code>{citation.quote}</blockquote>)}</details></div></section>}
      <section className="report-stream" aria-labelledby="report-items-title"><div className="section-heading"><p className="eyebrow">claims under review</p><h2 id="report-items-title">逐条核对，不准就纠正</h2></div>{report.items.length === 0 ? <p className="empty-evidence">没有候选通过本周证据门。</p> : report.items.map((item) => <ReportCard key={item.id} item={item} reportId={report.id} verdict={feedback[item.id]} pending={judge.isPending} onFeedback={(verdict) => { judge.mutate({ report_id: report.id, assertion_id: item.id, verdict }); }} />)}</section>
      <div className="report-foot-grid"><section><h2>风险模型门</h2><strong>{report.hazard_participates ? "已通过，可参与" : "未通过，不参与"}</strong><p>{report.hazard_reason}</p>{report.hazard_validation_auc !== null && <code>AUC {report.hazard_validation_auc.toFixed(3)}</code>}</section><section><h2>本周反思</h2><ol>{report.reflection_prompts.map((prompt) => <li key={prompt}>{prompt}</li>)}</ol></section></div>
      {report.skipped.length > 0 && <details className="report-skipped"><summary>查看被证据门挡下的候选 · {report.skipped.length}</summary>{report.skipped.map((item) => <p key={item.id}><code>{item.kind}</code>{item.reason}</p>)}</details>}
    </>}
  </section>;
}

function ReportCard({ item, reportId, verdict, pending, onFeedback }: { item: ReportItemView; reportId: string; verdict?: string; pending: boolean; onFeedback: (verdict: string) => void }) {
  const confidence = useMemo(() => Math.round(item.confidence * 100), [item.confidence]);
  return <article className="report-card" data-category={item.category}><header><span>{item.category} · {item.kind}</span><strong>{confidence}%</strong></header><h3>{item.claim}</h3>{item.suggested_action && <p>{item.suggested_action}</p>}<details><summary>证据来源 · {item.evidence_ids.length}</summary><code>{item.evidence_ids.join(" · ") || "无证据，不应进入报告"}</code></details><footer aria-label={`${item.id} 准确度反馈`}><span>{verdict ? <><CheckCircleIcon weight="fill" /> 已记录：{verdict === "accurate" ? "准确" : "不准"}</> : "这符合你的实际吗？"}</span><button type="button" disabled={pending || verdict === "accurate"} onClick={() => { onFeedback("accurate"); }}><ThumbsUpIcon /> 准确</button><button type="button" disabled={pending || verdict === "inaccurate"} onClick={() => { onFeedback("inaccurate"); }}><ThumbsDownIcon /> 不准</button><span className="sr-only">报告 {reportId}</span></footer></article>;
}

function ReportError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  return <div className="route-error" role="alert"><strong>暂时读不到镜像报告</strong><span>{normalizeCommandError(error).message}</span><button type="button" onClick={onRetry}>重试</button></div>;
}
