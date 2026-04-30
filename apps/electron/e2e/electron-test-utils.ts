import { test as base, expect, type ElectronApplication, type Page } from "@playwright/test";
import { _electron as electron } from "@playwright/test";
import path from "node:path";

async function launchElectronApp(): Promise<ElectronApplication> {
  const appPath = path.resolve(import.meta.dirname, "..");
  const mainPath = path.join(appPath, "dist-electron", "main.cjs");

  return electron.launch({
    args: [mainPath],
    cwd: appPath,
    env: {
      ...process.env,
      NODE_ENV: "test",
    },
  });
}

export const test = base.extend<{
  electronApp: ElectronApplication;
  page: Page;
}>({
  electronApp: [
    async (
      // oxlint-disable-next-line no-empty-pattern
      {},
      use,
    ) => {
      const app = await launchElectronApp();
      await use(app);
      await app.close();
    },
    { scope: "test" },
  ],

  page: [
    async ({ electronApp }, use) => {
      const page = await electronApp.firstWindow();
      await page.waitForLoadState("networkidle");
      await use(page);
    },
    { scope: "test" },
  ],
});

export { expect };
