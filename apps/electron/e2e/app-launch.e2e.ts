import { test, expect } from "./electron-test-utils";

test.describe("应用启动", () => {
  test("窗口标题应为 seaki", async ({ page }) => {
    const title = await page.title();
    expect(title).toBe("seaki");
  });

  test("应渲染会话历史侧边栏", async ({ page }) => {
    const sidebar = page.locator("[aria-label='session history']");
    await expect(sidebar).toBeVisible();
  });

  test("应渲染对话流主区域", async ({ page }) => {
    const chatPanel = page.locator("[aria-label='chat flow']");
    await expect(chatPanel).toBeVisible();
  });

  test("应渲染 Wiki 侧边栏", async ({ page }) => {
    const wikiSidebar = page.locator("[aria-label='wiki sidebar']");
    await expect(wikiSidebar).toBeVisible();
  });
});
