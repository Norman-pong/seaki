import { test, expect } from "@playwright/test";
import { electronLauncher } from "./electron-test-utils";

test.describe("应用启动", () => {
  test("窗口标题应为 seaki", async () => {
    const { electronApp, page } = await electronLauncher();

    const title = await page.title();
    expect(title).toBe("seaki");

    await electronApp.close();
  });

  test("应渲染 workspace status 区域", async () => {
    const { electronApp, page } = await electronLauncher();

    const status = page.locator("[aria-label='workspace status']");
    await expect(status).toBeVisible();

    await electronApp.close();
  });

  test("应渲染 MVP screen grid", async () => {
    const { electronApp, page } = await electronLauncher();

    const grid = page.locator("[aria-label='electron mvp screens']");
    await expect(grid).toBeVisible();

    await electronApp.close();
  });
});
