import { createDomainRuntime } from "@seaki/domain";
import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";
import { createMockTransportClient } from "@seaki/transport";
import type { FrontendTransportEvent } from "@seaki/transport";

export interface ElectronAppModel {
  readonly importStage: string;
  readonly workspaceStage: string;
  readonly workspaceTitle: string;
}

function previewEvent(
  seq: number,
  eventType: string,
  payload: Record<string, unknown> = {},
): FrontendTransportEvent {
  return {
    actor_id: "electron-preview",
    causation_id: `cause_${seq}`,
    correlation_id: "preview_boot",
    event_id: `evt_preview_${seq}`,
    idempotency_key: `preview_${seq}`,
    occurred_at: `2026-04-28T00:00:0${seq}.000Z`,
    payload,
    payload_schema_hash: SCHEMA_HASH,
    replayable: true,
    revision: "wiki_rev_0",
    schema_version: SCHEMA_VERSION,
    scope: "workspace:ws_local_preview",
    seq,
    task_id: eventType.startsWith("import.") ? "task_import_preview" : "task_workspace_preview",
    transaction_id: `tx_preview_${seq}`,
    type: eventType,
    workspace_id: "ws_local_preview",
  };
}

export async function createElectronAppModel(): Promise<ElectronAppModel> {
  const transport = createMockTransportClient({
    events: [
      previewEvent(1, "daemon.ready"),
      previewEvent(2, "workspace.init.completed", {
        workspace: {
          audit_head: "audit_preview",
          current_revision: "wiki_rev_0",
          index_status: {
            last_good_revision: null,
            stale_reason: null,
            state: "stale",
            updated_at: null,
          },
          root_uri: "file:///workspace",
          state: "ready",
          workspace_id: "ws_local_preview",
        },
      }),
      previewEvent(3, "import.stage.changed", {
        stage: "selected",
      }),
    ],
  });
  const runtime = createDomainRuntime(transport);

  await runtime.client.workspace.init({
    rootUri: "file:///workspace",
    workspaceId: "ws_local_preview",
  });
  await runtime.client.files.prepareUserSelected({
    declaredMime: "text/markdown",
    declaredSize: 0,
    displayName: "等待选择本机文件",
    opaqueFileRef: "electron://selection/pending",
    platform: "electron",
    selectionId: "sel_pending",
  });
  await runtime.replay(0);

  const snapshot = runtime.store.getSnapshot();

  return {
    importStage: snapshot.imports[0]?.stage ?? "selected",
    workspaceStage: snapshot.workspace.stage,
    workspaceTitle: snapshot.workspace.dto ? "ws_local_preview" : "ws_local_preview",
  };
}
