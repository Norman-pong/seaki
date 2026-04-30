import { test, expect } from "./electron-test-utils";

test.describe("SessionSearch Screen 交互", () => {
  test("应显示候选会话列表", async ({ page }) => {
    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=SessionSearch"),
    });
    await expect(panel).toBeVisible();

    const candidates = panel.locator("section.searchItem");
    const count = await candidates.count();
    expect(count).toBeGreaterThanOrEqual(1);
  });

  test("应包含 transcript 输入区域", async ({ page }) => {
    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=SessionSearch"),
    });
    const textarea = panel.locator("textarea");
    const hasTextarea = (await textarea.count()) > 0;
    expect(hasTextarea).toBe(true);
  });

  test("应包含 Redact Session 按钮", async ({ page }) => {
    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=SessionSearch"),
    });
    const button = panel.locator("button", { hasText: /Redact|脱敏/ });
    await expect(button).toBeVisible();
  });
});
