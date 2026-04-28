import { describe, expect, it } from "vitest";

import { createMockTransportClient } from "@seaki/transport";

import { bootApp, initWorkspaceShell, prepareUserSelectedFile } from "./index";

describe("domain use case shells", () => {
  it("maps daemon heartbeat into AppBoot state", async () => {
    const transport = createMockTransportClient({
      heartbeat: {
        checkedAt: "2026-04-28T00:00:00.000Z",
        status: "daemon.ready",
        workspaceId: "ws_123",
      },
    });

    await expect(bootApp(transport)).resolves.toEqual({
      stage: "daemon.ready",
    });
  });

  it("creates a workspace shell without daemon-side effects", () => {
    expect(
      initWorkspaceShell({
        auditHead: "audit_1",
        currentRevision: "wiki_rev_0",
        rootUri: "file:///tmp/seaki",
        workspaceId: "ws_123",
      }),
    ).toMatchObject({
      state: {
        stage: "ready",
      },
      workspaceId: "ws_123",
    });
  });

  it("keeps selected file contents opaque to the frontend domain", () => {
    expect(
      prepareUserSelectedFile({
        declaredMime: "text/markdown",
        declaredSize: 128,
        displayName: "notes.md",
        opaqueFileRef: "electron://selection/1",
        platform: "electron",
        selectionId: "sel_1",
      }),
    ).toEqual({
      selection: {
        declaredMime: "text/markdown",
        declaredSize: 128,
        displayName: "notes.md",
        opaqueFileRef: "electron://selection/1",
        platform: "electron",
        selectionId: "sel_1",
      },
      state: {
        stage: "selected",
      },
    });
  });
});
