import { describe, expect, it } from "vitest";

import { createElectronAppModel } from "./appModel";

describe("createElectronAppModel", () => {
  it("binds Electron preview state to domain/state packages", async () => {
    await expect(createElectronAppModel()).resolves.toEqual({
      importStage: "selected",
      workspaceStage: "ready",
      workspaceTitle: "ws_local_preview",
    });
  });
});
