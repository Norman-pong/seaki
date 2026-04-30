import { test, expect } from "./electron-test-utils";

test.describe("Approval Diff 工作流", () => {
  test("应渲染 Approval 工具栏", async ({ page }) => {
    const toolbar = page.locator("[aria-label='approval actions']");
    await expect(toolbar).toBeVisible();
  });

  test("应渲染 Approval Grid 包含 Source 和 Patch diff", async ({ page }) => {
    const grid = page.locator("[aria-label='approval diff']");
    await expect(grid).toBeVisible();
    const diffBlock = grid.locator("[aria-label='patch diff']");
    await expect(diffBlock).toBeVisible();
  });

  test("应渲染 Claims Panel 包含 claim 列表", async ({ page }) => {
    const claimsPanel = page.locator("[aria-labelledby='claims-title']");
    await expect(claimsPanel).toBeVisible();
    const title = claimsPanel.locator("h2");
    await expect(title).toHaveText("Citation validation / risk / taint");
  });

  test("应显示 approval result counts", async ({ page }) => {
    const statusStrip = page.locator("[aria-label='approval result counts']");
    await expect(statusStrip).toBeVisible();
  });
});
