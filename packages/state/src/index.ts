import type { DaemonConnectionStatus } from "@seaki/transport";

export interface AppBootState {
  readonly stage: DaemonConnectionStatus;
}

export type WorkspaceStage = "uninitialized" | "initializing" | "ready" | "degraded" | "error";

export type WorkspaceDegradedReason = "index_stale" | "audit_readonly" | "daemon_recovering";

export interface WorkspaceState {
  readonly stage: WorkspaceStage;
  readonly reason?: WorkspaceDegradedReason;
}

export type ImportStage =
  | "selected"
  | "grant_requested"
  | "granted"
  | "capability_denied"
  | "raw_committed"
  | "parse_running"
  | "parsed"
  | "partial"
  | "failed"
  | "patch_proposed"
  | "approval_pending"
  | "committed"
  | "denied"
  | "indexed"
  | "index_stale";

export interface ImportState {
  readonly stage: ImportStage;
}

export interface FrontendRuntimeState {
  readonly appBoot: AppBootState;
  readonly workspace: WorkspaceState;
  readonly imports: readonly ImportState[];
}

export function createInitialRuntimeState(): FrontendRuntimeState {
  return {
    appBoot: {
      stage: "daemon.connecting",
    },
    imports: [],
    workspace: {
      stage: "uninitialized",
    },
  };
}

export function reduceAppBoot(status: DaemonConnectionStatus): AppBootState {
  return {
    stage: status,
  };
}

export function startWorkspaceInit(state: WorkspaceState): WorkspaceState {
  if (state.stage === "ready") {
    return state;
  }

  return {
    stage: "initializing",
  };
}

export function finishWorkspaceInit(reason?: WorkspaceDegradedReason): WorkspaceState {
  if (reason) {
    return {
      reason,
      stage: "degraded",
    };
  }

  return {
    stage: "ready",
  };
}

export function createSelectedImport(): ImportState {
  return {
    stage: "selected",
  };
}

export function advanceImportState(state: ImportState, next: ImportStage): ImportState {
  const allowedNextStages: Record<ImportStage, readonly ImportStage[]> = {
    approval_pending: ["committed", "denied"],
    capability_denied: [],
    committed: ["indexed", "index_stale"],
    denied: [],
    failed: [],
    grant_requested: ["granted", "capability_denied"],
    granted: ["raw_committed"],
    index_stale: [],
    indexed: [],
    parse_running: ["parsed", "partial", "failed"],
    parsed: ["patch_proposed"],
    partial: ["patch_proposed", "failed"],
    patch_proposed: ["approval_pending"],
    raw_committed: ["parse_running"],
    selected: ["grant_requested"],
  };

  if (!allowedNextStages[state.stage].includes(next)) {
    return state;
  }

  return {
    stage: next,
  };
}
