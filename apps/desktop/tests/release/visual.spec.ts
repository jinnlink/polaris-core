import { expect, test } from "@playwright/test";

for (const [name, route] of [["today", "/"], ["map", "/map"], ["settings", "/settings"]] as const) {
  test(`visual: ${name}`, async ({ page }) => {
    await page.goto(`/#${route}`);
    await expect(page.locator("main h1")).toBeVisible();
    await expect(page).toHaveScreenshot(`${name}.png`, {
      animations: "disabled",
      fullPage: true,
      maxDiffPixelRatio: 0.01,
    });
  });
}
