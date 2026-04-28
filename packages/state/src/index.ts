import type {
  DaemonConnectionStatus,
  FrontendEventEnvelope,
  ImportStage as DTOImportStage,
  IndexStatusDTO,
  WorkspaceDTO,
} from "@seaki/dto";

export interface AppBootState {
  readonly stage: DaemonConnectionStatus;
}

export type WorkspaceStage = "uninitialized" | "initializing" | "ready" | "degraded" | "error";

export type WorkspaceDegradedReason = "index_stale" | "audit_readonly" | "daemon_recovering";

export interface WorkspaceState {
  readonly stage: WorkspaceStage;
  readonly dto?: WorkspaceDTO;
  readonly indexStatus?: IndexStatusDTO;
  readonly reason?: WorkspaceDegradedReason;
}

export type ImportStage = DTOImportStage;

export interface ImportState {
  readonly committed: boolean;
  readonly stage: ImportStage;
  readonly taskId?: string;
  readonly workspaceId?: string;
}

export interface TaskRuntimeState {
  readonly id: string;
  readonly kind: "workspace.init" | "source.ingest" | "unknown";
  readonly lastEventSeq: number;
  readonly stage: string;
}

export interface FrontendRuntimeState {
  readonly appBoot: AppBootState;
  readonly imports: readonly ImportState[];
  readonly lastSeq: number;
  readonly tasks: Readonly<Record<string, TaskRuntimeState>>;
  readonly workspace: WorkspaceState;
}

export type FrontendRuntimeEvent = FrontendEventEnvelope & {
  readonly event_type?: string;
  readonly payload?: unknown;
  readonly type?: string;
};

export type RuntimeStateListener = (state: FrontendRuntimeState) => void;

export interface RuntimeStore {
  getSnapshot(): FrontendRuntimeState;
  dispatch(event: FrontendRuntimeEvent): FrontendRuntimeState;
  subscribe(listener: RuntimeStateListener): () => void;
}

const IMPORT_STAGE_ORDER: Record<ImportStage, readonly ImportStage[]> = {
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

const COMMITTED_IMPORT_STAGES = new Set<ImportStage>(["committed", "indexed", "index_stale"]);

export function createInitialRuntimeState(): FrontendRuntimeState {
  return {
    appBoot: {
      stage: "daemon.connecting",
    },
    imports: [],
    lastSeq: 0,
    tasks: {},
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

export function finishWorkspaceInit(
  input: {
    readonly dto?: WorkspaceDTO | undefined;
    readonly indexStatus?: IndexStatusDTO | undefined;
    readonly reason?: WorkspaceDegradedReason | undefined;
  } = {},
): WorkspaceState {
  const workspaceFields = {
    ...(input.dto ? { dto: input.dto } : {}),
    ...(input.indexStatus ? { indexStatus: input.indexStatus } : {}),
  };

  if (input.reason) {
    return {
      ...workspaceFields,
      reason: input.reason,
      stage: "degraded",
    };
  }

  return {
    ...workspaceFields,
    stage: "ready",
  };
}

export function failWorkspaceInit(): WorkspaceState {
  return {
    stage: "error",
  };
}

export function createSelectedImport(
  input: {
    readonly taskId?: string | undefined;
    readonly workspaceId?: string | undefined;
  } = {},
): ImportState {
  return {
    committed: false,
    stage: "selected",
    ...(input.taskId ? { taskId: input.taskId } : {}),
    ...(input.workspaceId ? { workspaceId: input.workspaceId } : {}),
  };
}

export function advanceImportState(state: ImportState, next: ImportStage): ImportState {
  if (state.stage === next) {
    return state;
  }

  if (!IMPORT_STAGE_ORDER[state.stage].includes(next)) {
    return state;
  }

  return {
    ...state,
    committed: state.committed || COMMITTED_IMPORT_STAGES.has(next),
    stage: next,
  };
}

function getEventType(event: FrontendRuntimeEvent): string {
  return event.type;
}

function getPayload(event: FrontendRuntimeEvent): Record<string, unknown> {
  if (event.payload && typeof event.payload === "object") {
    return event.payload as Record<string, unknown>;
  }

  return {};
}

function getString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function getWorkspaceReason(value: unknown): WorkspaceDegradedReason | undefined {
  if (value === "index_stale" || value === "audit_readonly" || value === "daemon_recovering") {
    return value;
  }

  return undefined;
}

function getImportStage(value: unknown): ImportStage | undefined {
  return typeof value === "string" && value in IMPORT_STAGE_ORDER
    ? (value as ImportStage)
    : undefined;
}

function taskIdFor(
  event: FrontendRuntimeEvent,
  payload: Record<string, unknown>,
): string | undefined {
  return getString(event.task_id) ?? getString(payload.taskId) ?? getString(payload.task_id);
}

function upsertTask(
  state: FrontendRuntimeState,
  event: FrontendRuntimeEvent,
  kind: TaskRuntimeState["kind"],
  stage: string,
): Readonly<Record<string, TaskRuntimeState>> {
  const payload = getPayload(event);
  const taskId = taskIdFor(event, payload);

  if (!taskId) {
    return state.tasks;
  }

  return {
    ...state.tasks,
    [taskId]: {
      id: taskId,
      kind,
      lastEventSeq: Number(event.seq),
      stage,
    },
  };
}

function reduceWorkspaceEvent(
  state: FrontendRuntimeState,
  event: FrontendRuntimeEvent,
): FrontendRuntimeState {
  const payload = getPayload(event);
  const reason = getWorkspaceReason(
    payload.reason ?? payload.degradedReason ?? payload.degraded_reason,
  );
  const workspace = finishWorkspaceInit({
    dto: payload.workspace as WorkspaceDTO | undefined,
    indexStatus: payload.indexStatus as IndexStatusDTO | undefined,
    reason,
  });

  return {
    ...state,
    tasks: upsertTask(state, event, "workspace.init", workspace.stage),
    workspace,
  };
}

function reduceImportEvent(
  state: FrontendRuntimeState,
  event: FrontendRuntimeEvent,
): FrontendRuntimeState {
  const payload = getPayload(event);
  const nextStage = getImportStage(payload.stage ?? payload.import_stage);

  if (!nextStage) {
    return state;
  }

  const taskId = taskIdFor(event, payload);
  const workspaceId = getString(event.workspace_id);
  const existingIndex = state.imports.findIndex((item) => {
    if (taskId && item.taskId) {
      return item.taskId === taskId;
    }

    return false;
  });
  const current =
    existingIndex >= 0
      ? (state.imports[existingIndex] ??
        createSelectedImport({
          taskId,
          workspaceId,
        }))
      : createSelectedImport({
          taskId,
          workspaceId,
        });
  const isDraftOrTemporary = payload.draft === true || payload.temporary === true;
  const advanced =
    isDraftOrTemporary && COMMITTED_IMPORT_STAGES.has(nextStage)
      ? current
      : advanceImportState(current, nextStage);
  const nextImport = isDraftOrTemporary
    ? {
        ...advanced,
        committed: current.committed,
      }
    : advanced;
  const imports =
    existingIndex >= 0
      ? state.imports.map((item, index) => (index === existingIndex ? nextImport : item))
      : [...state.imports, nextImport];

  return {
    ...state,
    imports,
    tasks: upsertTask(state, event, "source.ingest", nextImport.stage),
  };
}

export function reduceRuntimeState(
  state: FrontendRuntimeState,
  event: FrontendRuntimeEvent,
): FrontendRuntimeState {
  const eventType = getEventType(event);
  const nextState = {
    ...state,
    lastSeq: Math.max(state.lastSeq, Number(event.seq)),
  };

  if (eventType === "daemon.ready") {
    return {
      ...nextState,
      appBoot: reduceAppBoot("daemon.ready"),
    };
  }

  if (eventType === "daemon.unavailable") {
    return {
      ...nextState,
      appBoot: reduceAppBoot("daemon.unavailable"),
    };
  }

  if (eventType === "workspace.init.started") {
    return {
      ...nextState,
      tasks: upsertTask(nextState, event, "workspace.init", "initializing"),
      workspace: startWorkspaceInit(nextState.workspace),
    };
  }

  if (eventType === "workspace.init.completed" || eventType === "workspace.ready") {
    return reduceWorkspaceEvent(nextState, event);
  }

  if (eventType === "workspace.init.failed" || eventType === "workspace.error") {
    return {
      ...nextState,
      tasks: upsertTask(nextState, event, "workspace.init", "error"),
      workspace: failWorkspaceInit(),
    };
  }

  if (eventType === "import.stage.changed" || eventType === "source.ingest.stage.changed") {
    return reduceImportEvent(nextState, event);
  }

  return nextState;
}

export function createRuntimeStore(
  initialState: FrontendRuntimeState = createInitialRuntimeState(),
): RuntimeStore {
  let state = initialState;
  const listeners = new Set<RuntimeStateListener>();

  return {
    dispatch(event) {
      state = reduceRuntimeState(state, event);

      for (const listener of listeners) {
        listener(state);
      }

      return state;
    },
    getSnapshot() {
      return state;
    },
    subscribe(listener) {
      listeners.add(listener);

      return () => {
        listeners.delete(listener);
      };
    },
  };
}
