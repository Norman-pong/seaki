import { describe, expect, it } from "vitest";

import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";

import {
  advanceImportState,
  createInitialRuntimeState,
  createRuntimeStore,
  createSelectedImport,
  finishWorkspaceInit,
  failWorkspaceInit,
  reduceAppBoot,
  reduceRuntimeState,
  startWorkspaceInit,
} from "./index";
import type { FrontendRuntimeEvent, ImportStage } from "./index";

function event(
  seq: number,
  eventType: string,
  payload: Record<string, unknown> = {},
): FrontendRuntimeEvent {
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

function dispatchImportStages(stages: readonly ImportStage[]) {
  const store = createRuntimeStore();

  stages.forEach((stage, index) => {
    store.dispatch(
      event(index + 1, "import.stage.changed", {
        stage,
      }),
    );
  });

  return store.getSnapshot();
}

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
    expect(finishWorkspaceInit({ reason: "index_stale" })).toEqual({
      reason: "index_stale",
      stage: "degraded",
    });
    expect(failWorkspaceInit()).toEqual({
      stage: "error",
    });
  });

  it("keeps import transitions inside the documented M0 shell", () => {
    const selected = createSelectedImport();
    const grantRequested = advanceImportState(selected, "grant_requested");
    const granted = advanceImportState(grantRequested, "granted");

    expect(granted).toEqual({
      committed: false,
      stage: "granted",
    });
    expect(advanceImportState(selected, "indexed")).toBe(selected);
  });

  it("applies mock daemon envelopes to AppBoot, Workspace, and Import state machines", () => {
    const store = createRuntimeStore();

    store.dispatch(event(1, "daemon.ready"));
    store.dispatch(event(2, "workspace.init.started"));
    store.dispatch(
      event(3, "workspace.init.completed", {
        workspace: {
          audit_head: "audit_1",
          current_revision: "wiki_rev_0",
          index_status: {
            last_good_revision: null,
            stale_reason: null,
            state: "stale",
            updated_at: null,
          },
          root_uri: "file:///tmp/seaki",
          state: "ready",
          workspace_id: "ws_1",
        },
      }),
    );
    store.dispatch(
      event(4, "import.stage.changed", {
        stage: "selected",
      }),
    );
    store.dispatch(
      event(5, "import.stage.changed", {
        stage: "grant_requested",
      }),
    );

    expect(store.getSnapshot()).toMatchObject({
      appBoot: {
        stage: "daemon.ready",
      },
      imports: [
        {
          committed: false,
          stage: "grant_requested",
          taskId: "task_import_1",
        },
      ],
      lastSeq: 5,
      workspace: {
        stage: "ready",
      },
    });
  });

  it("does not treat draft or temporary import events as committed state", () => {
    const beforeCommit = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "import.stage.changed", {
        stage: "selected",
      }),
    );
    const afterDraftCommit = reduceRuntimeState(
      {
        ...beforeCommit,
        imports: [
          {
            committed: false,
            stage: "approval_pending",
            taskId: "task_import_1",
            workspaceId: "ws_1",
          },
        ],
      },
      event(2, "import.stage.changed", {
        draft: true,
        stage: "committed",
      }),
    );

    expect(afterDraftCommit.imports[0]).toMatchObject({
      committed: false,
      stage: "approval_pending",
    });
  });

  it("maps workspace failure envelopes into the Workspace error state", () => {
    const snapshot = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "workspace.init.failed", {
        error: "daemon unavailable",
      }),
    );

    expect(snapshot).toMatchObject({
      tasks: {
        task_workspace_1: {
          kind: "workspace.init",
          lastEventSeq: 1,
          stage: "error",
        },
      },
      workspace: {
        stage: "error",
      },
    });
  });

  it("replays the complete import happy path from selected to indexed", () => {
    const stages: readonly ImportStage[] = [
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "parsed",
      "patch_proposed",
      "approval_pending",
      "committed",
      "indexed",
    ];

    const snapshot = dispatchImportStages(stages);

    expect(snapshot.imports).toHaveLength(1);
    expect(snapshot.imports[0]).toMatchObject({
      committed: true,
      stage: "indexed",
      taskId: "task_import_1",
      workspaceId: "ws_1",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      kind: "source.ingest",
      lastEventSeq: stages.length,
      stage: "indexed",
    });
  });

  it("moves grant_requested imports to capability_denied when capability is refused", () => {
    const snapshot = dispatchImportStages(["selected", "grant_requested", "capability_denied"]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: false,
      stage: "capability_denied",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "capability_denied",
    });
  });

  it("allows partial parse results to downgrade to failed", () => {
    const snapshot = dispatchImportStages([
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "partial",
      "failed",
    ]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: false,
      stage: "failed",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "failed",
    });
  });

  it("keeps approval_pending imports uncommitted when approval is denied", () => {
    const snapshot = dispatchImportStages([
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "parsed",
      "patch_proposed",
      "approval_pending",
      "denied",
    ]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: false,
      stage: "denied",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "denied",
    });
  });

  it("marks committed imports as index_stale without clearing committed state", () => {
    const snapshot = dispatchImportStages([
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "parsed",
      "patch_proposed",
      "approval_pending",
      "committed",
      "index_stale",
    ]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: true,
      stage: "index_stale",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "index_stale",
    });
  });
});
