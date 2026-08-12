import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import axe from "axe-core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { previewSettingsWorkspace } from "../lib/settingsPreview";
import { SettingsPage } from "./SettingsPage";

const commands = vi.hoisted(() => ({ exportProfile: vi.fn(), getSettingsWorkspace: vi.fn(), resetProfile: vi.fn(), submitProfileMeasurement: vi.fn(), updateAiProfile: vi.fn(), updateProfileSettings: vi.fn() }));
vi.mock("../lib/commands", async (importOriginal) => ({ ...await importOriginal<typeof import("../lib/commands")>(), ...commands }));
function renderPage() { const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } }); return render(<QueryClientProvider client={client}><SettingsPage /></QueryClientProvider>); }

describe("SettingsPage", () => {
  beforeEach(() => { vi.clearAllMocks(); commands.getSettingsWorkspace.mockResolvedValue(previewSettingsWorkspace()); commands.updateProfileSettings.mockResolvedValue({ effect: "profile_settings_updated", message: "已保存" }); commands.updateAiProfile.mockResolvedValue({ effect: "ai_profile_updated", message: "已保存" }); commands.resetProfile.mockResolvedValue({ effect: "profile_reset", message: "已清除" }); });
  it("shows real privacy degradation and writes profile controls", async () => { renderPage(); expect(await screen.findByRole("heading", { name: "控制画像、AI 与数据边界。" })).toBeVisible(); expect(screen.getByText("POLARIS_TIER0_ONLY=1")).toBeVisible(); fireEvent.click(screen.getByRole("checkbox", { name: /启用本地画像/ })); await waitFor(() => { expect(commands.updateProfileSettings).toHaveBeenCalled(); }); expect(commands.updateProfileSettings.mock.calls[0]?.[0]).toEqual(expect.objectContaining({ enabled: false })); });
  it("has no detectable structural accessibility violations", async () => { const { container } = renderPage(); await screen.findByRole("heading", { name: "控制画像、AI 与数据边界。" }); const result = await axe.run(container, { rules: { "color-contrast": { enabled: false } } }); expect(result.violations).toEqual([]); });
});
