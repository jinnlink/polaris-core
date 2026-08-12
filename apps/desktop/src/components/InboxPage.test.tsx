import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { InboxPage } from "./InboxPage";

const commands = vi.hoisted(() => ({
  actOnInbox: vi.fn(),
  captureWorkspace: vi.fn(),
  draftInboxPractice: vi.fn(),
  getInboxWorkspace: vi.fn(),
  enqueueBackgroundJob: vi.fn(),
  submitInboxPractice: vi.fn(),
}));

vi.mock("../lib/commands", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/commands")>();
  return { ...original, ...commands };
});

const item = {
  capture_id: "capture-1",
  evidence_id: "evidence-1",
  status: "open",
  learner_kind: "reference",
  source: "desktop-inbox",
  content_type: "text/plain",
  text_preview: "借用不能活得比所有者更久。",
  concept_hint: "ownership",
  note: null,
  created_at: "2026-08-10T10:00:00Z",
  updated_at: "2026-08-10T10:00:00Z",
  message: "仅记录资料，不影响掌握度",
  actions: [{ action: "accept", label: "转成一道小题" }, { action: "reject", label: "忽略" }],
};

function renderPage(initialEntry = "/inbox") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[initialEntry]}><InboxPage /></MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("InboxPage", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.localStorage.setItem("polaris.practice.session.v1", "session-test");
    vi.clearAllMocks();
    commands.getInboxWorkspace.mockResolvedValue([item]);
    commands.actOnInbox.mockResolvedValue({ capture_id: "capture-1", status: "practice_ready", effect: "recorded_only", message: "已准备小题" });
    commands.draftInboxPractice.mockResolvedValue({ capture_id: "capture-1", evidence_id: "evidence-1", status: "practice_ready", concept_hint: "ownership", task_type: "free_recall", prompt: "解释借用为何不能越过所有者生命周期。", source_excerpt: item.text_preview, message: "请亲自作答" });
    commands.submitInboxPractice.mockResolvedValue({ capture_id: "capture-1", attempt_id: "attempt-1", status: "submitted", effect: "provisional", message: "回答已本地落账", provisional_score: 0.7, degraded: false });
    commands.enqueueBackgroundJob.mockResolvedValue({ effect: "background_job_enqueued", message: "已排队" });
  });

  it("keeps raw capture separate from mastery and turns it into a verified answer", async () => {
    renderPage();

    expect(await screen.findByText("Raw capture ≠ 掌握")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "转成一道小题" }));
    expect(await screen.findByRole("heading", { name: "解释借用为何不能越过所有者生命周期。" })).toBeVisible();
    fireEvent.change(screen.getByLabelText("你的回答"), { target: { value: "引用必须在所有者析构前结束。" } });
    fireEvent.click(screen.getByRole("button", { name: "4" }));
    fireEvent.click(screen.getByRole("button", { name: /提交并验证/ }));

    await waitFor(() => {
      expect(commands.submitInboxPractice).toHaveBeenCalledWith(expect.objectContaining({
        capture_id: "capture-1",
        session_id: "session-test",
        response_text: "引用必须在所有者析构前结束。",
        self_confidence: 4,
      }));
    });
    expect(await screen.findByRole("heading", { name: "回答已经本地落账。" })).toBeVisible();
    expect(screen.getByText(/临时结果 70%.*证据 evidence-1/)).toBeVisible();
  });

  it("restores an Inbox practice draft after it is reopened", async () => {
    window.localStorage.setItem("polaris.inbox.draft.capture-1", JSON.stringify({ answer: "恢复的 Inbox 草稿", confidence: 2 }));
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "转成一道小题" }));

    expect(await screen.findByDisplayValue("恢复的 Inbox 草稿")).toBeVisible();
    expect(screen.getByRole("button", { name: "2" })).toHaveAttribute("aria-pressed", "true");
  });
});
