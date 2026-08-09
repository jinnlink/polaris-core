import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import type {
  CommandError,
  PackSwitchReceipt,
  StatusSnapshot,
  TodaySnapshot,
  WindowModeReceipt,
} from "../contracts/core";

export const commandKeys = {
  status: ["core", "status"] as const,
  today: ["core", "today"] as const,
};

export async function getStatus(): Promise<StatusSnapshot> {
  return invoke<StatusSnapshot>("status");
}

export async function getToday(): Promise<TodaySnapshot> {
  return invoke<TodaySnapshot>("today");
}

export async function switchPack(packId: string): Promise<PackSwitchReceipt> {
  return invoke<PackSwitchReceipt>("switch_pack", { packId });
}

export async function setWindowMode(
  mode: "compact" | "workspace",
): Promise<WindowModeReceipt> {
  return invoke<WindowModeReceipt>("set_window_mode", { mode });
}

export async function hideToTray(): Promise<void> {
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
