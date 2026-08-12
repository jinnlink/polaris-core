import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import axe from "axe-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { previewTrustWorkspace } from "../lib/governancePreview";
import { TrustPage } from "./TrustPage";

const commands = vi.hoisted(() => ({ getTrustWorkspace: vi.fn() }));
vi.mock("../lib/commands", async (importOriginal) => ({ ...await importOriginal<typeof import("../lib/commands")>(), ...commands }));

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}><TrustPage /></QueryClientProvider>);
}

describe("TrustPage", () => {
  beforeEach(() => { commands.getTrustWorkspace.mockResolvedValue(previewTrustWorkspace()); });

  it("shows all F1-F5 gates, experiments, recent runs and manual governance", async () => {
    renderPage();
    expect(await screen.findByRole("heading", { name: "系统知道什么，也公开它还不知道什么。" })).toBeVisible();
    for (const framework of ["F1", "F2", "F3", "F4", "F5"]) expect(screen.getByText(framework)).toBeVisible();
    expect(screen.getByText(/contrastive_case 对照 worked_example/)).toBeVisible();
    expect(screen.getByRole("table", { name: "实验准入治理参数" })).toBeVisible();
  });

  it("has no detectable structural accessibility violations", async () => {
    const { container } = renderPage();
    await screen.findByRole("heading", { name: "系统知道什么，也公开它还不知道什么。" });
    const result = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(result.violations).toEqual([]);
  });
});
