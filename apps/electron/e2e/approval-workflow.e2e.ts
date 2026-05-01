import { test, expect } from "./electron-test-utils";

test.describe("Approval Diff 工作流", () => {
  test("应渲染 Approval 控件", async ({ page }) => {
    await page.locator("[data-tab='review']").click();
    const widget = page.locator("[aria-label='approval widget']");
    await expect(widget).toBeVisible();
  });

  test("Approval 控件应包含 claim 列表", async ({ page }) => {
    await page.locator("[data-tab='review']").click();
    const claims = page.locator(".approval-claim-item");
    await expect.poll(() => claims.count()).toBeGreaterThanOrEqual(1);
  });

  test("应显示批量批准按钮", async ({ page }) => {
    await page.locator("[data-tab='review']").click();
    const button = page.locator("button", { hasText: /批量批准/ });
    await expect(button).toBeVisible();
  });
});
