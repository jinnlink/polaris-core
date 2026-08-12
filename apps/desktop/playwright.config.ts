import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/release",
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["line"], ["html", { open: "never" }]] : "line",
  expect: { timeout: 8_000 },
  use: {
    baseURL: "http://127.0.0.1:5180",
    channel: process.env.POLARIS_SYSTEM_CHROME === "1" ? "chrome" : undefined,
    colorScheme: "light",
    locale: "zh-CN",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "pnpm exec vite build --mode release-test && pnpm exec vite preview --host 127.0.0.1 --port 5180",
    port: 5180,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
  projects: [
    {
      name: "windows-x64",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1280, height: 800 },
      },
    },
    {
      name: "windows-200-percent",
      testMatch: /visual\.spec\.ts/,
      use: {
        browserName: "chromium",
        viewport: { width: 420, height: 640 },
        deviceScaleFactor: 2,
      },
    },
  ],
});
