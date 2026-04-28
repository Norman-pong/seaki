import { describe, expect, it } from "vitest";

import { M0_DOMAIN_USE_CASE_METHODS, SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";
import type {
  ApprovalDecisionResultDTO,
  ApprovalReviewDTO,
  SearchResultDTO,
  WikiPatchProposalDTO,
} from "@seaki/dto";
import { createMockTransportClient } from "@seaki/transport";
import type { FrontendTransportEvent } from "@seaki/transport";

import {
  bootApp,
  createDomainClient,
  createDomainRuntime,
  initWorkspaceShell,
  prepareUserSelectedFile,
} from "./index";

function event(
  seq: number,
  eventType: string,
  payload: Record<string, unknown> = {},
): FrontendTransportEvent {
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

function patchProposal(): WikiPatchProposalDTO {
  const range = {
    end: 18,
    label: "README.md:1-2",
    start: 0,
    unit: "line" as const,
  };
  const validation = {
    citation_id: "cite_1",
    claim_id: "claim_1",
    cited_ranges: [range],
    evidence: [
      {
        citation_id: "cite_1",
        cited_ranges: [range],
        degraded_reason: null,
        excerpt: "seaki requires cited claims.",
        range,
        source_id: "source_1",
        source_title: "README.md",
        visibility: "visible" as const,
      },
    ],
    reason: null,
    security_flags: ["no_active_content"],
    state: "valid" as const,
    taint_flags: ["untrusted_content"],
  };
  const risk = {
    factors: ["updates committed wiki page"],
    level: "medium" as const,
    requires_manual_approval: true,
    summary: "One cited claim will be added.",
  };

  return {
    base_revision: "wiki_rev_0",
    citation_validation: [validation],
    claim_ids: ["claim_1"],
    claims: [
      {
        citation_ids: ["cite_1"],
        citation_validation: [validation],
        claim_id: "claim_1",
        page_id: "page_1",
        risk_summary: risk,
        security_flags: ["no_active_content"],
        taint_flags: ["untrusted_content"],
        text: "Approval changes need citation evidence.",
      },
    ],
    diff: {
      added_lines: 1,
      affected_paths: ["wiki/page_1.md"],
      format: "unified",
      removed_lines: 0,
      text: "+ Approval changes need citation evidence.",
    },
    patch_id: "patch_1",
    risk_summary: risk,
    security_flags: ["no_active_content"],
    taint_flags: ["untrusted_content"],
  };
}

function approvalReview(): ApprovalReviewDTO {
  const proposal = patchProposal();

  return {
    proposal,
    request: {
      approval_id: "approval_1",
      audit_id: null,
      claim_decisions: [],
      expires_at: "2026-04-28T01:00:00.000Z",
      patch_id: proposal.patch_id,
      policy_decision: "requires_approval",
      proposal,
      rejection_reason: null,
      required_by: "wiki.patch.transaction",
      status: "pending",
      wal_entry_id: null,
    },
  };
}

function approvalResult(status: ApprovalDecisionResultDTO["status"]): ApprovalDecisionResultDTO {
  return {
    approval_id: "approval_1",
    audit_id: "audit_approval_1",
    claim_decisions: [
      {
        claim_id: "claim_1",
        decided_at: "2026-04-28T00:10:00.000Z",
        decided_by: "user_1",
        decision: status === "rejected" ? "reject" : "approve",
        reason: status === "rejected" ? "citation insufficient" : null,
      },
    ],
    committed_revision: status === "committed" ? "wiki_rev_1" : null,
    denied_reason: status === "rejected" ? "citation insufficient" : null,
    patch_id: "patch_1",
    rejection_reason: status === "rejected" ? "citation insufficient" : null,
    status,
    transaction_id: "txn_approval_1",
    wal_entry_id: "wal_approval_1",
  };
}

function searchResults(): readonly SearchResultDTO[] {
  return [
    {
      citation_refs: [
        {
          citation_id: "cite_fresh",
          claim_id: "claim_fresh",
          degraded_reason: null,
          range: {
            end: 8,
            label: "README.md:4-8",
            start: 4,
            unit: "line",
          },
          source_id: "source_readme",
          wiki_page_id: "page_search",
        },
      ],
      index_status: {
        last_good_revision: "wiki_rev_2",
        stale_reason: null,
        state: "fresh",
        updated_at: "2026-04-28T00:20:00.000Z",
      },
      kind: "claim",
      result_id: "result_fresh",
      snippet: "Fresh cited search result.",
      title: "Fresh citation result",
    },
    {
      citation_refs: [
        {
          citation_id: "cite_stale",
          claim_id: "claim_stale",
          degraded_reason: "index_stale",
          range: {
            end: 3,
            label: "notes.md:1-3",
            start: 1,
            unit: "line",
          },
          source_id: "source_notes",
          wiki_page_id: "page_notes",
        },
      ],
      index_status: {
        last_good_revision: "wiki_rev_1",
        stale_reason: "source visibility changed",
        state: "stale",
        updated_at: "2026-04-28T00:10:00.000Z",
      },
      kind: "wiki_page",
      result_id: "result_stale",
      snippet: "Stale cited search result.",
      title: "Stale citation result",
    },
  ] satisfies readonly SearchResultDTO[];
}

describe("domain use case shells", () => {
  it("maps daemon ready events into AppBoot state", async () => {
    const transport = createMockTransportClient({
      events: [event(1, "daemon.ready")],
    });

    await expect(bootApp(transport)).resolves.toEqual({
      stage: "daemon.ready",
    });
  });

  it("replays daemon events through the domain runtime store", async () => {
    const runtime = createDomainRuntime(
      createMockTransportClient({
        events: [
          event(1, "daemon.ready"),
          event(2, "workspace.init.completed"),
          event(3, "import.stage.changed", {
            stage: "selected",
          }),
        ],
      }),
    );

    await runtime.replay(0);

    expect(runtime.store.getSnapshot()).toMatchObject({
      appBoot: {
        stage: "daemon.ready",
      },
      imports: [
        {
          stage: "selected",
        },
      ],
      workspace: {
        stage: "ready",
      },
    });
  });

  it("creates a workspace shell without daemon-side effects", () => {
    expect(
      initWorkspaceShell({
        auditHead: "audit_1",
        currentRevision: "wiki_rev_0",
        rootUri: "file:///tmp/seaki",
        workspaceId: "ws_123",
      }),
    ).toMatchObject({
      state: {
        stage: "ready",
      },
      workspaceId: "ws_123",
    });
  });

  it("keeps selected file contents opaque to the frontend domain", () => {
    expect(
      prepareUserSelectedFile({
        declaredMime: "text/markdown",
        declaredSize: 128,
        displayName: "notes.md",
        opaqueFileRef: "electron://selection/1",
        platform: "electron",
        selectionId: "sel_1",
      }),
    ).toEqual({
      selection: {
        declaredMime: "text/markdown",
        declaredSize: 128,
        displayName: "notes.md",
        opaqueFileRef: "electron://selection/1",
        platform: "electron",
        selectionId: "sel_1",
      },
      state: {
        committed: false,
        stage: "selected",
      },
    });
  });

  it("sends every M0 use case through transport request methods", async () => {
    const transport = createMockTransportClient();
    const client = createDomainClient(transport);

    await client.workspace.init({
      rootUri: "file:///tmp/seaki",
      workspaceId: "ws_1",
    });
    await client.files.prepareUserSelected({
      declaredMime: "text/markdown",
      declaredSize: 10,
      displayName: "notes.md",
      opaqueFileRef: "electron://selection/1",
      platform: "electron",
      selectionId: "sel_1",
    });
    await client.source.ingestSelectedFile({
      selectionId: "sel_1",
      workspaceId: "ws_1",
    });
    await client.approval.reviewPatch({
      patchId: "patch_1",
      workspaceId: "ws_1",
    });
    await client.approval.decide({
      approvalId: "approval_1",
      decision: "approve",
      workspaceId: "ws_1",
    });
    await client.wiki.readPage({
      pageId: "page_1",
      workspaceId: "ws_1",
    });
    await client.search.query({
      query: "citation",
      workspaceId: "ws_1",
    });
    await client.citation.resolve({
      citationId: "cite_1",
      workspaceId: "ws_1",
    });

    expect(transport.getRequests().map((request) => request.method)).toEqual([
      "workspace.init",
      "files.prepareUserSelected",
      "source.ingestSelectedFile",
      "approval.reviewPatch",
      "approval.decide",
      "wiki.readPage",
      "search.query",
      "citation.resolve",
    ]);
  });

  it("returns typed approval review and decision DTOs from domain use cases", async () => {
    const review = approvalReview();
    const result = approvalResult("approved");
    const transport = createMockTransportClient({
      responder: {
        "approval.decide": result,
        "approval.reviewPatch": review,
      },
    });
    const client = createDomainClient(transport);

    await expect(
      client.approval.reviewPatch({
        patchId: "patch_1",
        workspaceId: "ws_1",
      }),
    ).resolves.toMatchObject({
      proposal: {
        diff: {
          format: "unified",
        },
        claims: [
          {
            citation_validation: [
              {
                evidence: [
                  {
                    source_id: "source_1",
                  },
                ],
              },
            ],
          },
        ],
      },
      request: {
        status: "pending",
      },
    });
    await expect(
      client.approval.decide({
        approvalId: "approval_1",
        claimDecisions: [
          {
            claimId: "claim_1",
            decision: "approve",
          },
        ],
        decision: "approve",
        workspaceId: "ws_1",
      }),
    ).resolves.toEqual(result);
  });

  it("returns typed SearchResultDTOs only through the search.query domain use case", async () => {
    const results = searchResults();
    const transport = createMockTransportClient({
      responder(record) {
        expect(record.method).toBe(M0_DOMAIN_USE_CASE_METHODS.SEARCH_QUERY);

        return results;
      },
    });
    const client = createDomainClient(transport);

    const response: readonly SearchResultDTO[] = await client.search.query({
      query: "citation freshness",
      workspaceId: "ws_1",
    });

    expect(response).toEqual(results);
    expect(response.map((result) => result.index_status.state)).toEqual(["fresh", "stale"]);
    expect(
      response.flatMap((result) => result.citation_refs).map((ref) => ref.citation_id),
    ).toEqual(["cite_fresh", "cite_stale"]);
    expect(transport.getRequests()).toEqual([
      {
        input: {
          query: "citation freshness",
          workspaceId: "ws_1",
        },
        method: M0_DOMAIN_USE_CASE_METHODS.SEARCH_QUERY,
      },
    ]);
  });
});
