import { useQuery } from "@tanstack/react-query";
import { ArrowRightIcon, EyeIcon, ShieldCheckIcon } from "@phosphor-icons/react";
import { Link } from "react-router-dom";

import { commandKeys, getProfileWorkspace, normalizeCommandError } from "../lib/commands";

export function ProfilePage() {
  const profile = useQuery({ queryKey: commandKeys.profile, queryFn: getProfileWorkspace, staleTime: 30_000 });
  if (profile.isPending) return <div className="route-loading" role="status">正在整理画像证据…</div>;
  if (profile.error) {
    const error = normalizeCommandError(profile.error);
    return <div className="route-error" role="alert"><strong>暂时读不到画像</strong><span>{error.message}</span><button type="button" onClick={() => void profile.refetch()}>重试</button></div>;
  }
  const data = profile.data;
  return (
    <section className="profile-workspace" aria-labelledby="profile-title">
      <header className="profile-hero">
        <div><p className="eyebrow">profile · evidence, not identity</p><h1 id="profile-title">这不是“你是哪种人”。</h1><p>{data.notice}</p></div>
        <div className={`profile-state profile-state--${data.settings.enabled ? "on" : "off"}`}><ShieldCheckIcon size={20} />{data.settings.enabled ? "本地画像已启用" : "画像已关闭"}</div>
      </header>

      <section className="profile-facts" aria-labelledby="facts-title"><div className="section-heading"><p className="eyebrow">observed facts</p><h2 id="facts-title">先看发生过什么</h2></div><div className="profile-fact-grid">{data.facts.map((fact) => <article key={fact.id}><span>{fact.label}</span><strong>{fact.value}</strong><p>{fact.detail}</p></article>)}</div></section>

      <section className="profile-dimensions" aria-labelledby="dimensions-title"><div className="section-heading"><p className="eyebrow">slow posteriors</p><h2 id="dimensions-title">再看仍在验证的慢变量</h2></div><div className="profile-dimension-list">{data.dimensions.length === 0 ? <p className="profile-empty">证据还不足。系统不会为了填满页面制造人格结论。</p> : data.dimensions.map((dimension) => <article className="profile-dimension" key={dimension.key} data-gate={dimension.gate_status}><header><div><span>{dimension.label}</span><strong>{Math.round(dimension.mean * 100)}%</strong></div><em>{dimension.gate_label}</em></header><div className="credible-range" aria-label={`${dimension.label} 区间 ${String(Math.round(dimension.lower * 100))}% 到 ${String(Math.round(dimension.upper * 100))}%`}><span style={{ left: `${String(dimension.lower * 100)}%`, width: `${String(Math.max(2, (dimension.upper - dimension.lower) * 100))}%` }} /><i style={{ left: `${String(dimension.mean * 100)}%` }} /></div><p>{Math.round(dimension.lower * 100)}–{Math.round(dimension.upper * 100)}% · {dimension.evidence_count} 条证据</p><dl><div><dt>用途</dt><dd>{dimension.purpose}</dd></div><div><dt>不会影响</dt><dd>{dimension.will_not_affect}</dd></div></dl><details><summary><EyeIcon size={16} /> 查看证据来源</summary><code>{dimension.evidence_ids.length ? dimension.evidence_ids.join(" · ") : "暂无可展示证据"}</code></details></article>)}</div></section>

      <footer className="workbench-actions profile-actions"><Link className="primary-action" to="/settings">管理画像</Link><Link className="secondary-action" to="/goals">查看目标 <ArrowRightIcon size={18} /></Link><Link className="quiet-action" to="/">返回 Today</Link></footer>
    </section>
  );
}
