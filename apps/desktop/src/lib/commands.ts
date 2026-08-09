import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import type { CommandError, StatusSnapshot } from "../contracts/core";

export const commandKeys = {
  status: ["core", "status"] as const,
};

export async function getStatus(): Promise<StatusSnapshot> {
  return invoke<StatusSnapshot>("status");
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
