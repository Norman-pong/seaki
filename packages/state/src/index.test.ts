import { describe, expect, it } from "vitest";

import {
  advanceImportState,
  createInitialRuntimeState,
  createSelectedImport,
  finishWorkspaceInit,
  reduceAppBoot,
  startWorkspaceInit,
} from "./index";

describe("frontend runtime state", () => {
  it("starts with connecting app boot and an uninitialized workspace", () => {
    expect(createInitialRuntimeState()).toMatchObject({
      appBoot: {
        stage: "daemon.connecting",
      },
      workspace: {
        stage: "uninitialized",
      },
    });
  });

  it("captures daemon readiness without inventing workspace facts", () => {
    expect(reduceAppBoot("daemon.ready")).toEqual({
      stage: "daemon.ready",
    });
  });

  it("moves workspace init into ready or degraded states", () => {
    expect(startWorkspaceInit({ stage: "uninitialized" })).toEqual({
      stage: "initializing",
    });
    expect(finishWorkspaceInit()).toEqual({
      stage: "ready",
    });
    expect(finishWorkspaceInit("index_stale")).toEqual({
      reason: "index_stale",
      stage: "degraded",
    });
  });

  it("keeps import transitions inside the documented M0 shell", () => {
    const selected = createSelectedImport();
    const grantRequested = advanceImportState(selected, "grant_requested");
    const granted = advanceImportState(grantRequested, "granted");

    expect(granted).toEqual({
      stage: "granted",
    });
    expect(advanceImportState(selected, "indexed")).toBe(selected);
  });
});
