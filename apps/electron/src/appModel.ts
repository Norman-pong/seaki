import { createDomainClient, createDomainRuntime } from "@seaki/domain";
import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";
import type {
  ApprovalDecisionResultDTO,
  ApprovalReviewDTO,
  ApprovalRequestDTO,
  CitationValidationDTO,
  SourceCardDTO,
  WikiPatchProposalDTO,
} from "@seaki/dto";
import { createMockTransportClient } from "@seaki/transport";
import type { FrontendTransportEvent } from "@seaki/transport";
import type { ClaimApprovalDecisionInput, DecideApprovalInput, DomainClient } from "@seaki/domain";
import { createMvpScreenModel } from "./mvpScreenModel";
import type { MvpScreenModel } from "./mvpScreenModel";

export type ApprovalResult =
  | "pending"
  | "approved"
  | "applying"
  | "committed"
  | "rejected"
  | "conflict"
  | "expired";

export type CitationValidationState = CitationValidationDTO["state"];

export interface ApprovalClaimModel {
  readonly claimId: string;
  readonly title: string;
  readonly statement: string;
  readonly status: ApprovalResult;
  readonly citationId: string;
  readonly citationState: CitationValidationState;
  readonly citationReason: string | null;
  readonly sourceRange: string;
  readonly sourceExcerpt: string;
  readonly riskLevel: "low" | "medium" | "high";
  readonly riskSummary: string;
  readonly taintFlags: readonly string[];
  readonly securityFlags: readonly string[];
  readonly rejectionReason: string | null;
}

export interface PatchDiffLine {
  readonly id: string;
  readonly kind: "context" | "add" | "remove";
  readonly text: string;
}

export interface ApprovalDiffModel {
  readonly approvalRequest: ApprovalRequestDTO;
  readonly claims: readonly ApprovalClaimModel[];
  readonly patch: WikiPatchProposalDTO;
  readonly patchLines: readonly PatchDiffLine[];
  readonly rejectionDrafts: Readonly<Record<string, string>>;
  readonly source: SourceCardDTO;
  readonly status: ApprovalResult;
  readonly statusCounts: Readonly<Record<ApprovalResult, number>>;
}

export interface ElectronAppModel extends MvpScreenModel {
  readonly approval: ApprovalDiffModel;
  readonly importStage: string;
  readonly pipelineStage: string;
  readonly memoryStage: string;
  readonly channelStage: string;
  readonly workspaceStage: string;
  readonly workspaceTitle: string;
}

export type ApprovalActionClient = Pick<DomainClient, "approval">;

const PREVIEW_WORKSPACE_ID = "ws_local_preview";

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
        reason: "index_stale",
        workspace: {
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
        },
      }),
      previewEvent(3, "import.stage.changed", {
        stage: "selected",
      }),
    ],
    responder(record) {
      if (record.method === "approval.reviewPatch") {
        return approvalReview();
      }

      if (record.method === "approval.decide") {
        return approvalDecisionResult(record.input as DecideApprovalInput);
      }

      return undefined;
    },
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
  const review = await runtime.client.approval.reviewPatch({
    patchId: approvalRequest.patch_id,
    workspaceId: PREVIEW_WORKSPACE_ID,
  });
  await runtime.replay(0);

  const snapshot = runtime.store.getSnapshot();

  return {
    approval: createApprovalDiffModel(review),
    ...createMvpScreenModel(snapshot.workspace.dto, snapshot.appBoot.stage),
    importStage: snapshot.imports[0]?.stage ?? "selected",
    pipelineStage: snapshot.pipelines[0]?.status ?? "composing",
    memoryStage: snapshot.memories[0]?.status ?? "idle",
    channelStage: snapshot.channels[0]?.outboxStatus ?? "idle",
    workspaceStage: snapshot.workspace.stage,
    workspaceTitle: snapshot.workspace.dto ? "ws_local_preview" : "ws_local_preview",
  };
}

const APPROVAL_RESULTS: readonly ApprovalResult[] = [
  "pending",
  "approved",
  "applying",
  "committed",
  "rejected",
  "conflict",
  "expired",
];

const sourceCard: SourceCardDTO = {
  citation_refs: [
    {
      citation_id: "cit_decision_context",
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
    },
    {
      citation_id: "cit_risk_boundary",
      claim_id: "claim_policy_boundary",
      degraded_reason: null,
      range: {
        end: 35,
        label: "L28-L35",
        start: 28,
        unit: "line",
      },
      source_id: "src_local_note_20260428",
      wiki_page_id: "wiki_m0_decision",
    },
    {
      citation_id: "cit_parser_warning",
      claim_id: "claim_pdf_warning",
      degraded_reason: "source parser reported active-content markers",
      range: {
        end: 2,
        label: "P2",
        start: 2,
        unit: "page",
      },
      source_id: "src_local_note_20260428",
      wiki_page_id: "wiki_m0_decision",
    },
  ],
  origin_display: "本机资料 / 2026-04-28-import.md",
  range: {
    end: 35,
    label: "L12-L35",
    start: 12,
    unit: "line",
  },
  source_id: "src_local_note_20260428",
  summary: "M0 本机 source 导入后生成的 DecisionRecord 候选证据。",
  title: "2026-04-28 本机导入记录",
  visibility: "visible",
};

const decisionRange = {
  end: 18,
  label: "L12-L18",
  start: 12,
  unit: "line",
} as const;

const riskRange = {
  end: 35,
  label: "L28-L35",
  start: 28,
  unit: "line",
} as const;

const parserWarningRange = {
  end: 2,
  label: "P2",
  start: 2,
  unit: "page",
} as const;

const citationValidation: readonly CitationValidationDTO[] = [
  {
    citation_id: "cit_decision_context",
    cited_ranges: [decisionRange],
    claim_id: "claim_source_scope",
    evidence: [
      {
        citation_id: "cit_decision_context",
        cited_ranges: [decisionRange],
        degraded_reason: null,
        excerpt: "本次导入只覆盖 workspace 内选择的 Markdown 资料。",
        range: decisionRange,
        source_id: sourceCard.source_id,
        source_title: sourceCard.title,
        visibility: "visible",
      },
    ],
    reason: null,
    security_flags: ["policy_review_required"],
    state: "valid",
    taint_flags: ["untrusted_content"],
  },
  {
    citation_id: "cit_risk_boundary",
    cited_ranges: [riskRange],
    claim_id: "claim_policy_boundary",
    evidence: [
      {
        citation_id: "cit_risk_boundary",
        cited_ranges: [riskRange],
        degraded_reason: null,
        excerpt: "patch proposal 必须经过 policy、approval、WAL 后才能提交。",
        range: riskRange,
        source_id: sourceCard.source_id,
        source_title: sourceCard.title,
        visibility: "visible",
      },
    ],
    reason: null,
    security_flags: ["audit_wal_required"],
    state: "valid",
    taint_flags: ["untrusted_content"],
  },
  {
    citation_id: "cit_parser_warning",
    cited_ranges: [parserWarningRange],
    claim_id: "claim_pdf_warning",
    evidence: [
      {
        citation_id: "cit_parser_warning",
        cited_ranges: [parserWarningRange],
        degraded_reason: "source parser reported active-content markers",
        excerpt: "PDF frame 含 active content 标记，parser 已剥离并降级。",
        range: parserWarningRange,
        source_id: sourceCard.source_id,
        source_title: sourceCard.title,
        visibility: "visible",
      },
    ],
    reason: "parser warning: embedded action marker stripped",
    security_flags: ["active_content_stripped", "manual_review"],
    state: "degraded",
    taint_flags: ["untrusted_content", "degraded_citation"],
  },
  {
    citation_id: "cit_stale_base",
    cited_ranges: [],
    claim_id: "claim_stale_base",
    evidence: [],
    reason: "base revision wiki_rev_0 is no longer head",
    security_flags: ["base_revision_mismatch"],
    state: "invalid",
    taint_flags: ["derived_state"],
  },
  {
    citation_id: "cit_expired_review",
    cited_ranges: [],
    claim_id: "claim_expired_review",
    evidence: [],
    reason: "approval ttl elapsed",
    security_flags: ["approval_ttl_expired"],
    state: "degraded",
    taint_flags: ["approval_state"],
  },
];

const patchText = [
  "@@ DecisionRecord: M0 local ingest @@",
  "- status: draft",
  "+ status: proposed",
  "+ evidence:",
  "+   - cit_decision_context",
  "+   - cit_risk_boundary",
  "+ security:",
  "+   taint: untrusted_content",
  "+   flags: parser_sanitized, policy_review_required",
].join("\n");

const patchRisk = {
  factors: ["wiki_revision_write", "citation_degraded", "wal_required"],
  level: "high",
  requires_manual_approval: true,
  summary: "写入 typed wiki page，引用本机 source range；需要人工确认降级 citation。",
} as const;

const patch: WikiPatchProposalDTO = {
  base_revision: "wiki_rev_0",
  citation_validation: citationValidation,
  claim_ids: [
    "claim_source_scope",
    "claim_policy_boundary",
    "claim_pdf_warning",
    "claim_stale_base",
    "claim_expired_review",
  ],
  claims: [
    {
      citation_ids: ["cit_decision_context"],
      citation_validation: [citationValidation[0] as CitationValidationDTO],
      claim_id: "claim_source_scope",
      page_id: "wiki_m0_decision",
      risk_summary: {
        factors: ["wiki_page_add"],
        level: "low",
        requires_manual_approval: true,
        summary: "只新增 DecisionRecord 证据引用，不触发外部副作用。",
      },
      security_flags: ["policy_review_required"],
      taint_flags: ["untrusted_content"],
      text: "本机导入范围限制在当前 workspace 选择文件。",
    },
    {
      citation_ids: ["cit_risk_boundary"],
      citation_validation: [citationValidation[1] as CitationValidationDTO],
      claim_id: "claim_policy_boundary",
      page_id: "wiki_m0_decision",
      risk_summary: {
        factors: ["search_candidate_visibility", "wal_required"],
        level: "medium",
        requires_manual_approval: true,
        summary: "会让 wiki revision 进入可搜索候选，commit 前必须审批。",
      },
      security_flags: ["audit_wal_required"],
      taint_flags: ["untrusted_content"],
      text: "wiki patch 不能绕过 approval/WAL 直接提交。",
    },
    {
      citation_ids: ["cit_parser_warning"],
      citation_validation: [citationValidation[2] as CitationValidationDTO],
      claim_id: "claim_pdf_warning",
      page_id: "wiki_m0_decision",
      risk_summary: {
        factors: ["degraded_citation", "parser_sanitized"],
        level: "high",
        requires_manual_approval: true,
        summary: "证据来自降级 frame，保留审计但不应自动批准。",
      },
      security_flags: ["active_content_stripped", "manual_review"],
      taint_flags: ["untrusted_content", "degraded_citation"],
      text: "降级 citation 只能作为待复核证据进入 proposal。",
    },
  ],
  diff: {
    added_lines: 7,
    affected_paths: ["wiki/decision-records/m0-local-ingest.md"],
    format: "unified",
    removed_lines: 1,
    text: patchText,
  },
  patch_id: "patch_decision_record_import",
  risk_summary: patchRisk,
  security_flags: ["parser_sanitized", "policy_review_required"],
  taint_flags: ["untrusted_content"],
};

const approvalRequest: ApprovalRequestDTO = {
  approval_id: "appr_m0_07_local",
  audit_id: null,
  claim_decisions: [],
  expires_at: "2026-04-28T18:30:00.000+08:00",
  patch_id: patch.patch_id,
  policy_decision: "requires_approval",
  proposal: patch,
  rejection_reason: null,
  required_by: "wiki.patch.transaction",
  status: "pending",
  wal_entry_id: null,
};

function approvalReview(): ApprovalReviewDTO {
  return {
    proposal: patch,
    request: approvalRequest,
  };
}

function approvalDecisionResult(input: DecideApprovalInput): ApprovalDecisionResultDTO {
  const rejected = input.decision === "reject";
  const decidedAt = "2026-04-28T18:15:00.000+08:00";

  return {
    approval_id: input.approvalId,
    audit_id: `audit_${input.approvalId}`,
    claim_decisions:
      input.claimDecisions?.map((decision) => ({
        claim_id: decision.claimId,
        decided_at: decidedAt,
        decided_by: "electron-preview",
        decision: decision.decision,
        reason: decision.reason ?? null,
      })) ?? [],
    committed_revision: rejected ? null : "wiki_rev_preview_next",
    denied_reason: rejected ? (input.reason ?? "claim rejected") : null,
    patch_id: approvalRequest.patch_id,
    rejection_reason: rejected ? (input.reason ?? "claim rejected") : null,
    status: rejected ? "rejected" : "approved",
    transaction_id: rejected ? null : "txn_preview_approval",
    wal_entry_id: `wal_${input.approvalId}`,
  };
}

const claims: readonly ApprovalClaimModel[] = [
  {
    citationId: "cit_decision_context",
    citationReason: null,
    citationState: "valid",
    claimId: "claim_source_scope",
    rejectionReason: null,
    riskLevel: "low",
    riskSummary: "只新增 DecisionRecord 证据引用，不触发外部副作用。",
    securityFlags: ["policy_review_required"],
    sourceExcerpt: "本次导入只覆盖 workspace 内选择的 Markdown 资料。",
    sourceRange: "L12-L18",
    statement: "本机导入范围限制在当前 workspace 选择文件。",
    status: "pending",
    taintFlags: ["untrusted_content"],
    title: "source scope",
  },
  {
    citationId: "cit_risk_boundary",
    citationReason: null,
    citationState: "valid",
    claimId: "claim_policy_boundary",
    rejectionReason: null,
    riskLevel: "medium",
    riskSummary: "会让 wiki revision 进入可搜索候选，commit 前必须审批。",
    securityFlags: ["audit_wal_required"],
    sourceExcerpt: "patch proposal 必须经过 policy、approval、WAL 后才能提交。",
    sourceRange: "L28-L35",
    statement: "wiki patch 不能绕过 approval/WAL 直接提交。",
    status: "pending",
    taintFlags: ["untrusted_content"],
    title: "approval boundary",
  },
  {
    citationId: "cit_parser_warning",
    citationReason: "parser warning: embedded action marker stripped",
    citationState: "degraded",
    claimId: "claim_pdf_warning",
    rejectionReason: null,
    riskLevel: "high",
    riskSummary: "证据来自降级 frame，保留审计但不应自动批准。",
    securityFlags: ["active_content_stripped", "manual_review"],
    sourceExcerpt: "PDF frame 含 active content 标记，parser 已剥离并降级。",
    sourceRange: "P2",
    statement: "降级 citation 只能作为待复核证据进入 proposal。",
    status: "pending",
    taintFlags: ["untrusted_content", "degraded_citation"],
    title: "parser warning",
  },
  {
    citationId: "cit_stale_base",
    citationReason: "base revision wiki_rev_0 is no longer head",
    citationState: "invalid",
    claimId: "claim_stale_base",
    rejectionReason: null,
    riskLevel: "medium",
    riskSummary: "base revision 过旧，需要重新生成 patch。",
    securityFlags: ["base_revision_mismatch"],
    sourceExcerpt: "已有更新写入 wiki_rev_1，当前 proposal 不能直接 apply。",
    sourceRange: "wiki_rev_1",
    statement: "过旧 base revision 会阻止本次 transaction commit。",
    status: "conflict",
    taintFlags: ["derived_state"],
    title: "base conflict",
  },
  {
    citationId: "cit_expired_review",
    citationReason: "approval ttl elapsed",
    citationState: "degraded",
    claimId: "claim_expired_review",
    rejectionReason: null,
    riskLevel: "low",
    riskSummary: "审批窗口已过期，必须重新请求 approval。",
    securityFlags: ["approval_ttl_expired"],
    sourceExcerpt: "旧 approval request 超过 TTL 后不能复用。",
    sourceRange: "approval ttl",
    statement: "过期审批请求不能进入 apply 阶段。",
    status: "expired",
    taintFlags: ["approval_state"],
    title: "expired approval",
  },
  {
    citationId: "cit_committed_prior",
    citationReason: null,
    citationState: "valid",
    claimId: "claim_committed_prior",
    rejectionReason: null,
    riskLevel: "low",
    riskSummary: "前序审批已提交，用于展示 committed 结果态。",
    securityFlags: ["audit_recorded"],
    sourceExcerpt: "audit head 已记录上一条 claim commit。",
    sourceRange: "audit#7",
    statement: "已提交 claim 必须显示为只读结果。",
    status: "committed",
    taintFlags: ["audit_state"],
    title: "committed claim",
  },
];

function countStatuses(
  nextClaims: readonly ApprovalClaimModel[],
): Readonly<Record<ApprovalResult, number>> {
  const counts = Object.fromEntries(APPROVAL_RESULTS.map((status) => [status, 0])) as Record<
    ApprovalResult,
    number
  >;

  for (const claim of nextClaims) {
    counts[claim.status] += 1;
  }

  return counts;
}

export function deriveApprovalStatus(nextClaims: readonly ApprovalClaimModel[]): ApprovalResult {
  if (nextClaims.some((claim) => claim.status === "applying")) {
    return "applying";
  }

  if (nextClaims.some((claim) => claim.status === "conflict")) {
    return "conflict";
  }

  if (nextClaims.some((claim) => claim.status === "expired")) {
    return "expired";
  }

  if (nextClaims.some((claim) => claim.status === "pending")) {
    return "pending";
  }

  if (nextClaims.some((claim) => claim.status === "rejected")) {
    return "rejected";
  }

  if (nextClaims.every((claim) => claim.status === "committed")) {
    return "committed";
  }

  return "approved";
}

function buildApprovalDiffModel(
  nextClaims: readonly ApprovalClaimModel[],
  rejectionDrafts: Readonly<Record<string, string>> = {},
  review: ApprovalReviewDTO = approvalReview(),
): ApprovalDiffModel {
  return {
    approvalRequest: review.request,
    claims: nextClaims,
    patch: review.proposal,
    patchLines: patchLinesFrom(review.proposal),
    rejectionDrafts,
    source: sourceCard,
    status: deriveApprovalStatus(nextClaims),
    statusCounts: countStatuses(nextClaims),
  };
}

export function createApprovalDiffModel(
  review: ApprovalReviewDTO = approvalReview(),
): ApprovalDiffModel {
  return buildApprovalDiffModel(claims, {}, review);
}

export function updateRejectionDraft(
  model: ApprovalDiffModel,
  claimId: string,
  reason: string,
): ApprovalDiffModel {
  return buildApprovalDiffModel(
    model.claims,
    {
      ...model.rejectionDrafts,
      [claimId]: reason,
    },
    {
      proposal: model.patch,
      request: model.approvalRequest,
    },
  );
}

export function approvePendingClaims(model: ApprovalDiffModel): ApprovalDiffModel {
  const nextClaims = model.claims.map((claim) => {
    if (!canBatchApproveClaim(claim)) {
      return claim;
    }

    return {
      ...claim,
      rejectionReason: null,
      status: "approved" as const,
    };
  });

  return rebuildFromModel(model, nextClaims);
}

export function approveClaim(model: ApprovalDiffModel, claimId: string): ApprovalDiffModel {
  const nextClaims = model.claims.map((claim) => {
    if (claim.claimId !== claimId || !canApproveClaim(claim)) {
      return claim;
    }

    return {
      ...claim,
      rejectionReason: null,
      status: "approved" as const,
    };
  });

  return rebuildFromModel(model, nextClaims);
}

export function applyApprovedClaims(model: ApprovalDiffModel): ApprovalDiffModel {
  const nextClaims = model.claims.map((claim) => {
    if (claim.status !== "approved" && claim.status !== "applying") {
      return claim;
    }

    return {
      ...claim,
      status: "committed" as const,
    };
  });

  return rebuildFromModel(model, nextClaims);
}

export function markApprovedClaimsApplying(model: ApprovalDiffModel): ApprovalDiffModel {
  const nextClaims = model.claims.map((claim) => {
    if (claim.status !== "approved") {
      return claim;
    }

    return {
      ...claim,
      status: "applying" as const,
    };
  });

  return rebuildFromModel(model, nextClaims);
}

export function rejectClaim(model: ApprovalDiffModel, claimId: string): ApprovalDiffModel {
  const reason = model.rejectionDrafts[claimId]?.trim() ?? "";

  if (!reason) {
    return model;
  }

  const nextClaims = model.claims.map((claim) => {
    if (claim.claimId !== claimId || claim.status === "committed") {
      return claim;
    }

    return {
      ...claim,
      rejectionReason: reason,
      status: "rejected" as const,
    };
  });

  return rebuildFromModel(model, nextClaims, {
    ...model.rejectionDrafts,
    [claimId]: "",
  });
}

export async function approvePendingClaimsViaDomain(
  model: ApprovalDiffModel,
  client: ApprovalActionClient = createPreviewApprovalClient(),
): Promise<ApprovalDiffModel> {
  const claimDecisions = model.claims
    .filter(canBatchApproveClaim)
    .map((claim) => claimDecisionInput(claim, "approve"));

  if (claimDecisions.length === 0) {
    return model;
  }

  await client.approval.decide(decideInput(model, "approve", claimDecisions));

  return approvePendingClaims(model);
}

export async function approveClaimViaDomain(
  model: ApprovalDiffModel,
  claimId: string,
  client: ApprovalActionClient = createPreviewApprovalClient(),
): Promise<ApprovalDiffModel> {
  const claim = model.claims.find((item) => item.claimId === claimId);
  if (!claim || !canApproveClaim(claim)) {
    return model;
  }

  await client.approval.decide(
    decideInput(model, "approve", [claimDecisionInput(claim, "approve")]),
  );

  return approveClaim(model, claimId);
}

export async function markApprovedClaimsApplyingViaDomain(
  model: ApprovalDiffModel,
  client: ApprovalActionClient = createPreviewApprovalClient(),
): Promise<ApprovalDiffModel> {
  const claimDecisions = model.claims
    .filter((claim) => claim.status === "approved")
    .map((claim) => claimDecisionInput(claim, "approve"));

  if (claimDecisions.length === 0) {
    return model;
  }

  await client.approval.decide(decideInput(model, "approve", claimDecisions));

  return markApprovedClaimsApplying(model);
}

export async function applyApprovedClaimsViaDomain(
  model: ApprovalDiffModel,
  client: ApprovalActionClient = createPreviewApprovalClient(),
): Promise<ApprovalDiffModel> {
  const claimDecisions = model.claims
    .filter((claim) => claim.status === "approved" || claim.status === "applying")
    .map((claim) => claimDecisionInput(claim, "approve"));

  if (claimDecisions.length === 0) {
    return model;
  }

  await client.approval.decide(decideInput(model, "approve", claimDecisions));

  return applyApprovedClaims(model);
}

export async function rejectClaimViaDomain(
  model: ApprovalDiffModel,
  claimId: string,
  client: ApprovalActionClient = createPreviewApprovalClient(),
): Promise<ApprovalDiffModel> {
  const reason = model.rejectionDrafts[claimId]?.trim() ?? "";
  const claim = model.claims.find((item) => item.claimId === claimId);

  if (!claim || !reason || claim.status === "committed") {
    return model;
  }

  await client.approval.decide(
    decideInput(model, "reject", [claimDecisionInput(claim, "reject", reason)], reason),
  );

  return rejectClaim(model, claimId);
}

function rebuildFromModel(
  model: ApprovalDiffModel,
  nextClaims: readonly ApprovalClaimModel[],
  rejectionDrafts: Readonly<Record<string, string>> = model.rejectionDrafts,
): ApprovalDiffModel {
  return buildApprovalDiffModel(nextClaims, rejectionDrafts, {
    proposal: model.patch,
    request: model.approvalRequest,
  });
}

function patchLinesFrom(proposal: WikiPatchProposalDTO): readonly PatchDiffLine[] {
  const diffText = typeof proposal.diff === "string" ? proposal.diff : proposal.diff.text;

  return diffText.split("\n").map((text, index) => ({
    id: `diff_${index}`,
    kind: text.startsWith("+") ? "add" : text.startsWith("-") ? "remove" : "context",
    text,
  }));
}

function canBatchApproveClaim(claim: ApprovalClaimModel): boolean {
  return (
    canApproveClaim(claim) &&
    claim.citationState === "valid" &&
    claim.riskLevel !== "high" &&
    !claim.securityFlags.includes("manual_review")
  );
}

function canApproveClaim(claim: ApprovalClaimModel): boolean {
  return claim.status === "pending" && claim.citationState !== "invalid";
}

function claimDecisionInput(
  claim: ApprovalClaimModel,
  decision: ClaimApprovalDecisionInput["decision"],
  reason?: string,
): ClaimApprovalDecisionInput {
  return {
    claimId: claim.claimId,
    decision,
    ...(reason ? { reason } : {}),
  };
}

function decideInput(
  model: ApprovalDiffModel,
  decision: DecideApprovalInput["decision"],
  claimDecisions: readonly ClaimApprovalDecisionInput[],
  reason?: string,
): DecideApprovalInput {
  return {
    approvalId: model.approvalRequest.approval_id,
    claimDecisions,
    decision,
    ...(reason ? { reason } : {}),
    workspaceId: PREVIEW_WORKSPACE_ID,
  };
}

function createPreviewApprovalClient(): ApprovalActionClient {
  return createDomainClient(
    createMockTransportClient({
      responder(record) {
        if (record.method === "approval.reviewPatch") {
          return approvalReview();
        }

        if (record.method === "approval.decide") {
          return approvalDecisionResult(record.input as DecideApprovalInput);
        }

        return undefined;
      },
    }),
  );
}
