import { listen } from "@tauri-apps/api/event";
import type { QueryClient } from "@tanstack/react-query";

export const DATA_CHANGED_EVENT = "polaris://data-changed";

export async function installCoreEventRefresh(
  queryClient: QueryClient,
): Promise<() => void> {
  return listen(DATA_CHANGED_EVENT, () => {
    void queryClient.invalidateQueries({ queryKey: ["core"] });
  });
}
