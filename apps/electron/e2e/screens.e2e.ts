import { test, expect } from "@playwright/test";
import { electronLauncher } from "./electron-test-utils";

test.describe("MVP Screens 渲染", () => {
  test("应渲染全部 12 个 Screen Panel", async () => {
    const { electronApp, page } = await electronLauncher();

    const panels = page.locator("article.screenPanel");
    await expect(panels).toHaveCount(13);

    await electronApp.close();
  });

  test("DaemonStatus Panel 应包含状态文本", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=DaemonStatus"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("WorkspaceShell Panel 应包含工作区信息", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=WorkspaceShell"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("ImportQueue Panel 应显示导入队列", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=ImportQueue"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("WikiReader Panel 应显示 wiki 标题", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=WikiReader"),
    });
    await expect(panel).toBeVisible();

    const heading = panel.locator("h2");
    await expect(heading).not.toBeEmpty();

    await electronApp.close();
  });

  test("SearchResults Panel 应显示搜索结果", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=SearchResults"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("Answer Panel 应显示回答文本", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=Answer"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("CitationPreview Panel 应显示引用预览", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=CitationPreview"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("PipelineDryRun Panel 应显示管道信息", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=PipelineDryRun"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("MemoryBrowser Panel 应显示笔记列表", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=MemoryBrowser"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("SessionSearch Panel 应显示会话搜索", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=SessionSearch"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("ProjectNoteEditor Panel 应显示项目笔记", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=ProjectNoteEditor"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("OutboxViewer Panel 应显示出站队列", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=OutboxViewer"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });

  test("ChannelStatus Panel 应显示通道状态", async () => {
    const { electronApp, page } = await electronLauncher();

    const panel = page.locator("article.screenPanel", {
      has: page.locator("text=ChannelStatus"),
    });
    await expect(panel).toBeVisible();

    await electronApp.close();
  });
});
