import { test, expect } from "./electron-test-utils";

test.describe("应用启动", () => {
  test("窗口标题应为 seaki", async ({ page }) => {
    const title = await page.title();
    expect(title).toBe("seaki");
  });

  test("应渲染 workspace status 区域", async ({ page }) => {
    const status = page.locator("[aria-label='workspace status']");
    await expect(status).toBeVisible();
  });

  test("应渲染 MVP screen grid", async ({ page }) => {
    const grid = page.locator("[aria-label='electron mvp screens']");
    await expect(grid).toBeVisible();
  });
});
