import { normalizeCommandError } from "./commands";

describe("command error envelope", () => {
  it("preserves the Rust error envelope", () => {
    const error = { code: "core_error", message: "broken", retryable: false };
    expect(normalizeCommandError(error)).toEqual(error);
  });

  it("normalizes unknown invoke failures", () => {
    expect(normalizeCommandError(new Error("offline"))).toEqual({
      code: "desktop_error",
      message: "offline",
      retryable: false,
    });
  });
});
