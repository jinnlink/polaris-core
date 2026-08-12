import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";

export type UpdateCheckResult =
  | { status: "disabled"; reason: string }
  | { status: "current" }
  | { status: "available"; version: string; date: string | null; notes: string | null };

export type UpdateProgress = {
  downloaded: number;
  total: number | null;
  finished: boolean;
};

let pendingUpdate: Update | null = null;

export function updaterEnabled(): boolean {
  return !import.meta.env.DEV && import.meta.env.VITE_POLARIS_UPDATER_ENABLED === "true";
}

export async function checkForUpdate(): Promise<UpdateCheckResult> {
  if (!updaterEnabled()) {
    pendingUpdate = null;
    return {
      status: "disabled",
      reason: "当前是开发或未签名构建；只有正式发行版会连接签名更新通道。",
    };
  }
  pendingUpdate = await check();
  if (!pendingUpdate) return { status: "current" };
  return {
    status: "available",
    version: pendingUpdate.version,
    date: pendingUpdate.date ?? null,
    notes: pendingUpdate.body ?? null,
  };
}

export async function installCheckedUpdate(
  onProgress: (progress: UpdateProgress) => void,
): Promise<void> {
  if (!pendingUpdate) throw new Error("请先检查并确认可用更新。");
  let downloaded = 0;
  let total: number | null = null;
  await pendingUpdate.downloadAndInstall((event: DownloadEvent) => {
    if (event.event === "Started") total = event.data.contentLength ?? null;
    if (event.event === "Progress") downloaded += event.data.chunkLength;
    onProgress({ downloaded, total, finished: event.event === "Finished" });
  });
}

export const productionUpdater = {
  check: checkForUpdate,
  install: installCheckedUpdate,
};

export type UpdaterAdapter = typeof productionUpdater;
