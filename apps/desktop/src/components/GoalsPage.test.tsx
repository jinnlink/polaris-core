import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { GoalsPage } from "./GoalsPage";

const commands = vi.hoisted(() => ({ archiveGoal: vi.fn(), deleteGoal: vi.fn(), getGoalsWorkspace: vi.fn(), refreshGoal: vi.fn(), saveGoal: vi.fn() }));
vi.mock("../lib/commands", async (importOriginal) => ({ ...await importOriginal<typeof import("../lib/commands")>(), ...commands }));
const goal = { id: "goal-1", title: "掌握算法边界", description: "验证迁移", status: "active", deadline: "2026-09-01", pace: "steady", priority: 70, scope: { pack_ids: ["algorithms"], dimension_keys: [], concept_ids: [] }, overall_progress: 0.5, dimensions: [{ id: "dim-1", dimension_key: "mastery", display_name: "平均掌握度", metric_type: "mastery_mean", current_value: 0.4, target_value: 0.8, weight: 1, progress: 0.5 }], milestones: [{ id: "m-1", title: "稳定掌握", status: "pending", reached_at: null, dimension_key: "mastery", threshold: 0.8, manual: false }] };
function renderPage() { const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } }); return render(<QueryClientProvider client={client}><MemoryRouter><GoalsPage /></MemoryRouter></QueryClientProvider>); }

describe("GoalsPage", () => {
  beforeEach(() => { vi.clearAllMocks(); commands.getGoalsWorkspace.mockResolvedValue({ generated_at: "2026-08-12", goals: [goal], selected_goal_id: "goal-1", actions: [{ id: "a-1", kind: "目标行动", title: "做一道边界题", detail: "目标范围内行动", route: "/practice", concept_id: "dijkstra", expected_success: 0.7 }] }); commands.saveGoal.mockResolvedValue({ goal_id: "goal-1", effect: "updated", message: "目标已更新" }); commands.archiveGoal.mockResolvedValue({ goal_id: "goal-1", effect: "archived", message: "已归档" }); commands.deleteGoal.mockResolvedValue({ goal_id: "goal-1", effect: "deleted", message: "已删除" }); commands.refreshGoal.mockResolvedValue({ goal_id: "goal-1", effect: "refreshed", message: "已刷新" }); });
  it("shows dimensions milestones and two-to-three scoped actions", async () => { renderPage(); expect(await screen.findByRole("heading", { name: "掌握算法边界" })).toBeVisible(); expect(screen.getByText("0.40 / 0.80")).toBeVisible(); expect(screen.getByText("稳定掌握")).toBeVisible(); expect(screen.getByRole("link", { name: /做一道边界题/ })).toBeVisible(); });
  it("keeps archive distinct from confirmed delete and supports editing", async () => { renderPage(); await screen.findByRole("heading", { name: "掌握算法边界" }); fireEvent.click(screen.getByRole("button", { name: "编辑" })); fireEvent.change(screen.getByLabelText("标题"), { target: { value: "更新后的目标" } }); fireEvent.click(screen.getByRole("button", { name: "保存目标" })); await waitFor(() => { expect(commands.saveGoal).toHaveBeenCalled(); expect(commands.saveGoal.mock.calls[0]?.[0]).toEqual(expect.objectContaining({ title: "更新后的目标" })); }); });
});
