import type {
  ApprovalClaimDecisionDTO,
  ApprovalDecisionResultDTO,
  ApprovalRequestDTO,
  ApprovalStatus as DTOApprovalStatus,
  DaemonConnectionStatus,
  FrontendEventEnvelope,
  ImportStage as DTOImportStage,
  IndexStatusDTO,
  WikiPatchProposalDTO,
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

export type ApprovalStatus = DTOApprovalStatus;

export interface ApprovalState {
  readonly approvalId: string;
  readonly patchId: string;
  readonly status: ApprovalStatus;
  readonly committed: boolean;
  readonly claimDecisions: readonly ApprovalClaimDecisionDTO[];
  readonly committedRevision?: string;
  readonly request?: ApprovalRequestDTO;
  readonly result?: ApprovalDecisionResultDTO;
  readonly taskId?: string;
  readonly workspaceId?: string;
}

export interface TaskRuntimeState {
  readonly id: string;
  readonly kind: "workspace.init" | "source.ingest" | "approval" | "unknown";
  readonly lastEventSeq: number;
  readonly stage: string;
}

export interface FrontendRuntimeState {
  readonly appBoot: AppBootState;
  readonly approvals: readonly ApprovalState[];
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

const APPROVAL_STATUS_BY_EVENT_TYPE: Readonly<Record<string, ApprovalStatus>> = {
  "approval.applying": "applying",
  "approval.committed": "committed",
  "approval.conflict": "conflict",
  "approval.expired": "expired",
  "approval.rejected": "rejected",
};

const APPROVAL_STATUSES = new Set<ApprovalStatus>([
  "pending",
  "approved",
  "applying",
  "committed",
  "rejected",
  "expired",
  "conflict",
]);

export function createInitialRuntimeState(): FrontendRuntimeState {
  return {
    appBoot: {
      stage: "daemon.connecting",
    },
    approvals: [],
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

function getApprovalStatus(value: unknown): ApprovalStatus | undefined {
  return typeof value === "string" && APPROVAL_STATUSES.has(value as ApprovalStatus)
    ? (value as ApprovalStatus)
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
  const workspaceDto = payload.workspace as WorkspaceDTO | undefined;
  const workspace = finishWorkspaceInit({
    dto: workspaceDto,
    indexStatus: (payload.indexStatus as IndexStatusDTO | undefined) ?? workspaceDto?.index_status,
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

function getRequest(value: unknown): ApprovalRequestDTO | undefined {
  return value && typeof value === "object" ? (value as ApprovalRequestDTO) : undefined;
}

function getProposal(value: unknown): WikiPatchProposalDTO | undefined {
  return value && typeof value === "object" ? (value as WikiPatchProposalDTO) : undefined;
}

function getDecisionResult(value: unknown): ApprovalDecisionResultDTO | undefined {
  return value && typeof value === "object" ? (value as ApprovalDecisionResultDTO) : undefined;
}

function getClaimDecisions(value: unknown): readonly ApprovalClaimDecisionDTO[] | undefined {
  return Array.isArray(value) ? (value as readonly ApprovalClaimDecisionDTO[]) : undefined;
}

function approvalIdFor(payload: Record<string, unknown>, request?: ApprovalRequestDTO): string {
  return (
    getString(payload.approval_id) ??
    getString(payload.approvalId) ??
    request?.approval_id ??
    "approval:unknown"
  );
}

function patchIdFor(payload: Record<string, unknown>, request?: ApprovalRequestDTO): string {
  return (
    getString(payload.patch_id) ??
    getString(payload.patchId) ??
    request?.patch_id ??
    request?.proposal?.patch_id ??
    "patch:unknown"
  );
}

function statusFromDecision(value: unknown): ApprovalStatus | undefined {
  if (value === "approve" || value === "approved") {
    return "approved";
  }

  if (value === "reject" || value === "denied") {
    return "rejected";
  }

  return undefined;
}

function reduceApprovalEvent(
  state: FrontendRuntimeState,
  event: FrontendRuntimeEvent,
): FrontendRuntimeState {
  const eventType = getEventType(event);
  const payload = getPayload(event);
  const request = getRequest(payload.request);
  const result = getDecisionResult(payload.result);
  const explicitStatus =
    getApprovalStatus(payload.status) ??
    result?.status ??
    request?.status ??
    statusFromDecision(payload.decision);
  const eventStatus = APPROVAL_STATUS_BY_EVENT_TYPE[eventType];
  const nextStatus = eventStatus ?? explicitStatus ?? "pending";
  const approvalId = result?.approval_id ?? approvalIdFor(payload, request);
  const patchId = result?.patch_id ?? patchIdFor(payload, request);
  const taskId = taskIdFor(event, payload);
  const workspaceId = getString(event.workspace_id);
  const existingIndex = state.approvals.findIndex((approval) => {
    if (approval.approvalId === approvalId) {
      return true;
    }

    return approval.patchId === patchId;
  });
  const fallbackApproval: ApprovalState = {
    approvalId,
    claimDecisions: [],
    committed: false,
    patchId,
    status: "pending",
  };
  const current =
    existingIndex >= 0 ? (state.approvals[existingIndex] ?? fallbackApproval) : fallbackApproval;
  const draftOrTemporary = payload.draft === true || payload.temporary === true;
  const claimDecisions =
    result?.claim_decisions ??
    getClaimDecisions(payload.claim_decisions) ??
    request?.claim_decisions ??
    current.claimDecisions;
  const proposal = getProposal(payload.proposal) ?? request?.proposal;
  const requestFromReview =
    request ??
    (proposal
      ? {
          approval_id: approvalId,
          audit_id: null,
          claim_decisions: claimDecisions,
          expires_at: getString(payload.expires_at) ?? "",
          patch_id: patchId,
          policy_decision: "requires_approval" as const,
          proposal,
          rejection_reason: null,
          required_by: getString(payload.required_by) ?? "",
          status: nextStatus,
          wal_entry_id: null,
        }
      : undefined);
  const committedRevision =
    eventType === "approval.committed" && !draftOrTemporary
      ? (result?.committed_revision ??
        getString(payload.committed_revision) ??
        getString(payload.revision) ??
        event.revision)
      : current.committedRevision;
  const resultForState = draftOrTemporary && nextStatus === "committed" ? undefined : result;
  const nextApproval = {
    ...current,
    approvalId,
    claimDecisions,
    committed: current.committed || (eventType === "approval.committed" && !draftOrTemporary),
    patchId,
    status: draftOrTemporary && nextStatus === "committed" ? current.status : nextStatus,
    ...(committedRevision ? { committedRevision } : {}),
    ...(requestFromReview ? { request: requestFromReview } : {}),
    ...(resultForState ? { result: resultForState } : {}),
    ...(taskId ? { taskId } : {}),
    ...(workspaceId ? { workspaceId } : {}),
  };
  const approvals =
    existingIndex >= 0
      ? state.approvals.map((approval, index) =>
          index === existingIndex ? nextApproval : approval,
        )
      : [...state.approvals, nextApproval];

  return {
    ...state,
    approvals,
    tasks: upsertTask(state, event, "approval", nextApproval.status),
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

  if (
    eventType === "approval.reviewed" ||
    eventType === "approval.decided" ||
    eventType === "approval.applying" ||
    eventType === "approval.committed" ||
    eventType === "approval.rejected" ||
    eventType === "approval.conflict" ||
    eventType === "approval.expired"
  ) {
    return reduceApprovalEvent(nextState, event);
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
