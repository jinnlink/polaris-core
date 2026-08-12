import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArchiveIcon, ArrowRightIcon, BookOpenTextIcon, CheckCircleIcon, PlusIcon, ShieldCheckIcon, TrayIcon } from "@phosphor-icons/react";
import { Link, useSearchParams } from "react-router-dom";

import type { InboxPracticeDraft, InboxPracticeSubmitReceipt, InboxWorkspaceItem } from "../contracts/core";
import { actOnInbox, captureWorkspace, commandKeys, draftInboxPractice, getInboxWorkspace, normalizeCommandError, processGradeQueue, submitInboxPractice } from "../lib/commands";

const OPEN_INBOX = { statuses: [] as string[], limit: 50 };
const SESSION_KEY = "polaris.practice.session.v1";

function practiceSessionId() {
  const existing = window.localStorage.getItem(SESSION_KEY);
  if (existing) return existing;
  const value = `desktop-${crypto.randomUUID()}`;
  window.localStorage.setItem(SESSION_KEY, value);
  return value;
}

function inboxDraftKey(captureId: string) {
  return `polaris.inbox.draft.${captureId}`;
}

export function InboxPage() {
  const queryClient = useQueryClient();
  const [searchParams] = useSearchParams();
  const [captureOpen, setCaptureOpen] = useState(searchParams.get("capture") === "1");
  const [captureText, setCaptureText] = useState("");
  const [captureKind, setCaptureKind] = useState("reference");
  const [draft, setDraft] = useState<InboxPracticeDraft | null>(null);
  const [answer, setAnswer] = useState("");
  const [confidence, setConfidence] = useState<number | null>(null);
  const [practiceReceipt, setPracticeReceipt] = useState<{ receipt: InboxPracticeSubmitReceipt; evidenceId: string } | null>(null);
  const [draftStartedAt, setDraftStartedAt] = useState(0);
  const sessionId = useMemo(practiceSessionId, []);
  const conceptId = searchParams.get("concept");
  const inbox = useQuery({ queryKey: commandKeys.inbox(OPEN_INBOX), queryFn: () => getInboxWorkspace(OPEN_INBOX) });

  useEffect(() => {
    if (!draft) return;
    const raw = window.localStorage.getItem(inboxDraftKey(draft.capture_id));
    if (!raw) return;
    try {
      const saved = JSON.parse(raw) as { answer?: string; confidence?: number | null };
      setAnswer(saved.answer ?? "");
      setConfidence(saved.confidence ?? null);
    } catch {
      window.localStorage.removeItem(inboxDraftKey(draft.capture_id));
    }
  }, [draft]);

  useEffect(() => {
    if (!draft) return;
    window.localStorage.setItem(inboxDraftKey(draft.capture_id), JSON.stringify({ answer, confidence }));
  }, [answer, confidence, draft]);
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["core", "inbox"] });
  const capture = useMutation({
    mutationFn: () => captureWorkspace({ session_id: sessionId, source: "desktop-inbox", content_type: "text/plain", text: captureText, learner_kind: captureKind, candidate_concept_ids: conceptId ? [conceptId] : [], note: null }),
    onSuccess: async () => { setCaptureText(""); setCaptureOpen(false); await refresh(); },
  });
  const action = useMutation({
    mutationFn: ({ item, action: nextAction }: { item: InboxWorkspaceItem; action: string }) => actOnInbox({ capture_id: item.capture_id, action: nextAction, note: null }),
    onSuccess: async (_, variables) => {
      await refresh();
      if (variables.action === "accept") {
        const nextDraft = await draftInboxPractice(variables.item.capture_id);
        setPracticeReceipt(null); setDraft(nextDraft); setDraftStartedAt(Date.now());
      }
    },
  });
  const submit = useMutation({
    mutationFn: () => {
      if (!draft || confidence === null) throw new Error("请先完成回答和把握程度");
      return submitInboxPractice({ capture_id: draft.capture_id, session_id: sessionId, response_text: answer, self_confidence: confidence, latency_ms: Math.max(0, Date.now() - draftStartedAt), hint_count: 0 });
    },
    onSuccess: async (nextReceipt) => {
      if (draft) {
        window.localStorage.removeItem(inboxDraftKey(draft.capture_id));
        setPracticeReceipt({ receipt: nextReceipt, evidenceId: draft.evidence_id });
      }
      setDraft(null); setAnswer(""); setConfidence(null); await refresh();
      void processGradeQueue().catch(() => undefined);
    },
  });

  if (inbox.isPending) return <div className="route-loading" role="status">正在读取本地收件箱…</div>;
  if (inbox.error) {
    const error = normalizeCommandError(inbox.error);
    return <div className="route-error" role="alert"><strong>暂时读不到收件箱</strong><span>{error.message}</span><button type="button" onClick={() => void inbox.refetch()}>重试</button></div>;
  }

  const mutationError = capture.error ?? action.error ?? submit.error;
  return (
    <section className="workbench inbox-workbench" aria-labelledby="inbox-title">
      <header className="workbench-header">
        <div><p className="eyebrow">inbox</p><h1 id="inbox-title">把线索变成可验证的问题。</h1><p>资料先留在缓冲层；只有你亲自作答并提交后，才可能影响掌握度。</p></div>
        <button className="primary-action" type="button" onClick={() => { setCaptureOpen((value) => !value); }}><PlusIcon size={18} /> 记录资料</button>
      </header>

      <aside className="recorded-only-banner"><ShieldCheckIcon size={21} /><div><strong>Raw capture ≠ 掌握</strong><p>阅读、粘贴和外部 AI 结论只会保存为资料，不会替你证明会了。</p></div></aside>

      {captureOpen && (
        <form className="capture-composer" onSubmit={(event) => { event.preventDefault(); capture.mutate(); }}>
          <label htmlFor="capture-text">刚刚学到或卡住了什么？</label>
          <textarea id="capture-text" value={captureText} onChange={(event) => { setCaptureText(event.target.value); }} rows={4} placeholder="粘贴原文、错误日志，或写下自己的观察。" />
          <div><label>资料类型<select value={captureKind} onChange={(event) => { setCaptureKind(event.target.value); }}><option value="reference">参考资料</option><option value="own_answer">自己的回答</option><option value="error_log">错误日志</option><option value="code_change">代码修改</option><option value="chat_excerpt">对话摘录</option></select></label><button className="primary-action" type="submit" disabled={!captureText.trim() || capture.isPending}>仅保存资料</button><button className="quiet-action" type="button" onClick={() => { setCaptureOpen(false); }}>取消</button></div>
        </form>
      )}

      {mutationError && <p className="inline-error" role="alert">{normalizeCommandError(mutationError).message}。内容仍保留，可重试或返回 Today。</p>}

      <div className="inbox-grid">
        <div className="inbox-list" aria-label="待处理资料">
          {inbox.data.length === 0 ? <EmptyInbox onCapture={() => { setCaptureOpen(true); }} /> : inbox.data.map((item) => (
            <article className="inbox-item" key={item.capture_id}>
              <div className="inbox-item__meta"><span>{item.learner_kind}</span><span>{item.source}</span><time>{item.updated_at.slice(0, 10)}</time></div>
              <p>{item.text_preview}</p>
              <div className="inbox-item__hint"><span>{item.concept_hint ?? "尚未定位知识点"}</span><strong>{item.message}</strong></div>
              <div className="inbox-item__actions">
                {item.actions.slice(0, 3).map((option) => <button key={option.action} type="button" disabled={action.isPending} onClick={() => { action.mutate({ item, action: option.action }); }}>{option.label}</button>)}
                <details><summary aria-label="更多动作">•••</summary><button type="button" onClick={() => { action.mutate({ item, action: "archive" }); }}><ArchiveIcon size={15} /> 归档</button></details>
              </div>
            </article>
          ))}
        </div>

        <aside className="inbox-practice-panel" aria-live="polite">
          {!draft && practiceReceipt ? (
            <div className="inbox-submission-receipt" role="status">
              <CheckCircleIcon size={34} weight="fill" />
              <p className="eyebrow">answer recorded</p>
              <h2>回答已经本地落账。</h2>
              <p>{practiceReceipt.receipt.message}</p>
              <p className="receipt-provenance">临时结果 {Math.round(practiceReceipt.receipt.provisional_score * 100)}% · 本地启发式暂记 · 证据 {practiceReceipt.evidenceId}</p>
              <div className="workbench-actions"><Link className="primary-action" to="/practice">继续正常练习</Link><Link className="quiet-action" to="/">返回 Today</Link></div>
            </div>
          ) : !draft ? <div className="inbox-practice-panel__empty"><BookOpenTextIcon size={30} /><strong>选择“转成一道小题”</strong><p>系统只使用现有候选知识点，不会从界面偷偷改图谱。</p></div> : (
            <form onSubmit={(event) => { event.preventDefault(); if (confidence) submit.mutate(); }}>
              <span className="eyebrow">draft practice · {draft.concept_hint}</span><h2>{draft.prompt}</h2><blockquote>{draft.source_excerpt}</blockquote>
              <label htmlFor="inbox-answer">你的回答</label><textarea id="inbox-answer" value={answer} onChange={(event) => { setAnswer(event.target.value); }} rows={6} />
              <fieldset><legend>反馈前把握程度</legend><div className="confidence-scale confidence-scale--compact">{[1,2,3,4,5].map((value) => <button type="button" key={value} aria-pressed={confidence === value} className={confidence === value ? "is-selected" : ""} onClick={() => { setConfidence(value); }}>{value}</button>)}</div></fieldset>
              <div className="workbench-actions"><button className="primary-action" type="submit" disabled={!answer.trim() || confidence === null || submit.isPending}>提交并验证 <ArrowRightIcon size={18} /></button><button className="secondary-action" type="button" onClick={() => { setDraft(null); }}>稍后再答</button><Link className="quiet-action" to="/">返回 Today</Link></div>
            </form>
          )}
        </aside>
      </div>
    </section>
  );
}

function EmptyInbox({ onCapture }: { onCapture: () => void }) {
  return <div className="empty-inbox"><TrayIcon size={34} /><strong>收件箱已经清空</strong><p>可以记录新资料、开始正常练习，或返回 Today。</p><div className="workbench-actions"><button className="primary-action" type="button" onClick={onCapture}>记录资料</button><Link className="secondary-action" to="/practice">开始练习</Link><Link className="quiet-action" to="/">返回 Today</Link></div></div>;
}
