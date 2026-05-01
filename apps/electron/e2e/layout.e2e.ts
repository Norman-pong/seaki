import { test, expect } from "./electron-test-utils";

test.describe("新布局组件渲染", () => {
  test("会话侧边栏应包含多个会话项", async ({ page }) => {
    const sessionItems = page.locator(".session-item");
    await expect(sessionItems).toHaveCount(3);
  });

  test("对话流应包含消息气泡", async ({ page }) => {
    const messages = page.locator(".chat-message");
    await expect.poll(() => messages.count()).toBeGreaterThanOrEqual(2);
  });

  test("对话流应包含输入框", async ({ page }) => {
    const input = page.locator(".chat-textarea");
    await expect(input).toBeVisible();
  });

  test("Wiki 页面树应可见", async ({ page }) => {
    await page.locator("[data-tab='pages']").click();
    const tree = page.locator("[aria-label='wiki page tree']");
    await expect(tree).toBeVisible();
  });

  test("Wiki 页面树应包含层级节点", async ({ page }) => {
    await page.locator("[data-tab='pages']").click();
    const treeNodes = page.locator(".tree-node-row");
    await expect.poll(() => treeNodes.count()).toBeGreaterThanOrEqual(4);
  });

  test("Wiki 预览面板应可见", async ({ page }) => {
    await page.locator("[data-tab='pages']").click();
    const preview = page.locator("[aria-label='wiki page preview']");
    await expect(preview).toBeVisible();
  });

  test("对话流中应包含卡片", async ({ page }) => {
    const cards = page.locator(".chat-card");
    await expect.poll(() => cards.count()).toBeGreaterThanOrEqual(1);
  });
});
