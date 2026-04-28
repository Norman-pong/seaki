import { describe, expect, it } from "vitest";

import { createElectronAppModel } from "./appModel";

describe("createElectronAppModel", () => {
  it("binds Electron preview state to domain/state packages", () => {
    expect(createElectronAppModel()).toEqual({
      importStage: "selected",
      workspaceStage: "degraded",
      workspaceTitle: "ws_local_preview",
    });
  });
});
