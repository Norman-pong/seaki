import type {
  CitationRefDTO,
  DaemonConnectionStatus,
  ImportStage as DTOImportStage,
  IndexStatusDTO,
  SearchResultDTO,
  SourceCardDTO,
  SourceManifestDTO,
  UserSelectedFileDTO,
  WorkspaceDTO,
} from "@seaki/dto";

export interface DaemonStatusModel {
  readonly auditMode: "writable" | "readonly";
  readonly canOpenLogs: boolean;
  readonly canReconnect: boolean;
  readonly detail: string;
  readonly heartbeatAt: string | null;
  readonly status: DaemonConnectionStatus | "daemon.degraded";
}

export interface WorkspaceShellModel {
  readonly auditHead: string;
  readonly canInitWorkspace: boolean;
  readonly canRebuildIndex: boolean;
  readonly currentRevision: string;
  readonly degradedReasons: readonly string[];
  readonly indexStatus: IndexStatusDTO;
  readonly rootUri: string;
  readonly state: WorkspaceDTO["state"];
  readonly workspaceId: string;
}

export interface ImportQueueItemModel {
  readonly action: "authorize" | "retry_parse" | "rebuild_index" | "inspect" | "none";
  readonly committed: boolean;
  readonly detail: string;
  readonly displayName: string;
  readonly manifest?: SourceManifestDTO;
  readonly selection: UserSelectedFileDTO;
  readonly stage: DTOImportStage;
  readonly taskId: string;
}

export interface WikiReaderModel {
  readonly citationRefs: readonly CitationRefDTO[];
  readonly committedRevision: string;
  readonly draftVisible: boolean;
  readonly pageId: string;
  readonly status: "committed" | "degraded" | "no_access";
  readonly title: string;
  readonly warning: string | null;
}

export interface SearchResultsModel {
  readonly emptyReason: string | null;
  readonly filteredByPermission: number;
  readonly isLoading: boolean;
  readonly query: string;
  readonly results: readonly SearchResultDTO[];
  readonly status: "loading" | "ready" | "empty" | "stale" | "filtered_by_permission";
}

export interface AnswerModel {
  readonly answerId: string;
  readonly text: string;
  readonly citationRefs: readonly CitationRefDTO[];
  readonly status: "composed" | "degraded" | "no_access";
}

export interface CitationPreviewModel {
  readonly annotation?: {
    readonly annotationId: string;
    readonly note: string;
  };
  readonly citation: CitationRefDTO;
  readonly preview: SourceCardDTO | null;
  readonly recoverability: "retry" | "request_access" | "inspect_source" | "none";
  readonly status:
    | "resolving"
    | "open_source_range"
    | "open_wiki_anchor"
    | "degraded"
    | "no_access";
}

export interface MvpScreenModel {
  readonly answer: AnswerModel;
  readonly citationPreview: CitationPreviewModel;
  readonly daemonStatus: DaemonStatusModel;
  readonly importQueue: readonly ImportQueueItemModel[];
  readonly searchResults: SearchResultsModel;
  readonly wikiReader: WikiReaderModel;
  readonly workspaceShell: WorkspaceShellModel;
}

const previewWorkspace: WorkspaceDTO = {
  audit_head: "audit_preview",
  current_revision: "wiki_rev_0",
  index_status: {
    last_good_revision: null,
    stale_reason: "source visibility changed; rebuild required",
    state: "stale",
    updated_at: null,
  },
  root_uri: "file:///workspace",
  state: "degraded",
  workspace_id: "ws_local_preview",
};

const selectedFile: UserSelectedFileDTO = {
  declared_mime: "text/markdown",
  declared_size: 18432,
  display_name: "2026-04-28-import.md",
  opaque_file_ref: "electron://selection/preview-md",
  platform: "electron",
  selection_id: "sel_preview_markdown",
};

const failedFile: UserSelectedFileDTO = {
  declared_mime: "application/pdf",
  declared_size: 7340032,
  display_name: "vendor-briefing.pdf",
  opaque_file_ref: "electron://selection/preview-pdf",
  platform: "electron",
  selection_id: "sel_preview_pdf",
};

const visibleCitationRef: CitationRefDTO = {
  citation_id: "cit_screen_source_scope",
  claim_id: "claim_source_scope",
  degraded_reason: null,
  range: {
    end: 18,
    label: "L12-L18",
    start: 12,
    unit: "line",
  },
  source_id: "src_local_note_20260428",
  wiki_page_id: "wiki_m0_decision",
};

const restrictedCitationRef: CitationRefDTO = {
  citation_id: "cit_restricted_source",
  claim_id: "claim_restricted",
  degraded_reason: "no_access",
  range: {
    end: 12,
    label: "restricted.md:8-12",
    start: 8,
    unit: "line",
  },
  source_id: "src_restricted",
  wiki_page_id: "wiki_restricted",
};

const indexedManifest: SourceManifestDTO = {
  mime: "text/markdown",
  origin_display: "本机资料 / 2026-04-28-import.md",
  parse_status: "index_stale",
  permission_scope: "workspace:ws_local_preview",
  size: selectedFile.declared_size,
  source_id: visibleCitationRef.source_id,
};

const failedManifest: SourceManifestDTO = {
  mime: "application/pdf",
  origin_display: "本机资料 / vendor-briefing.pdf",
  parse_status: "failed",
  permission_scope: "workspace:ws_local_preview",
  size: failedFile.declared_size,
  source_id: "src_vendor_briefing_pdf",
};

const staleSearchResult: SearchResultDTO = {
  citation_refs: [
    {
      ...visibleCitationRef,
      degraded_reason: "index_stale",
    },
  ],
  index_status: previewWorkspace.index_status,
  kind: "claim",
  result_id: "result_source_scope",
  snippet: "本机导入范围限制在当前 workspace 选择文件。",
  title: "source scope",
};

const filteredSearchResult: SearchResultDTO = {
  citation_refs: [restrictedCitationRef],
  index_status: {
    last_good_revision: "wiki_rev_0",
    stale_reason: null,
    state: "fresh",
    updated_at: "2026-04-28T00:10:00.000Z",
  },
  kind: "source",
  result_id: "result_restricted",
  snippet: null,
  title: "restricted source hidden",
};

export function createWorkspaceShellModel(workspace: WorkspaceDTO): WorkspaceShellModel {
  const indexStatus = workspace.index_status;
  const degradedReasons = [
    ...(workspace.state === "audit_readonly" ? ["audit_readonly"] : []),
    ...(workspace.state === "degraded" ? ["daemon_recovering"] : []),
    ...(indexStatus.state === "stale" ? ["index_stale"] : []),
    ...(indexStatus.state === "error" ? ["index_error"] : []),
  ];

  return {
    auditHead: workspace.audit_head,
    canInitWorkspace: workspace.state === "empty",
    canRebuildIndex: indexStatus.state === "stale" || indexStatus.state === "error",
    currentRevision: workspace.current_revision,
    degradedReasons,
    indexStatus,
    rootUri: workspace.root_uri,
    state: workspace.state,
    workspaceId: workspace.workspace_id,
  };
}

export function createSearchResultsModel(input: {
  readonly filteredByPermission?: number | undefined;
  readonly isLoading?: boolean | undefined;
  readonly query: string;
  readonly results: readonly SearchResultDTO[];
}): SearchResultsModel {
  const filteredByPermission =
    input.filteredByPermission ?? input.results.filter((result) => result.snippet === null).length;
  const hasStaleResult = input.results.some(
    (result) =>
      result.index_status.state === "stale" ||
      result.citation_refs.some((citation) => citation.degraded_reason === "index_stale"),
  );
  const status = input.isLoading
    ? "loading"
    : input.results.length === 0
      ? "empty"
      : hasStaleResult
        ? "stale"
        : filteredByPermission > 0
          ? "filtered_by_permission"
          : "ready";

  return {
    emptyReason: status === "empty" ? "没有可见搜索结果" : null,
    filteredByPermission,
    isLoading: input.isLoading ?? false,
    query: input.query,
    results: input.results,
    status,
  };
}

export function createAnswerModel(input: {
  readonly answerId: string;
  readonly text: string;
  readonly citationRefs: readonly CitationRefDTO[];
  readonly status: "composed" | "degraded" | "no_access";
}): AnswerModel {
  return {
    answerId: input.answerId,
    text: input.text,
    citationRefs: input.citationRefs,
    status: input.status,
  };
}

export function createCitationPreviewModel(
  citation: CitationRefDTO,
  preview: SourceCardDTO | null,
): CitationPreviewModel {
  if (!preview || preview.visibility !== "visible") {
    return {
      citation,
      preview: null,
      recoverability: "request_access",
      status: "no_access",
    };
  }

  if (citation.degraded_reason) {
    return {
      citation,
      preview,
      recoverability: "inspect_source",
      status: "degraded",
    };
  }

  return {
    annotation: {
      annotationId: "annotation_preview_1",
      note: "Evidence range is available for review.",
    },
    citation,
    preview,
    recoverability: "none",
    status: "open_source_range",
  };
}

export function createWikiReaderModel(
  workspace: WorkspaceDTO,
  citationRefs: readonly CitationRefDTO[],
): WikiReaderModel {
  const hasDegradedCitation = citationRefs.some((citation) => citation.degraded_reason);

  return {
    citationRefs,
    committedRevision: workspace.current_revision,
    draftVisible: false,
    pageId: "wiki_m0_decision",
    status: hasDegradedCitation ? "degraded" : "committed",
    title: "M0 本机导入 DecisionRecord",
    warning: hasDegradedCitation ? "部分 citation 已降级，草稿不会显示为已提交。" : null,
  };
}

export function createImportQueueModel(): readonly ImportQueueItemModel[] {
  return [
    {
      action: "rebuild_index",
      committed: true,
      detail: "已提交，等待 index rebuild 刷新搜索结果。",
      displayName: selectedFile.display_name,
      manifest: indexedManifest,
      selection: selectedFile,
      stage: "index_stale",
      taskId: "task_import_markdown",
    },
    {
      action: "retry_parse",
      committed: false,
      detail: "PDF parser 失败，可保留 raw source 后重试解析。",
      displayName: failedFile.display_name,
      manifest: failedManifest,
      selection: failedFile,
      stage: "failed",
      taskId: "task_import_pdf",
    },
  ];
}

export function createMvpScreenModel(
  workspace: WorkspaceDTO | undefined = previewWorkspace,
  daemonStage: DaemonConnectionStatus = "daemon.ready",
): MvpScreenModel {
  const resolvedWorkspace = workspace ?? previewWorkspace;
  const searchResults = createSearchResultsModel({
    filteredByPermission: 1,
    query: "workspace source boundary",
    results: [staleSearchResult, filteredSearchResult],
  });
  const citationPreview = createCitationPreviewModel(
    filteredSearchResult.citation_refs[0] as CitationRefDTO,
    null,
  );

  return {
    answer: createAnswerModel({
      answerId: "answer_preview_1",
      citationRefs: [staleSearchResult.citation_refs[0] as CitationRefDTO],
      status: "composed",
      text: "根据本机导入范围限制，当前 workspace 选择文件只能来自已授权路径。",
    }),
    citationPreview,
    daemonStatus: {
      auditMode: resolvedWorkspace.state === "audit_readonly" ? "readonly" : "writable",
      canOpenLogs: true,
      canReconnect: daemonStage !== "daemon.ready",
      detail:
        daemonStage === "daemon.ready"
          ? "daemon ready; replayable events are current"
          : "daemon connection requires recovery",
      heartbeatAt: "2026-04-28T00:00:04.000Z",
      status: resolvedWorkspace.state === "degraded" ? "daemon.degraded" : daemonStage,
    },
    importQueue: createImportQueueModel(),
    searchResults,
    wikiReader: createWikiReaderModel(resolvedWorkspace, [
      staleSearchResult.citation_refs[0] as CitationRefDTO,
      filteredSearchResult.citation_refs[0] as CitationRefDTO,
    ]),
    workspaceShell: createWorkspaceShellModel(resolvedWorkspace),
  };
}
