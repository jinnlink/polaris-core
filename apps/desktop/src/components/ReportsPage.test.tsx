import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import axe from "axe-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { previewReportsWorkspace } from "../lib/governancePreview";
import { ReportsPage } from "./ReportsPage";

const commands = vi.hoisted(() => ({ enqueueBackgroundJob: vi.fn(), getReportsWorkspace: vi.fn(), submitReportFeedback: vi.fn() }));
vi.mock("../lib/commands", async (importOriginal) => ({ ...await importOriginal<typeof import("../lib/commands")>(), ...commands }));

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><ReportsPage /></QueryClientProvider>);
}

describe("ReportsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.getReportsWorkspace.mockResolvedValue(previewReportsWorkspace());
    commands.enqueueBackgroundJob.mockResolvedValue({ effect: "background_job_enqueued", message: "已排队" });
    commands.submitReportFeedback.mockResolvedValue({ report_id: "report-2026-w33", effect: "feedback_inaccurate", message: "已记录" });
  });

  it("shows calibration chart, strict citations, top signal and feedback", async () => {
    renderPage();
    expect(await screen.findByRole("heading", { name: "看见模式，也看见它的证据边界。" })).toBeVisible();
    expect(screen.getByRole("img", { name: "最近练习的把握度与实际表现折线图" })).toBeVisible();
    expect(screen.getByText(/strict-citation · verified/)).toBeVisible();
    expect(screen.getAllByText(/Dijkstra 边界题/).length).toBeGreaterThan(0);
    fireEvent.click(screen.getAllByRole("button", { name: /不准/ })[0]);
    await waitFor(() => { expect(commands.submitReportFeedback).toHaveBeenCalled(); });
    expect(commands.submitReportFeedback.mock.calls[0]?.[0]).toEqual(expect.objectContaining({ verdict: "inaccurate" }));
  });

  it("has no detectable structural accessibility violations", async () => {
    const { container } = renderPage();
    await screen.findByRole("heading", { name: "看见模式，也看见它的证据边界。" });
    const result = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(result.violations).toEqual([]);
  });
});
