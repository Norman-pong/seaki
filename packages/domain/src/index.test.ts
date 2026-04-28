import { describe, expect, it } from "vitest";

import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";
import { createMockTransportClient } from "@seaki/transport";
import type { FrontendTransportEvent } from "@seaki/transport";

import {
  bootApp,
  createDomainClient,
  createDomainRuntime,
  initWorkspaceShell,
  prepareUserSelectedFile,
} from "./index";

function event(
  seq: number,
  eventType: string,
  payload: Record<string, unknown> = {},
): FrontendTransportEvent {
  return {
    actor_id: "test",
    causation_id: `cause_${seq}`,
    correlation_id: "corr_1",
    event_id: `evt_${seq}`,
    idempotency_key: `idem_${seq}`,
    occurred_at: `2026-04-28T00:00:0${seq}.000Z`,
    payload,
    payload_schema_hash: SCHEMA_HASH,
    replayable: true,
    revision: "wiki_rev_0",
    schema_version: SCHEMA_VERSION,
    scope: "workspace:ws_1",
    seq,
    task_id: eventType.startsWith("import.") ? "task_import_1" : "task_workspace_1",
    transaction_id: `tx_${seq}`,
    type: eventType,
    workspace_id: "ws_1",
  };
}

describe("domain use case shells", () => {
  it("maps daemon ready events into AppBoot state", async () => {
    const transport = createMockTransportClient({
      events: [event(1, "daemon.ready")],
    });

    await expect(bootApp(transport)).resolves.toEqual({
      stage: "daemon.ready",
    });
  });

  it("replays daemon events through the domain runtime store", async () => {
    const runtime = createDomainRuntime(
      createMockTransportClient({
        events: [
          event(1, "daemon.ready"),
          event(2, "workspace.init.completed"),
          event(3, "import.stage.changed", {
            stage: "selected",
          }),
        ],
      }),
    );

    await runtime.replay(0);

    expect(runtime.store.getSnapshot()).toMatchObject({
      appBoot: {
        stage: "daemon.ready",
      },
      imports: [
        {
          stage: "selected",
        },
      ],
      workspace: {
        stage: "ready",
      },
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
        committed: false,
        stage: "selected",
      },
    });
  });

  it("sends every M0 use case through transport request methods", async () => {
    const transport = createMockTransportClient();
    const client = createDomainClient(transport);

    await client.workspace.init({
      rootUri: "file:///tmp/seaki",
      workspaceId: "ws_1",
    });
    await client.files.prepareUserSelected({
      declaredMime: "text/markdown",
      declaredSize: 10,
      displayName: "notes.md",
      opaqueFileRef: "electron://selection/1",
      platform: "electron",
      selectionId: "sel_1",
    });
    await client.source.ingestSelectedFile({
      selectionId: "sel_1",
      workspaceId: "ws_1",
    });
    await client.approval.reviewPatch({
      patchId: "patch_1",
      workspaceId: "ws_1",
    });
    await client.approval.decide({
      approvalId: "approval_1",
      decision: "approve",
      workspaceId: "ws_1",
    });
    await client.wiki.readPage({
      pageId: "page_1",
      workspaceId: "ws_1",
    });
    await client.search.query({
      query: "citation",
      workspaceId: "ws_1",
    });
    await client.citation.resolve({
      citationId: "cite_1",
      workspaceId: "ws_1",
    });

    expect(transport.getRequests().map((request) => request.method)).toEqual([
      "workspace.init",
      "files.prepareUserSelected",
      "source.ingestSelectedFile",
      "approval.reviewPatch",
      "approval.decide",
      "wiki.readPage",
      "search.query",
      "citation.resolve",
    ]);
  });
});
