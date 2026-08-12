import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { UpdateProgress, UpdaterAdapter } from "../lib/updater";
import { UpdatePanel } from "./UpdatePanel";

describe("UpdatePanel", () => {
  it("explains why unsigned builds do not check for updates", async () => {
    const adapter: UpdaterAdapter = {
      check: vi.fn().mockResolvedValue({ status: "disabled", reason: "开发构建不启用签名更新。" }),
      install: vi.fn(),
    };
    render(<UpdatePanel adapter={adapter} />);
    expect(await screen.findByText("自动更新已禁用")).toBeVisible();
    expect(screen.getByText("开发构建不启用签名更新。")).toBeVisible();
    expect(adapter.install).not.toHaveBeenCalled();
  });

  it("shows signed release notes and requires an explicit install action", async () => {
    const adapter: UpdaterAdapter = {
      check: vi.fn().mockResolvedValue({ status: "available", version: "0.2.0", date: "2026-08-12", notes: "改进恢复体验。" }),
      install: vi.fn((onProgress: (progress: UpdateProgress) => void) => {
        onProgress({ downloaded: 10, total: 10, finished: true });
        return Promise.resolve();
      }),
    };
    render(<UpdatePanel adapter={adapter} />);
    expect(await screen.findByRole("heading", { name: "Polaris 0.2.0" })).toBeVisible();
    expect(screen.getByText("改进恢复体验。")).toBeVisible();
    expect(adapter.install).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "确认下载并安装 0.2.0" }));
    await waitFor(() => { expect(adapter.install).toHaveBeenCalledTimes(1); });
    expect(await screen.findByText("验证完成，正在交给 Windows 安装…")).toBeVisible();
  });

  it("keeps the current version usable when the channel fails", async () => {
    const adapter: UpdaterAdapter = {
      check: vi.fn().mockRejectedValue(new Error("网络不可用")),
      install: vi.fn(),
    };
    render(<UpdatePanel adapter={adapter} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("当前版本仍可继续使用");
    fireEvent.click(screen.getByRole("button", { name: "重新检查" }));
    await waitFor(() => { expect(adapter.check).toHaveBeenCalledTimes(2); });
  });
});
