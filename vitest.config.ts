import path from "node:path";
import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "apps/electron/src"),
    },
  },
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "electron",
          root: "apps/electron",
          environment: "jsdom",
          setupFiles: ["vitest.setup.ts"],
          include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
          exclude: ["e2e/**", "node_modules/**"],
        },
      },
      {
        extends: true,
        test: {
          name: "packages",
          environment: "node",
          include: ["packages/*/src/**/*.test.ts"],
          exclude: ["node_modules/**"],
        },
      },
    ],
  },
});
