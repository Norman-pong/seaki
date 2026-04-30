import { defineConfig } from "vite";

export default defineConfig({
  build: {
    lib: {
      entry: {
        main: "./src/electron/main.ts",
        preload: "./src/electron/preload.ts",
      },
      formats: ["cjs"],
      fileName: (_format, entryName) => `${entryName}.cjs`,
    },
    outDir: "dist-electron",
    emptyOutDir: true,
    rollupOptions: {
      external: ["electron", "node:path"],
    },
  },
});
