import type { QueryClient } from "@tanstack/react-query";
import { vi } from "vitest";

const { listenMock } = vi.hoisted(() => ({ listenMock: vi.fn() }));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

import { DATA_CHANGED_EVENT, installCoreEventRefresh } from "./events";

describe("Tauri data refresh event", () => {
  it("invalidates every Core query and returns the unlisten function", async () => {
    const unlisten = vi.fn();
    const invalidateQueries = vi.fn();
    listenMock.mockImplementationOnce(
      (_event: string, handler: () => void) => {
        handler();
        return Promise.resolve(unlisten);
      },
    );
    const queryClient = { invalidateQueries } as unknown as QueryClient;

    const dispose = await installCoreEventRefresh(queryClient);

    expect(listenMock).toHaveBeenCalledWith(DATA_CHANGED_EVENT, expect.any(Function));
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ["core"] });
    expect(dispose).toBe(unlisten);
  });
});
