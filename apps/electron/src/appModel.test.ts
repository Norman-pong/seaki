import { describe, expect, it } from "vitest";

import {
  applyApprovedClaims,
  approveClaim,
  approveClaimViaDomain,
  approvePendingClaims,
  approvePendingClaimsViaDomain,
  createApprovalDiffModel,
  createElectronAppModel,
  markApprovedClaimsApplying,
  rejectClaim,
  rejectClaimViaDomain,
  updateRejectionDraft,
} from "./appModel";
import type { ApprovalActionClient } from "./appModel";
import {
  createCitationPreviewModel,
  createMvpScreenModel,
  createSearchResultsModel,
  createWikiReaderModel,
} from "./mvpScreenModel";
import type { DecideApprovalInput } from "@seaki/domain";
import type { CitationRefDTO, SearchResultDTO, SourceCardDTO, WorkspaceDTO } from "@seaki/dto";

describe("createElectronAppModel", () => {
  it("binds Electron preview state to domain/state packages", async () => {
    const model = await createElectronAppModel();

    expect(model.importStage).toBe("selected");
    expect(model.workspaceStage).toBe("degraded");
    expect(model.workspaceTitle).toBe("ws_local_preview");
    expect(model.daemonStatus.status).toBe("daemon.degraded");
    expect(model.workspaceShell.indexStatus.state).toBe("stale");
    expect(model.workspaceShell.canRebuildIndex).toBe(true);
    expect(model.importQueue.map((item) => item.stage)).toEqual(["index_stale", "failed"]);
    expect(model.searchResults.status).toBe("stale");
    expect(model.searchResults.filteredByPermission).toBe(1);
    expect(model.wikiReader.draftVisible).toBe(false);
    expect(model.citationPreview.status).toBe("no_access");
    expect(model.approval.approvalRequest.policy_decision).toBe("requires_approval");
    expect(model.approval.patch.claim_ids).toContain("claim_source_scope");
    expect(model.approval.source.citation_refs).toHaveLength(3);
  });

  it("keeps draft wiki content hidden from committed reader state", () => {
    const workspace = workspaceDto("fresh");
    const reader = createWikiReaderModel(workspace, [citationRef()]);

    expect(reader.status).toBe("committed");
    expect(reader.committedRevision).toBe("wiki_rev_committed");
    expect(reader.draftVisible).toBe(false);
    expect(reader.warning).toBeNull();
  });

  it("models stale and permission-filtered search results without restricted snippets", () => {
    const stale = searchResult("result_stale", "stale", "visible snippet", "index_stale");
    const hidden = searchResult("result_hidden", "fresh", null, "no_access");
    const search = createSearchResultsModel({
      query: "workspace source boundary",
      results: [stale, hidden],
    });

    expect(search.status).toBe("stale");
    expect(search.filteredByPermission).toBe(1);
    expect(search.results[1]?.snippet).toBeNull();
    expect(search.results[1]?.citation_refs[0]?.degraded_reason).toBe("no_access");
  });

  it("models citation preview degraded and no_access recovery states", () => {
    const visibleSource = sourceCard("visible");
    const degraded = createCitationPreviewModel(
      {
        ...citationRef(),
        degraded_reason: "index_stale",
      },
      visibleSource,
    );
    const noAccess = createCitationPreviewModel(citationRef(), sourceCard("restricted"));

    expect(degraded).toMatchObject({
      recoverability: "inspect_source",
      status: "degraded",
    });
    expect(noAccess).toMatchObject({
      preview: null,
      recoverability: "request_access",
      status: "no_access",
    });
  });

  it("exposes all M0-09 screen contracts in a single Electron model", () => {
    const screens = createMvpScreenModel(workspaceDto("stale"));

    expect(Object.keys(screens).sort()).toEqual([
      "answer",
      "channelOutbox",
      "citationPreview",
      "daemonStatus",
      "importQueue",
      "memoryBrowser",
      "pipelineDryRun",
      "searchResults",
      "wikiReader",
      "workspaceShell",
    ]);
    expect(screens.workspaceShell.degradedReasons).toContain("index_stale");
    expect(screens.importQueue.some((item) => item.action === "retry_parse")).toBe(true);
  });
});

describe("ApprovalDiff model actions", () => {
  it("includes every M0-07 approval result state across the mock/action flow", () => {
    const initial = createApprovalDiffModel();
    const approved = approvePendingClaims(initial);
    const applying = markApprovedClaimsApplying(approved);
    const rejected = rejectClaim(
      updateRejectionDraft(initial, "claim_source_scope", "source range too broad"),
      "claim_source_scope",
    );

    const statuses = new Set([
      ...initial.claims.map((claim) => claim.status),
      ...approved.claims.map((claim) => claim.status),
      ...applying.claims.map((claim) => claim.status),
      ...rejected.claims.map((claim) => claim.status),
    ]);

    expect(statuses).toEqual(
      new Set(["pending", "approved", "applying", "committed", "rejected", "conflict", "expired"]),
    );
  });

  it("batch approves only low-risk valid pending claims", () => {
    const initial = createApprovalDiffModel();
    const next = approvePendingClaims(initial);
    const degradedClaim = next.claims.find((claim) => claim.claimId === "claim_pdf_warning");

    expect(next).not.toBe(initial);
    expect(next.statusCounts.approved).toBe(2);
    expect(degradedClaim?.status).toBe("pending");
    expect(next.statusCounts.conflict).toBe(1);
    expect(next.statusCounts.expired).toBe(1);
    expect(next.statusCounts.committed).toBe(1);
    expect(initial.statusCounts.pending).toBe(3);
  });

  it("moves approved claims through applying to committed", () => {
    const approved = approvePendingClaims(createApprovalDiffModel());
    const applying = markApprovedClaimsApplying(approved);
    const committed = applyApprovedClaims(applying);

    expect(applying.statusCounts.applying).toBe(2);
    expect(committed.statusCounts.committed).toBe(3);
    expect(committed.statusCounts.applying).toBe(0);
  });

  it("allows a single explicit approval for degraded manual-review claims", () => {
    const next = approveClaim(createApprovalDiffModel(), "claim_pdf_warning");
    const claim = next.claims.find((item) => item.claimId === "claim_pdf_warning");

    expect(claim?.status).toBe("approved");
  });

  it("requires a rejection reason before rejecting a single claim", () => {
    const initial = createApprovalDiffModel();
    const unchanged = rejectClaim(initial, "claim_source_scope");
    const withDraft = updateRejectionDraft(initial, "claim_source_scope", "citation mismatch");
    const rejected = rejectClaim(withDraft, "claim_source_scope");
    const rejectedClaim = rejected.claims.find((claim) => claim.claimId === "claim_source_scope");

    expect(unchanged).toBe(initial);
    expect(rejectedClaim?.status).toBe("rejected");
    expect(rejectedClaim?.rejectionReason).toBe("citation mismatch");
    expect(rejected.rejectionDrafts.claim_source_scope).toBe("");
  });

  it("routes approval actions through the domain use case", async () => {
    const decisions: DecideApprovalInput[] = [];
    const client: ApprovalActionClient = {
      approval: {
        async decide(input) {
          decisions.push(input);

          return {
            approval_id: input.approvalId,
            audit_id: `audit_${input.approvalId}`,
            claim_decisions:
              input.claimDecisions?.map((decision) => ({
                claim_id: decision.claimId,
                decided_at: "2026-04-28T18:15:00.000+08:00",
                decided_by: "test",
                decision: decision.decision,
                reason: decision.reason ?? null,
              })) ?? [],
            committed_revision: null,
            denied_reason: input.decision === "reject" ? (input.reason ?? null) : null,
            patch_id: "patch_decision_record_import",
            rejection_reason: input.decision === "reject" ? (input.reason ?? null) : null,
            status: input.decision === "reject" ? "rejected" : "approved",
            transaction_id: null,
            wal_entry_id: `wal_${input.approvalId}`,
          };
        },
        async reviewPatch() {
          throw new Error("not needed");
        },
      },
    };

    const initial = createApprovalDiffModel();
    const batchApproved = await approvePendingClaimsViaDomain(initial, client);
    const singleApproved = await approveClaimViaDomain(initial, "claim_pdf_warning", client);
    const rejected = await rejectClaimViaDomain(
      updateRejectionDraft(initial, "claim_source_scope", "citation mismatch"),
      "claim_source_scope",
      client,
    );

    expect(decisions).toHaveLength(3);
    expect(decisions[0]?.claimDecisions?.map((decision) => decision.claimId)).toEqual([
      "claim_source_scope",
      "claim_policy_boundary",
    ]);
    expect(decisions[1]?.claimDecisions?.[0]).toMatchObject({
      claimId: "claim_pdf_warning",
      decision: "approve",
    });
    expect(decisions[2]).toMatchObject({
      decision: "reject",
      reason: "citation mismatch",
    });
    expect(batchApproved.statusCounts.approved).toBe(2);
    expect(
      singleApproved.claims.find((claim) => claim.claimId === "claim_pdf_warning")?.status,
    ).toBe("approved");
    expect(rejected.claims.find((claim) => claim.claimId === "claim_source_scope")?.status).toBe(
      "rejected",
    );
  });
});

function workspaceDto(state: "fresh" | "stale"): WorkspaceDTO {
  return {
    audit_head: "audit_head",
    current_revision: "wiki_rev_committed",
    index_status: {
      last_good_revision: state === "fresh" ? "wiki_rev_committed" : "wiki_rev_previous",
      stale_reason: state === "stale" ? "source visibility changed" : null,
      state,
      updated_at: state === "fresh" ? "2026-04-28T00:10:00.000Z" : null,
    },
    root_uri: "file:///workspace",
    state: state === "stale" ? "degraded" : "ready",
    workspace_id: "ws_test",
  };
}

function citationRef(): CitationRefDTO {
  return {
    citation_id: "cite_test",
    claim_id: "claim_test",
    degraded_reason: null,
    range: {
      end: 4,
      label: "source.md:1-4",
      start: 1,
      unit: "line",
    },
    source_id: "source_test",
    wiki_page_id: "wiki_test",
  };
}

function searchResult(
  resultId: string,
  state: "fresh" | "stale",
  snippet: string | null,
  degradedReason: string | null,
): SearchResultDTO {
  return {
    citation_refs: [
      {
        ...citationRef(),
        citation_id: `cite_${resultId}`,
        degraded_reason: degradedReason,
      },
    ],
    index_status: {
      last_good_revision: "wiki_rev_committed",
      stale_reason: state === "stale" ? "index stale" : null,
      state,
      updated_at: state === "fresh" ? "2026-04-28T00:10:00.000Z" : null,
    },
    kind: "claim",
    result_id: resultId,
    snippet,
    title: resultId,
  };
}

function sourceCard(visibility: SourceCardDTO["visibility"]): SourceCardDTO {
  return {
    citation_refs: [citationRef()],
    origin_display: "source.md",
    range: {
      end: 4,
      label: "source.md:1-4",
      start: 1,
      unit: "line",
    },
    source_id: "source_test",
    summary: "Visible source excerpt.",
    title: "source.md",
    visibility,
  };
}
