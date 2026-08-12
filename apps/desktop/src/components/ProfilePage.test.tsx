import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { ProfilePage } from "./ProfilePage";

vi.mock("../lib/commands", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/commands")>();
  return { ...original, getProfileWorkspace: vi.fn().mockResolvedValue({ generated_at: "2026-08-12T00:00:00Z", settings: { enabled: true, disclosure_required: true, disclosure_acknowledged: true, summary_sharing_enabled: false, paused_until: null }, facts: [{ id: "sessions", label: "有效会话", value: "12", detail: "行为事实" }], dimensions: [{ key: "self_efficacy", label: "学习自我效能", mean: 0.7, lower: 0.4, upper: 0.9, evidence_count: 12, gate_status: "shadow", gate_label: "尚未通过验证", purpose: "影子模式", will_not_affect: "不会参与调度、评分、掌握度或确定性解释。", evidence_ids: ["evidence-1"] }], notice: "不是人格类型", actions: [] }) };
});

describe("ProfilePage", () => {
  it("shows uncertainty, evidence and the shadow non-effect boundary", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><MemoryRouter><ProfilePage /></MemoryRouter></QueryClientProvider>);
    expect(await screen.findByRole("heading", { name: "这不是“你是哪种人”。" })).toBeVisible();
    expect(screen.getByText("40–90% · 12 条证据")).toBeVisible();
    expect(screen.getByText("尚未通过验证")).toBeVisible();
    expect(screen.getByText(/不会参与调度、评分、掌握度/)).toBeVisible();
  });
});
