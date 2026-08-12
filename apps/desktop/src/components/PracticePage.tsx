import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowRightIcon, BookOpenTextIcon, CheckCircleIcon, ClockCounterClockwiseIcon, CloudArrowUpIcon, ShieldCheckIcon } from "@phosphor-icons/react";
import { Link } from "react-router-dom";

import type { PracticeSubmitReceipt } from "../contracts/core";
import {
  captureWorkspace,
  commandKeys,
  enqueueBackgroundJob,
  getAttemptGradeStatus,
  getPracticeWorkspace,
  normalizeCommandError,
  submitPractice,
} from "../lib/commands";

const SESSION_KEY = "polaris.practice.session.v1";

function practiceSessionId() {
  const existing = window.localStorage.getItem(SESSION_KEY);
  if (existing) return existing;
  const value = `desktop-${crypto.randomUUID()}`;
  window.localStorage.setItem(SESSION_KEY, value);
  return value;
}

function draftKey(taskEventId: string) {
  return `polaris.practice.draft.${taskEventId}`;
}

export function PracticePage() {
  const queryClient = useQueryClient();
  const sessionId = useMemo(practiceSessionId, []);
  const [response, setResponse] = useState("");
  const [confidence, setConfidence] = useState<number | null>(null);
  const [receipt, setReceipt] = useState<PracticeSubmitReceipt | null>(null);
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const practice = useQuery({
    queryKey: commandKeys.practice(sessionId),
    queryFn: () => getPracticeWorkspace(sessionId),
    staleTime: 30_000,
  });
  const task = practice.data?.task ?? null;

  useEffect(() => {
    if (!task) return;
    const raw = window.localStorage.getItem(draftKey(task.task_event_id));
    if (!raw) return;
    try {
      const draft = JSON.parse(raw) as { response?: string; confidence?: number | null };
      setResponse(draft.response ?? "");
      setConfidence(draft.confidence ?? null);
    } catch {
      window.localStorage.removeItem(draftKey(task.task_event_id));
    }
  }, [task]);

  useEffect(() => {
    if (!task || receipt) return;
    window.localStorage.setItem(draftKey(task.task_event_id), JSON.stringify({ response, confidence }));
  }, [confidence, receipt, response, task]);

  const submit = useMutation({
    mutationFn: submitPractice,
    onSuccess: async (nextReceipt) => {
      if (task) window.localStorage.removeItem(draftKey(task.task_event_id));
      setReceipt(nextReceipt);
      setSavedMessage(null);
      await queryClient.invalidateQueries({ queryKey: ["core"] });
      void enqueueBackgroundJob("grade_queue")
        .then(() => queryClient.invalidateQueries({ queryKey: commandKeys.lifecycle }))
        .catch(() => undefined);
    },
  });
  const save = useMutation({
    mutationFn: () => captureWorkspace({
      session_id: sessionId,
      source: "desktop-practice-draft",
      content_type: "text/plain",
      text: response,
      learner_kind: "own_answer",
      candidate_concept_ids: task ? [task.concept_id] : [],
      note: task ? `未提交草稿 · ${task.concept_name}` : "未提交练习草稿",
    }),
    onSuccess: (saved) => { setSavedMessage(saved.message); },
  });
  const grade = useQuery({
    queryKey: receipt ? commandKeys.grade(receipt.attempt_id) : ["core", "grade", "idle"],
    queryFn: () => {
      if (!receipt) throw new Error("尚无可查询的回答");
      return getAttemptGradeStatus(receipt.attempt_id);
    },
    enabled: Boolean(receipt),
  });

  if (practice.isPending) return <div className="route-loading" role="status">正在恢复学习现场…</div>;
  if (practice.error) {
    const error = normalizeCommandError(practice.error);
    return <WorkbenchError title="暂时取不到下一题" message={error.message} onRetry={() => void practice.refetch()} />;
  }

  if (!task) {
    return (
      <section className="workbench workbench--empty" aria-labelledby="practice-title">
        <p className="eyebrow">practice</p>
        <h1 id="practice-title">今天没有必须完成的题。</h1>
        <p>可以记录一条新资料、处理收件箱，或者安心回到 Today。</p>
        <div className="workbench-actions">
          <Link className="primary-action" to="/inbox?capture=1">记录资料</Link>
          <Link className="secondary-action" to="/inbox">处理收件箱</Link>
          <Link className="quiet-action" to="/">返回 Today</Link>
        </div>
      </section>
    );
  }

  const submitError = submit.error ? normalizeCommandError(submit.error) : null;
  const saveError = save.error ? normalizeCommandError(save.error) : null;
  return (
    <section className="workbench practice-workbench" aria-labelledby="practice-title">
      <header className="workbench-header">
        <div>
          <p className="eyebrow">practice · {task.task_type}</p>
          <h1 id="practice-title">验证你真正理解的部分。</h1>
          <p>{task.reason}</p>
        </div>
        <div className="local-first-badge"><ShieldCheckIcon size={18} /> 本地先落账</div>
      </header>

      <div className="practice-layout">
        <article className="prompt-sheet">
          <div className="prompt-sheet__meta">
            <span>{task.concept_name}</span>
            <span>{task.move_id}</span>
          </div>
          <h2>{task.prompt_text}</h2>
          <p className="prompt-sheet__note"><ClockCounterClockwiseIcon size={18} /> 重启应用也会恢复这道题和未提交草稿。</p>
        </article>

        <form className="answer-sheet" onSubmit={(event) => {
          event.preventDefault();
          if (confidence === null) return;
          submit.mutate({ session_id: sessionId, task_event_id: task.task_event_id, response_text: response, self_confidence: confidence });
        }}>
          <label htmlFor="practice-answer">你的解释</label>
          <textarea id="practice-answer" value={response} disabled={Boolean(receipt)} onChange={(event) => { setResponse(event.target.value); }} placeholder="先写出推理，再检查边界条件。" rows={9} />
          <fieldset disabled={Boolean(receipt)}>
            <legend>看到反馈前，你有多大把握？</legend>
            <div className="confidence-scale">
              {[1, 2, 3, 4, 5].map((value) => (
                <button key={value} className={confidence === value ? "is-selected" : ""} type="button" aria-pressed={confidence === value} onClick={() => { setConfidence(value); }}>
                  <strong>{value}</strong><span>{["猜的", "不稳", "一半", "较稳", "确定"][value - 1]}</span>
                </button>
              ))}
            </div>
          </fieldset>

          {submitError && <RecoveryPanel message={submitError.message} onRetry={() => { if (confidence !== null) submit.mutate({ session_id: sessionId, task_event_id: task.task_event_id, response_text: response, self_confidence: confidence }); }} onSave={() => { save.mutate(); }} />}
          {saveError && <p className="inline-error" role="alert">{saveError.message}</p>}
          {savedMessage && <p className="inline-success" role="status">{savedMessage}</p>}

          {!receipt ? (
            <div className="workbench-actions">
              <button className="primary-action" type="submit" disabled={!response.trim() || confidence === null || submit.isPending}>提交回答 <ArrowRightIcon size={18} /></button>
              <button className="secondary-action" type="button" disabled={!response.trim() || save.isPending} onClick={() => { save.mutate(); }}><BookOpenTextIcon size={18} /> 保存为资料</button>
              <Link className="quiet-action" to="/">返回 Today</Link>
            </div>
          ) : (
            <div className="submission-receipt" role="status">
              <CheckCircleIcon size={28} weight="fill" />
              <div>
                <strong>{receipt.message}</strong>
                <p>临时结果 {Math.round(receipt.provisional_score * 100)}% · {grade.data?.final_score == null ? "后台评分排队中" : `最终修正为 ${String(Math.round(grade.data.final_score * 100))}%`}</p>
                {grade.data && (
                  <p className="receipt-provenance">
                    依据：你的原始回答 · {grade.data.final_score == null ? "本地启发式暂记" : "evidence-bound 后台评分"} · 证据 {grade.data.evidence_id}
                  </p>
                )}
              </div>
              <button className="primary-action" type="button" onClick={() => { setReceipt(null); setResponse(""); setConfidence(null); void practice.refetch(); }}>下一题 <ArrowRightIcon size={18} /></button>
            </div>
          )}
        </form>
      </div>
    </section>
  );
}

function WorkbenchError({ title, message, onRetry }: { title: string; message: string; onRetry: () => void }) {
  return <div className="route-error" role="alert"><strong>{title}</strong><span>{message}</span><div><button type="button" onClick={onRetry}>重试</button> <Link to="/">返回 Today</Link></div></div>;
}

function RecoveryPanel({ message, onRetry, onSave }: { message: string; onRetry: () => void; onSave: () => void }) {
  return <aside className="recovery-panel" role="alert"><CloudArrowUpIcon size={22} /><div><strong>回答还在，没有丢失</strong><p>{message}</p><div><button type="button" onClick={onRetry}>重试提交</button><button type="button" onClick={onSave}>保存为资料</button><Link to="/">返回 Today</Link></div></div></aside>;
}
