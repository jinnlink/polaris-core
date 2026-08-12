import { useQuery } from "@tanstack/react-query";
import { ClockCounterClockwiseIcon, FlaskIcon, LockKeyIcon, ShieldCheckIcon, TestTubeIcon } from "@phosphor-icons/react";

import { commandKeys, getTrustWorkspace, normalizeCommandError } from "../lib/commands";

export function TrustPage() {
  const workspace = useQuery({ queryKey: commandKeys.trust, queryFn: getTrustWorkspace });
  if (workspace.isPending) return <div className="route-loading" role="status">正在核对实验门与后台活动…</div>;
  if (workspace.error) return <div className="route-error" role="alert"><strong>暂时读不到信任面板</strong><span>{normalizeCommandError(workspace.error).message}</span><button type="button" onClick={() => { void workspace.refetch(); }}>重试</button></div>;
  const data = workspace.data;
  const maxActivity = Math.max(1, ...data.recent_activity.map((item) => item.count_7d));
  return <section className="trust-workspace" aria-labelledby="trust-title">
    <header className="trust-hero"><div><p className="eyebrow">trust · gates, not theatre</p><h1 id="trust-title">系统知道什么，也公开它还不知道什么。</h1><p>这里展示真实门状态、预注册实验和最近后台运行；未通过不是失败文案，而是明确的产品边界。</p></div><div className="trust-local"><LockKeyIcon size={21} /><span>当前 Pack</span><strong>{data.current_pack_id ?? "未选择"}</strong></div></header>
    <section className="framework-gates" aria-labelledby="framework-title"><div className="section-heading"><p className="eyebrow">F1—F5</p><h2 id="framework-title">原创理论层的五道门</h2></div><div>{data.gates.map((gate) => <article key={gate.framework} data-status={gate.status}><header><strong>{gate.framework}</strong><span>{gate.status}</span></header><h3>{gate.name}</h3><p>{gate.reason}</p><footer><code>{gate.gate}</code>{gate.metric && <small>{gate.metric}</small>}</footer></article>)}</div></section>
    <div className="trust-grid">
      <section className="experiment-ledger" aria-labelledby="experiment-title"><header><div><p className="eyebrow">preregistered experiments</p><h2 id="experiment-title">正在发生的实验</h2></div><TestTubeIcon size={25} /></header>{data.breeding_experiments.length + data.mrt_experiments.length === 0 ? <p className="empty-evidence">最近没有活跃的育种或 MRT 实验。</p> : <div>{[...data.breeding_experiments, ...data.mrt_experiments].map((experiment) => <article key={`${experiment.kind}-${experiment.id}`}><span>{experiment.kind === "breeding" ? <FlaskIcon /> : <ShieldCheckIcon />}{experiment.kind} · {experiment.status}</span><h3>{experiment.title}</h3>{experiment.metric !== null && <strong>posterior {Math.round(experiment.metric * 100)}%</strong>}<p>{experiment.sample_summary}</p>{experiment.hypothesis && <blockquote>{experiment.hypothesis}</blockquote>}<time dateTime={experiment.at}>{experiment.at}</time></article>)}</div>}</section>
      <section className="activity-ledger" aria-labelledby="activity-title"><header><div><p className="eyebrow">last {data.window_days} days</p><h2 id="activity-title">后台真实运行</h2></div><ClockCounterClockwiseIcon size={25} /></header>{data.recent_activity.map((activity) => <article key={activity.id}><div><strong>{activity.label}</strong><span>{activity.count_7d}</span></div><i><b style={{ width: `${String(activity.count_7d / maxActivity * 100)}%` }} /></i><p>{activity.last_status ?? "无运行记录"}{activity.last_at ? ` · ${activity.last_at}` : ""}</p></article>)}</section>
    </div>
    <section className="governance-table" aria-labelledby="governance-title"><header><p className="eyebrow">manual governance</p><h2 id="governance-title">准入参数由人明确控制</h2><p>A 类门参数不会被后台自动调优。</p></header><div role="table" aria-label="实验准入治理参数"><div role="row" className="governance-table__head"><span role="columnheader">参数</span><span role="columnheader">当前 / 默认</span><span role="columnheader">边界</span><span role="columnheader">调整路径</span></div>{data.governance.map((parameter) => <div role="row" key={parameter.key}><code role="cell">{parameter.key}</code><span role="cell">{parameter.current_value} / {parameter.default_value}</span><span role="cell">{parameter.bounds ?? "—"}</span><span role="cell">{parameter.class} · {parameter.tuning_route}</span></div>)}</div></section>
  </section>;
}
