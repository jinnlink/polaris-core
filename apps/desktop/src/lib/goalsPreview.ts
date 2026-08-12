import type { GoalEditorInput, GoalMutationReceipt, GoalView, GoalWorkspaceSnapshot } from "../contracts/core";

const previewGoals: GoalView[] = [{
  id: "algorithm-boundaries", title: "掌握算法边界", description: "用迁移题验证复杂度与失效条件。", status: "active", deadline: "2026-09-01", pace: "steady", priority: 70,
  scope: { pack_ids: ["algorithms"], dimension_keys: [], concept_ids: ["dijkstra"] }, overall_progress: 0.58,
  dimensions: [{ id: "dim-mastery", dimension_key: "mastery", display_name: "平均掌握度", metric_type: "mastery_mean", current_value: 0.46, target_value: 0.8, weight: 1, progress: 0.58 }],
  milestones: [{ id: "milestone-stable", title: "达到稳定掌握", status: "pending", reached_at: null, dimension_key: "mastery", threshold: 0.8, manual: false }],
}];

export function previewGoalsWorkspace(selectedGoalId: string | null): GoalWorkspaceSnapshot {
  const selected = selectedGoalId ?? previewGoals.find((goal) => goal.status === "active")?.id ?? null;
  const goal = previewGoals.find((item) => item.id === selected);
  return { generated_at: new Date().toISOString(), goals: structuredClone(previewGoals), selected_goal_id: selected, actions: goal?.status === "active" ? [
    { id: "goal-practice", kind: "目标行动", title: "验证 Dijkstra 的负权边界", detail: "目标范围内最值得完成的迁移题。", route: "/practice?concept=dijkstra", concept_id: "dijkstra", expected_success: 0.66 },
    { id: "goal-map", kind: "检查依赖", title: "查看目标知识地形", detail: "确认先修与低置信来源。", route: "/map?concept=dijkstra", concept_id: "dijkstra", expected_success: null },
  ] : [] };
}

export function previewSaveGoal(input: GoalEditorInput): GoalMutationReceipt {
  const index = previewGoals.findIndex((goal) => goal.id === input.id);
  const existing = index >= 0 ? previewGoals[index] : null;
  const dimensions = input.dimensions.map((dimension) => ({ ...dimension, current_value: existing?.dimensions.find((item) => item.id === dimension.id)?.current_value ?? 0, progress: existing?.dimensions.find((item) => item.id === dimension.id)?.progress ?? 0 }));
  const milestones = input.milestones.map((milestone) => { const previous = existing?.milestones.find((item) => item.id === milestone.id); return { ...milestone, status: previous?.status ?? "pending", reached_at: previous?.reached_at ?? null }; });
  const next: GoalView = { ...input, overall_progress: existing?.overall_progress ?? 0, dimensions, milestones };
  if (index >= 0) previewGoals[index] = next; else previewGoals.unshift(next);
  return { goal_id: input.id, effect: index >= 0 ? "updated" : "created", message: index >= 0 ? "目标已更新。" : "目标已创建。" };
}

export function previewGoalMutation(goalId: string, effect: "refreshed" | "archived" | "deleted"): GoalMutationReceipt {
  const index = previewGoals.findIndex((goal) => goal.id === goalId);
  if (index < 0) throw new Error("找不到这个目标");
  if (effect === "deleted") previewGoals.splice(index, 1);
  if (effect === "archived" && previewGoals[index]) previewGoals[index].status = "archived";
  return { goal_id: goalId, effect, message: effect === "refreshed" ? "已从掌握证据刷新目标进度。" : effect === "archived" ? "目标已归档，历史进度仍保留。" : "目标及其维度、里程碑已删除。" };
}
