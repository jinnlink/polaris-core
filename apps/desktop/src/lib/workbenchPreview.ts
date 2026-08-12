import type {
  AttemptGradeStatus,
  CaptureWorkspaceInput,
  CaptureWorkspaceReceipt,
  GradeQueueReceipt,
  InboxActionInput,
  InboxActionReceipt,
  InboxPracticeDraft,
  InboxPracticeSubmitInput,
  InboxPracticeSubmitReceipt,
  InboxWorkspaceItem,
  InboxWorkspaceQuery,
  PracticeSubmitInput,
  PracticeSubmitReceipt,
  PracticeWorkspaceSnapshot,
} from "../contracts/core";

const now = "2026-08-10T10:00:00Z";
let sequence = 3;
const submittedTasks = new Set<string>();
const gradeStatuses = new Map<string, AttemptGradeStatus>();
const inboxItems: InboxWorkspaceItem[] = [
  {
    capture_id: "capture-dijkstra",
    evidence_id: "evidence-dijkstra",
    status: "practice_ready",
    learner_kind: "error_log",
    source: "AI IDE",
    content_type: "text/plain",
    text_preview: "Dijkstra 在负权边示例上的推理失败，原因可能是贪心不变量被破坏。",
    concept_hint: "Dijkstra 最短路径",
    note: "来自今天的调试记录",
    created_at: now,
    updated_at: now,
    message: "要不要用它做一道小题？",
    actions: [
      { action: "accept", label: "转成一道小题" },
      { action: "defer", label: "稍后再看" },
      { action: "ignore", label: "忽略" },
    ],
  },
  {
    capture_id: "capture-union-find",
    evidence_id: "evidence-union-find",
    status: "pending",
    learner_kind: "reference",
    source: "clipboard",
    content_type: "text/plain",
    text_preview: "路径压缩与按秩合并解决的是两种不同的树高来源。",
    concept_hint: "并查集",
    note: null,
    created_at: now,
    updated_at: now,
    message: "已保存，稍后帮你整理",
    actions: [
      { action: "accept", label: "转成一道小题" },
      { action: "defer", label: "稍后再看" },
      { action: "ignore", label: "忽略" },
    ],
  },
];

export function previewPracticeWorkspace(sessionId: string): PracticeWorkspaceSnapshot {
  return {
    task: {
      task_event_id: `preview-task-${sessionId}`,
      session_id: sessionId,
      concept_id: "dijkstra",
      concept_name: "Dijkstra 最短路径",
      move_id: "boundary_case",
      task_type: "explain",
      prompt_text: "为什么 Dijkstra 算法不能直接处理负权边？请用一个最小反例解释被破坏的不变量。",
      reason: "迁移证据不足 · 负权边边界仍模糊",
      issued_at: now,
    },
    actions: [
      { id: "submit", label: "提交回答", kind: "primary" },
      { id: "capture", label: "保存为资料", kind: "secondary" },
      { id: "today", label: "返回 Today", kind: "quiet" },
    ],
  };
}

export function previewSubmitPractice(input: PracticeSubmitInput): PracticeSubmitReceipt {
  if (!input.response_text.trim()) throw new Error("请先写下你的回答");
  if (input.self_confidence < 1 || input.self_confidence > 5) throw new Error("把握程度必须在 1–5 之间");
  if (submittedTasks.has(input.task_event_id)) throw new Error("这道题已经提交过了");
  submittedTasks.add(input.task_event_id);
  const attemptId = `preview-attempt-${String(sequence++)}`;
  gradeStatuses.set(attemptId, {
    attempt_id: attemptId,
    evidence_id: `evidence-${attemptId}`,
    provisional_score: 0.5,
    final_score: null,
    graded_at: null,
    queued: true,
  });
  return { attempt_id: attemptId, provisional_score: 0.5, degraded: true, message: "回答已本地落账；后台评分回来后会自动修正。" };
}

export function previewAttemptGradeStatus(attemptId: string): AttemptGradeStatus {
  const status = gradeStatuses.get(attemptId);
  if (!status) throw new Error("找不到这次回答");
  return status;
}

export function previewProcessGradeQueue(): GradeQueueReceipt {
  return { processed: 0, pending: [...gradeStatuses.values()].filter((item) => item.queued).length };
}

export function previewCaptureWorkspace(input: CaptureWorkspaceInput): CaptureWorkspaceReceipt {
  if (!input.text.trim()) throw new Error("资料内容不能为空");
  const captureId = `capture-preview-${String(sequence++)}`;
  inboxItems.unshift({
    capture_id: captureId,
    evidence_id: `evidence-preview-${String(sequence)}`,
    status: "pending",
    learner_kind: input.learner_kind,
    source: input.source,
    content_type: input.content_type,
    text_preview: input.text.trim().slice(0, 160),
    concept_hint: input.candidate_concept_ids[0] ?? null,
    note: input.note,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    message: "已保存，稍后帮你整理",
    actions: [
      { action: "accept", label: "转成一道小题" },
      { action: "defer", label: "稍后再看" },
      { action: "ignore", label: "忽略" },
    ],
  });
  return { capture_id: captureId, evidence_id: `evidence-preview-${String(sequence)}`, status: "pending", learner_kind: input.learner_kind, effect: "recorded_only", message: "已保存为学习资料，不会直接算作掌握。" };
}

export function previewInboxWorkspace(query: InboxWorkspaceQuery): InboxWorkspaceItem[] {
  const visible = query.statuses.length > 0 ? query.statuses : ["pending", "mapped", "practice_ready"];
  return inboxItems.filter((item) => visible.includes(item.status)).slice(0, query.limit || 20);
}

export function previewActOnInbox(input: InboxActionInput): InboxActionReceipt {
  const item = inboxItems.find((candidate) => candidate.capture_id === input.capture_id);
  if (!item) throw new Error("找不到这条资料");
  item.status = input.action === "accept" ? "practice_ready" : input.action === "ignore" ? "ignored" : input.action === "archive" ? "archived" : item.status;
  item.message = input.action === "accept" ? "要不要用它做一道小题？" : input.action === "defer" ? "已保留，稍后再看。" : input.action === "ignore" ? "已忽略。" : "已归档，不再提醒。";
  return { capture_id: item.capture_id, status: item.status, effect: "recorded_only", message: item.message };
}

export function previewDraftInboxPractice(captureId: string): InboxPracticeDraft {
  const item = inboxItems.find((candidate) => candidate.capture_id === captureId);
  if (!item || item.status !== "practice_ready") throw new Error("这条资料还没有准备成小题");
  return { capture_id: item.capture_id, evidence_id: item.evidence_id, status: item.status, concept_hint: item.concept_hint, task_type: "explain", prompt: `不用复述原文：请解释「${item.concept_hint ?? "这条资料"}」的关键机制，并给出一个边界条件。`, source_excerpt: item.text_preview, message: "先回答这道小题，再告诉我你的把握程度。" };
}

export function previewSubmitInboxPractice(input: InboxPracticeSubmitInput): InboxPracticeSubmitReceipt {
  const item = inboxItems.find((candidate) => candidate.capture_id === input.capture_id);
  if (!item) throw new Error("找不到这条资料");
  if (!input.response_text.trim()) throw new Error("请先写下你的回答");
  if (input.self_confidence < 1 || input.self_confidence > 5) throw new Error("把握程度必须在 1–5 之间");
  item.status = "practiced";
  const attemptId = `preview-attempt-${String(sequence++)}`;
  gradeStatuses.set(attemptId, { attempt_id: attemptId, evidence_id: item.evidence_id, provisional_score: 0.5, final_score: null, graded_at: null, queued: true });
  return { capture_id: item.capture_id, attempt_id: attemptId, status: "practiced", effect: "submitted", message: "已提交这道小题，系统会用你的回答更新学习状态。", provisional_score: 0.5, degraded: true };
}
