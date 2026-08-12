import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { PracticePage } from "./PracticePage";

const commands = vi.hoisted(() => ({
  captureWorkspace: vi.fn(),
  getAttemptGradeStatus: vi.fn(),
  getPracticeWorkspace: vi.fn(),
  enqueueBackgroundJob: vi.fn(),
  submitPractice: vi.fn(),
}));

vi.mock("../lib/commands", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/commands")>();
  return { ...original, ...commands };
});

const workspace = {
  task: {
    task_event_id: "task-event-1",
    session_id: "session-test",
    concept_id: "ownership",
    concept_name: "所有权",
    move_id: "retrieval",
    task_type: "free_recall",
    prompt_text: "解释所有权移动发生了什么。",
    reason: "现在回忆能最大化长期保留。",
    issued_at: "2026-08-10T10:00:00Z",
  },
  actions: [],
};

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter><PracticePage /></MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("PracticePage", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.localStorage.setItem("polaris.practice.session.v1", "session-test");
    vi.clearAllMocks();
    commands.getPracticeWorkspace.mockResolvedValue(workspace);
    commands.submitPractice.mockResolvedValue({ attempt_id: "attempt-1", provisional_score: 0.72, degraded: false, message: "回答已本地落账" });
    commands.getAttemptGradeStatus.mockResolvedValue({ attempt_id: "attempt-1", evidence_id: "evidence-answer-1", provisional_score: 0.72, final_score: null, graded_at: null, queued: true });
    commands.enqueueBackgroundJob.mockResolvedValue({ effect: "background_job_enqueued", message: "已排队" });
  });

  it("requires pre-feedback confidence and returns a zero-wait local receipt", async () => {
    renderPage();

    const answer = await screen.findByLabelText("你的解释");
    fireEvent.change(answer, { target: { value: "值被移动后，旧绑定不能再使用。" } });
    expect(screen.getByRole("button", { name: /提交回答/ })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "4较稳" }));
    fireEvent.click(screen.getByRole("button", { name: /提交回答/ }));

    await waitFor(() => {
      expect(commands.submitPractice.mock.calls[0]?.[0]).toEqual({
        session_id: "session-test",
        task_event_id: "task-event-1",
        response_text: "值被移动后，旧绑定不能再使用。",
        self_confidence: 4,
      });
    });
    expect(await screen.findByText("回答已本地落账")).toBeVisible();
    expect(screen.getByText(/后台评分排队中/)).toBeVisible();
    expect(await screen.findByText(/依据：你的原始回答.*证据 evidence-answer-1/)).toBeVisible();
  });

  it("restores the unfinished answer and confidence after restart", async () => {
    window.localStorage.setItem("polaris.practice.draft.task-event-1", JSON.stringify({ response: "恢复的草稿", confidence: 3 }));
    renderPage();

    expect(await screen.findByDisplayValue("恢复的草稿")).toBeVisible();
    expect(screen.getByRole("button", { name: "3一半" })).toHaveAttribute("aria-pressed", "true");
  });

  it("keeps the answer and offers three recovery actions after a submit failure", async () => {
    commands.submitPractice.mockRejectedValueOnce(new Error("database is busy"));
    renderPage();

    const answer = await screen.findByLabelText("你的解释");
    fireEvent.change(answer, { target: { value: "不会丢失的回答" } });
    fireEvent.click(screen.getByRole("button", { name: "2不稳" }));
    fireEvent.click(screen.getByRole("button", { name: /提交回答/ }));

    expect(await screen.findByText("回答还在，没有丢失")).toBeVisible();
    expect(answer).toHaveValue("不会丢失的回答");
    const recovery = within(screen.getByRole("alert"));
    expect(recovery.getByRole("button", { name: "重试提交" })).toBeVisible();
    expect(recovery.getByRole("button", { name: "保存为资料" })).toBeVisible();
    expect(recovery.getByRole("link", { name: "返回 Today" })).toBeVisible();
  });
});
