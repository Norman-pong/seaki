import { M0_DOMAIN_USE_CASE_METHODS } from "@seaki/dto";
import type {
  ApprovalDecisionResultDTO,
  ApprovalReviewDTO,
  FrontendEventEnvelope,
  IndexStatusDTO,
  SearchResultDTO,
  UserSelectedFileDTO,
  WorkspaceDTO,
} from "@seaki/dto";
import { createRuntimeStore, finishWorkspaceInit, startWorkspaceInit } from "@seaki/state";
import type {
  AppBootState,
  FrontendRuntimeEvent,
  ImportState,
  RuntimeStore,
  WorkspaceState,
} from "@seaki/state";
import type { TransportClient } from "@seaki/transport";

const DOMAIN_METHOD = {
  approvalDecide: M0_DOMAIN_USE_CASE_METHODS.APPROVAL_DECIDE,
  approvalReviewPatch: M0_DOMAIN_USE_CASE_METHODS.APPROVAL_REVIEW_PATCH,
  citationResolve: M0_DOMAIN_USE_CASE_METHODS.CITATION_RESOLVE,
  filesPrepareUserSelected: M0_DOMAIN_USE_CASE_METHODS.FILES_PREPARE_USER_SELECTED,
  searchQuery: M0_DOMAIN_USE_CASE_METHODS.SEARCH_QUERY,
  sourceIngestSelectedFile: M0_DOMAIN_USE_CASE_METHODS.SOURCE_INGEST_SELECTED_FILE,
  wikiReadPage: M0_DOMAIN_USE_CASE_METHODS.WIKI_READ_PAGE,
  workspaceInit: M0_DOMAIN_USE_CASE_METHODS.WORKSPACE_INIT,
} as const;

export interface WorkspaceShell {
  readonly auditHead?: string;
  readonly currentRevision?: string;
  readonly indexStatus?: IndexStatusDTO;
  readonly rootUri?: string;
  readonly state: WorkspaceState;
  readonly workspace?: WorkspaceDTO;
  readonly workspaceId?: string;
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

export interface WorkspaceInitInput {
  readonly rootUri: string;
  readonly workspaceId?: string;
}

export interface WorkspaceInitOutput {
  readonly auditHead?: string;
  readonly currentRevision?: string;
  readonly indexStatus?: IndexStatusDTO;
  readonly rootUri?: string;
  readonly workspace?: WorkspaceDTO;
  readonly workspaceId?: string;
}

export interface IngestSelectedFileInput {
  readonly selectionId: string;
  readonly workspaceId: string;
}

export interface ReviewPatchInput {
  readonly patchId: string;
  readonly workspaceId: string;
}

export interface DecideApprovalInput {
  readonly approvalId: string;
  readonly claimDecisions?: readonly ClaimApprovalDecisionInput[];
  readonly decision: "approve" | "reject";
  readonly reason?: string;
  readonly workspaceId: string;
}

export interface ClaimApprovalDecisionInput {
  readonly claimId: string;
  readonly decision: "approve" | "reject";
  readonly reason?: string;
}

export interface ReadWikiPageInput {
  readonly pageId: string;
  readonly revision?: string;
  readonly workspaceId: string;
}

export interface SearchQueryInput {
  readonly query: string;
  readonly workspaceId: string;
}

export interface ResolveCitationInput {
  readonly citationId: string;
  readonly workspaceId: string;
}

export interface DomainClient {
  readonly approval: {
    reviewPatch(input: ReviewPatchInput): Promise<ApprovalReviewDTO>;
    decide(input: DecideApprovalInput): Promise<ApprovalDecisionResultDTO>;
  };
  readonly citation: {
    resolve(input: ResolveCitationInput): Promise<unknown>;
  };
  readonly files: {
    prepareUserSelected(
      input: UserSelectedFileInput | UserSelectedFileDTO,
    ): Promise<UserSelectedFileDTO>;
  };
  readonly search: {
    query(input: SearchQueryInput): Promise<readonly SearchResultDTO[]>;
  };
  readonly source: {
    ingestSelectedFile(input: IngestSelectedFileInput): Promise<unknown>;
  };
  readonly wiki: {
    readPage(input: ReadWikiPageInput): Promise<unknown>;
  };
  readonly workspace: {
    init(input: WorkspaceInitInput): Promise<WorkspaceInitOutput>;
  };
}

export interface DomainRuntime {
  readonly client: DomainClient;
  readonly store: RuntimeStore;
  replay(fromSeq?: number): Promise<number>;
}

function request<TOutput, TInput>(
  transport: TransportClient,
  method: string,
  input: TInput,
): Promise<TOutput> {
  return transport.request<TOutput, TInput>(method, input);
}

export function createDomainClient(transport: TransportClient): DomainClient {
  return {
    approval: {
      decide(input) {
        return request(transport, DOMAIN_METHOD.approvalDecide, input);
      },
      reviewPatch(input) {
        return request(transport, DOMAIN_METHOD.approvalReviewPatch, input);
      },
    },
    citation: {
      resolve(input) {
        return request(transport, DOMAIN_METHOD.citationResolve, input);
      },
    },
    files: {
      prepareUserSelected(input) {
        return request(transport, DOMAIN_METHOD.filesPrepareUserSelected, input);
      },
    },
    search: {
      query(input) {
        return request(transport, DOMAIN_METHOD.searchQuery, input);
      },
    },
    source: {
      ingestSelectedFile(input) {
        return request(transport, DOMAIN_METHOD.sourceIngestSelectedFile, input);
      },
    },
    wiki: {
      readPage(input) {
        return request(transport, DOMAIN_METHOD.wikiReadPage, input);
      },
    },
    workspace: {
      init(input) {
        return request(transport, DOMAIN_METHOD.workspaceInit, input);
      },
    },
  };
}

export function createDomainRuntime(transport: TransportClient): DomainRuntime {
  const store = createRuntimeStore();

  return {
    client: createDomainClient(transport),
    async replay(fromSeq = store.getSnapshot().lastSeq) {
      return transport.replay(fromSeq, (event: FrontendEventEnvelope) => {
        store.dispatch(event as FrontendRuntimeEvent);
      });
    },
    store,
  };
}

export async function bootApp(transport: TransportClient): Promise<AppBootState> {
  const runtime = createDomainRuntime(transport);

  await runtime.replay(0);

  return runtime.store.getSnapshot().appBoot;
}

export function initWorkspaceShell(input: {
  readonly auditHead?: string;
  readonly currentRevision?: string;
  readonly degradedReason?: WorkspaceState["reason"];
  readonly indexStatus?: IndexStatusDTO;
  readonly rootUri?: string;
  readonly workspace?: WorkspaceDTO;
  readonly workspaceId?: string;
}): WorkspaceShell {
  const initializingState = startWorkspaceInit({
    stage: "uninitialized",
  });
  const workspaceInit = {
    ...(input.workspace ? { dto: input.workspace } : {}),
    ...(input.indexStatus ? { indexStatus: input.indexStatus } : {}),
    ...(input.degradedReason ? { reason: input.degradedReason } : {}),
  };
  const state =
    initializingState.stage === "initializing"
      ? finishWorkspaceInit(workspaceInit)
      : initializingState;

  return {
    ...(input.auditHead ? { auditHead: input.auditHead } : {}),
    ...(input.currentRevision ? { currentRevision: input.currentRevision } : {}),
    ...(input.indexStatus ? { indexStatus: input.indexStatus } : {}),
    ...(input.rootUri ? { rootUri: input.rootUri } : {}),
    state,
    ...(input.workspace ? { workspace: input.workspace } : {}),
    ...(input.workspaceId ? { workspaceId: input.workspaceId } : {}),
  };
}

export function prepareUserSelectedFile(input: UserSelectedFileInput): PreparedImport {
  return {
    selection: input,
    state: {
      committed: false,
      stage: "selected",
    },
  };
}
