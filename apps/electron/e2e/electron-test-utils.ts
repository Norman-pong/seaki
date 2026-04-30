import { type ElectronApplication, type Page, _electron as electron } from "@playwright/test";
import path from "node:path";

export async function electronLauncher(): Promise<{
  electronApp: ElectronApplication;
  page: Page;
}> {
  const appPath = path.resolve(import.meta.dirname, "..");
  const mainPath = path.join(appPath, "dist-electron", "main.cjs");

  const electronApp = await electron.launch({
    args: [mainPath],
    cwd: appPath,
    env: {
      ...process.env,
      NODE_ENV: "test",
    },
  });

  const page = await electronApp.firstWindow();
  await page.waitForLoadState("networkidle");

  return { electronApp, page };
}
