import {
  createSelectedImport,
  finishWorkspaceInit,
  reduceAppBoot,
  startWorkspaceInit,
} from "@seaki/state";
import type { AppBootState, ImportState, WorkspaceState } from "@seaki/state";
import type { TransportClient } from "@seaki/transport";

export interface WorkspaceShell {
  readonly auditHead: string;
  readonly currentRevision: string;
  readonly rootUri: string;
  readonly state: WorkspaceState;
  readonly workspaceId: string;
}

export interface UserSelectedFileInput {
  readonly declaredMime: string;
  readonly declaredSize: number;
  readonly displayName: string;
  readonly opaqueFileRef: string;
  readonly platform: "electron";
  readonly selectionId: string;
}

export interface PreparedImport {
  readonly selection: UserSelectedFileInput;
  readonly state: ImportState;
}

export async function bootApp(transport: TransportClient): Promise<AppBootState> {
  const heartbeat = await transport.connect();

  return reduceAppBoot(heartbeat.status);
}

export function initWorkspaceShell(input: {
  readonly auditHead: string;
  readonly currentRevision: string;
  readonly degradedReason?: WorkspaceState["reason"];
  readonly rootUri: string;
  readonly workspaceId: string;
}): WorkspaceShell {
  const initializingState = startWorkspaceInit({
    stage: "uninitialized",
  });
  const state =
    initializingState.stage === "initializing"
      ? finishWorkspaceInit(input.degradedReason)
      : initializingState;

  return {
    auditHead: input.auditHead,
    currentRevision: input.currentRevision,
    rootUri: input.rootUri,
    state,
    workspaceId: input.workspaceId,
  };
}

export function prepareUserSelectedFile(input: UserSelectedFileInput): PreparedImport {
  return {
    selection: input,
    state: createSelectedImport(),
  };
}
