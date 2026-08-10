import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import type {
  CommandError,
  MapWorkspaceQuery,
  MapWorkspaceSnapshot,
  PackSwitchReceipt,
  StatusSnapshot,
  TodaySnapshot,
  WindowModeReceipt,
} from "../contracts/core";
import { previewMapWorkspace } from "./mapPreview";

export const commandKeys = {
  status: ["core", "status"] as const,
  today: ["core", "today"] as const,
  map: (query: MapWorkspaceQuery) => ["core", "map", query] as const,
};

function isBrowserPreview() {
  return import.meta.env.DEV && !("__TAURI_INTERNALS__" in window);
}

function previewPacks() {
  return [
    { id: "algorithms", title: "算法基础 · 核心包", concept_count: 10_000, active: true, theta_mode: "shared" },
    { id: "rust", title: "Rust 掌握实验室", concept_count: 2_840, active: false, theta_mode: "isolated" },
  ];
}

export async function getStatus(): Promise<StatusSnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve({
      generated_at: new Date().toISOString(), current_pack: "algorithms", theta_mode: "shared",
      packs: previewPacks(), due_today: 3, phase_counts: [{ phase: "acquisition", count: 4 }, { phase: "transfer", count: 2 }], concepts: [],
    });
  }
  return invoke<StatusSnapshot>("status");
}

export async function getToday(): Promise<TodaySnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve({
      generated_at: new Date().toISOString(), current_pack: "algorithms", theta_mode: "shared", packs: previewPacks(),
      top_signal: { claim: "Dijkstra 能写出实现，但负权边与迁移推理仍不稳定。", confidence: 0.68, suggested_action: "先在地图看证据，再做一道边界变式。" },
      actions: [
        { id: "map-dijkstra", kind: "定位模糊", title: "看清 Dijkstra 的证据地形", detail: "从当前图谱确认低置信来源与两跳依赖。", route: "/map?concept=dijkstra", concept_id: "dijkstra", expected_success: 0.68 },
        { id: "practice-dijkstra", kind: "验证真懂", title: "完成负权边边界变式", detail: "用一道短练习验证迁移稳定性。", route: "/practice?concept=dijkstra", concept_id: "dijkstra", expected_success: 0.63 },
        { id: "rest", kind: "恢复", title: "今天先到这里", detail: "保留现场并隐藏到托盘。", route: null, concept_id: null, expected_success: null },
      ],
      notification_policy: { state_gate_passed: true, dominant_state: null, suppress_non_error: false },
    });
  }
  return invoke<TodaySnapshot>("today");
}

export async function getMapWorkspace(
  query: MapWorkspaceQuery,
): Promise<MapWorkspaceSnapshot> {
  if (isBrowserPreview()) {
    return Promise.resolve(previewMapWorkspace(query));
  }
  return invoke<MapWorkspaceSnapshot>("map_workspace", { query });
}

export async function switchPack(packId: string): Promise<PackSwitchReceipt> {
  if (isBrowserPreview()) return Promise.resolve({ active_pack: packId, theta_mode: packId === "rust" ? "isolated" : "shared" });
  return invoke<PackSwitchReceipt>("switch_pack", { packId });
}

export async function setWindowMode(
  mode: "compact" | "workspace",
): Promise<WindowModeReceipt> {
  if (isBrowserPreview()) return Promise.resolve({ mode });
  return invoke<WindowModeReceipt>("set_window_mode", { mode });
}

export async function hideToTray(): Promise<void> {
  if (isBrowserPreview()) return Promise.resolve();
  return invoke("hide_to_tray");
}

export function normalizeCommandError(error: unknown): CommandError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    "retryable" in error
  ) {
    return error as CommandError;
  }
  return {
    code: "desktop_error",
    message: error instanceof Error ? error.message : String(error),
    retryable: false,
  };
}

export async function openExternal(url: string): Promise<void> {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:") {
    throw new Error("只允许打开 HTTPS 外链");
  }
  await openUrl(parsed.toString());
}
