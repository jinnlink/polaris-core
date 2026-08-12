import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

const routes = ["/", "/map", "/practice", "/inbox", "/profile", "/goals", "/reports", "/trust", "/settings"];

for (const route of routes) {
  test(`axe: ${route}`, async ({ page }) => {
    await page.goto(`/#${route}`);
    await expect(page.locator("main h1")).toBeVisible();
    const result = await new AxeBuilder({ page }).analyze();
    expect(result.violations, JSON.stringify(result.violations, null, 2)).toEqual([]);
  });
}

test("keyboard: skip link reaches the workspace and map remains operable", async ({ page }) => {
  await page.goto("/#/");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "跳到主要内容" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("#main-content")).toBeFocused();

  await page.goto("/#/map");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await page.getByRole("button", { name: "键盘浏览" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByLabel("可键盘访问的地图节点").getByRole("button").first()).toBeVisible();
});

test("Windows accessibility media preferences are honored", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce", forcedColors: "active" });
  await page.goto("/#/settings");
  await expect(page.locator("main h1")).toBeVisible();
  expect(await page.evaluate(() => matchMedia("(prefers-reduced-motion: reduce)").matches)).toBe(true);
  expect(await page.evaluate(() => matchMedia("(forced-colors: active)").matches)).toBe(true);
  const duration = await page.locator(".primary-action").first().evaluate((element) => getComputedStyle(element).transitionDuration);
  expect(["0.01ms", "1e-05s"]).toContain(duration);
});

test("release shell and 10k map stay inside the interactive budget", async ({ page }) => {
  const coldStarted = Date.now();
  await page.goto("/#/");
  await expect(page.locator("main h1")).toBeVisible();
  expect(Date.now() - coldStarted).toBeLessThan(2_500);

  const mapStarted = Date.now();
  await page.goto("/#/map");
  await page.getByRole("button", { name: "全局总览" }).click();
  await expect(page.getByText("5 组聚合")).toBeVisible();
  const packConceptCounts = await page.locator(".atlas-global__row[data-kind='pack']").evaluateAll((rows) => rows.map((row) => Number(row.getAttribute("data-concept-count"))));
  expect(packConceptCounts.reduce((total, count) => total + count, 0)).toBe(10_000);
  expect(Date.now() - mapStarted).toBeLessThan(2_500);
  expect(await page.locator("canvas").count()).toBeLessThanOrEqual(1);
});
