import { describe, expect, it } from "vitest";

import { SCHEMA_HASH, SCHEMA_VERSION } from "@seaki/dto";
import type {
  ApprovalDecisionResultDTO,
  ApprovalRequestDTO,
  WikiPatchProposalDTO,
} from "@seaki/dto";

import {
  advanceImportState,
  createInitialRuntimeState,
  createRuntimeStore,
  createSelectedImport,
  finishWorkspaceInit,
  failWorkspaceInit,
  reduceAppBoot,
  reduceRuntimeState,
  startWorkspaceInit,
} from "./index";
import type { FrontendRuntimeEvent, ImportStage } from "./index";

function event(
  seq: number,
  eventType: string,
  payload: Record<string, unknown> = {},
): FrontendRuntimeEvent {
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
    task_id: eventType.startsWith("import.")
      ? "task_import_1"
      : eventType.startsWith("approval.")
        ? "task_approval_1"
        : "task_workspace_1",
    transaction_id: `tx_${seq}`,
    type: eventType,
    workspace_id: "ws_1",
  };
}

function approvalProposal(): WikiPatchProposalDTO {
  const range = {
    end: 12,
    label: "source.md:1",
    start: 1,
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
        excerpt: "Approval evidence comes from cited source ranges.",
        range,
        source_id: "source_1",
        source_title: "source.md",
        visibility: "visible" as const,
      },
    ],
    reason: null,
    security_flags: ["no_active_content"],
    state: "valid" as const,
    taint_flags: ["untrusted_content"],
  };
  const risk = {
    factors: ["writes wiki page"],
    level: "medium" as const,
    requires_manual_approval: true,
    summary: "Patch changes one claim.",
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
        text: "Draft claim",
      },
    ],
    diff: {
      added_lines: 1,
      affected_paths: ["wiki/page_1.md"],
      format: "unified",
      removed_lines: 0,
      text: "+ Draft claim",
    },
    patch_id: "patch_1",
    risk_summary: risk,
    security_flags: ["no_active_content"],
    taint_flags: ["untrusted_content"],
  };
}

function approvalRequest(status: ApprovalRequestDTO["status"] = "pending"): ApprovalRequestDTO {
  const proposal = approvalProposal();

  return {
    approval_id: "approval_1",
    audit_id: null,
    claim_decisions: [],
    expires_at: "2026-04-28T01:00:00.000Z",
    patch_id: proposal.patch_id,
    policy_decision: "requires_approval",
    proposal,
    rejection_reason: null,
    required_by: "wiki.patch.transaction",
    status,
    wal_entry_id: null,
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
        reason: status === "rejected" ? "not enough evidence" : null,
      },
    ],
    committed_revision: status === "committed" ? "wiki_rev_1" : null,
    denied_reason: status === "rejected" ? "not enough evidence" : null,
    patch_id: "patch_1",
    rejection_reason: status === "rejected" ? "not enough evidence" : null,
    status,
    transaction_id: "txn_approval_1",
    wal_entry_id: "wal_approval_1",
  };
}

function dispatchImportStages(stages: readonly ImportStage[]) {
  const store = createRuntimeStore();

  stages.forEach((stage, index) => {
    store.dispatch(
      event(index + 1, "import.stage.changed", {
        stage,
      }),
    );
  });

  return store.getSnapshot();
}

describe("frontend runtime state", () => {
  it("starts with connecting app boot and an uninitialized workspace", () => {
    expect(createInitialRuntimeState()).toMatchObject({
      appBoot: {
        stage: "daemon.connecting",
      },
      workspace: {
        stage: "uninitialized",
      },
    });
  });

  it("captures daemon readiness without inventing workspace facts", () => {
    expect(reduceAppBoot("daemon.ready")).toEqual({
      stage: "daemon.ready",
    });
  });

  it("moves workspace init into ready or degraded states", () => {
    expect(startWorkspaceInit({ stage: "uninitialized" })).toEqual({
      stage: "initializing",
    });
    expect(finishWorkspaceInit()).toEqual({
      stage: "ready",
    });
    expect(finishWorkspaceInit({ reason: "index_stale" })).toEqual({
      reason: "index_stale",
      stage: "degraded",
    });
    expect(failWorkspaceInit()).toEqual({
      stage: "error",
    });
  });

  it("keeps import transitions inside the documented M0 shell", () => {
    const selected = createSelectedImport();
    const grantRequested = advanceImportState(selected, "grant_requested");
    const granted = advanceImportState(grantRequested, "granted");

    expect(granted).toEqual({
      committed: false,
      stage: "granted",
    });
    expect(advanceImportState(selected, "indexed")).toBe(selected);
  });

  it("applies mock daemon envelopes to AppBoot, Workspace, and Import state machines", () => {
    const store = createRuntimeStore();

    store.dispatch(event(1, "daemon.ready"));
    store.dispatch(event(2, "workspace.init.started"));
    store.dispatch(
      event(3, "workspace.init.completed", {
        workspace: {
          audit_head: "audit_1",
          current_revision: "wiki_rev_0",
          index_status: {
            last_good_revision: null,
            stale_reason: null,
            state: "stale",
            updated_at: null,
          },
          root_uri: "file:///tmp/seaki",
          state: "ready",
          workspace_id: "ws_1",
        },
      }),
    );
    store.dispatch(
      event(4, "import.stage.changed", {
        stage: "selected",
      }),
    );
    store.dispatch(
      event(5, "import.stage.changed", {
        stage: "grant_requested",
      }),
    );

    expect(store.getSnapshot()).toMatchObject({
      appBoot: {
        stage: "daemon.ready",
      },
      imports: [
        {
          committed: false,
          stage: "grant_requested",
          taskId: "task_import_1",
        },
      ],
      lastSeq: 5,
      workspace: {
        stage: "ready",
      },
    });
  });

  it("does not treat draft or temporary import events as committed state", () => {
    const beforeCommit = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "import.stage.changed", {
        stage: "selected",
      }),
    );
    const afterDraftCommit = reduceRuntimeState(
      {
        ...beforeCommit,
        imports: [
          {
            committed: false,
            stage: "approval_pending",
            taskId: "task_import_1",
            workspaceId: "ws_1",
          },
        ],
      },
      event(2, "import.stage.changed", {
        draft: true,
        stage: "committed",
      }),
    );

    expect(afterDraftCommit.imports[0]).toMatchObject({
      committed: false,
      stage: "approval_pending",
    });
  });

  it("maps workspace failure envelopes into the Workspace error state", () => {
    const snapshot = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "workspace.init.failed", {
        error: "daemon unavailable",
      }),
    );

    expect(snapshot).toMatchObject({
      tasks: {
        task_workspace_1: {
          kind: "workspace.init",
          lastEventSeq: 1,
          stage: "error",
        },
      },
      workspace: {
        stage: "error",
      },
    });
  });

  it("recovers a degraded workspace when replay receives a fresh ready workspace", () => {
    const degraded = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "workspace.init.completed", {
        reason: "index_stale",
        workspace: {
          audit_head: "audit_1",
          current_revision: "wiki_rev_0",
          index_status: {
            last_good_revision: null,
            stale_reason: "source visibility changed",
            state: "stale",
            updated_at: null,
          },
          root_uri: "file:///tmp/seaki",
          state: "degraded",
          workspace_id: "ws_1",
        },
      }),
    );
    const recovered = reduceRuntimeState(
      degraded,
      event(2, "workspace.ready", {
        workspace: {
          audit_head: "audit_2",
          current_revision: "wiki_rev_1",
          index_status: {
            last_good_revision: "wiki_rev_1",
            stale_reason: null,
            state: "fresh",
            updated_at: "2026-04-28T00:10:00.000Z",
          },
          root_uri: "file:///tmp/seaki",
          state: "ready",
          workspace_id: "ws_1",
        },
      }),
    );

    expect(degraded.workspace).toMatchObject({
      reason: "index_stale",
      stage: "degraded",
    });
    expect(recovered.workspace).toMatchObject({
      indexStatus: {
        state: "fresh",
      },
      stage: "ready",
    });
    expect(recovered.workspace.reason).toBeUndefined();
  });

  it("replays the complete import happy path from selected to indexed", () => {
    const stages: readonly ImportStage[] = [
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "parsed",
      "patch_proposed",
      "approval_pending",
      "committed",
      "indexed",
    ];

    const snapshot = dispatchImportStages(stages);

    expect(snapshot.imports).toHaveLength(1);
    expect(snapshot.imports[0]).toMatchObject({
      committed: true,
      stage: "indexed",
      taskId: "task_import_1",
      workspaceId: "ws_1",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      kind: "source.ingest",
      lastEventSeq: stages.length,
      stage: "indexed",
    });
  });

  it("moves grant_requested imports to capability_denied when capability is refused", () => {
    const snapshot = dispatchImportStages(["selected", "grant_requested", "capability_denied"]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: false,
      stage: "capability_denied",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "capability_denied",
    });
  });

  it("allows partial parse results to downgrade to failed", () => {
    const snapshot = dispatchImportStages([
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "partial",
      "failed",
    ]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: false,
      stage: "failed",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "failed",
    });
  });

  it("keeps approval_pending imports uncommitted when approval is denied", () => {
    const snapshot = dispatchImportStages([
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "parsed",
      "patch_proposed",
      "approval_pending",
      "denied",
    ]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: false,
      stage: "denied",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "denied",
    });
  });

  it("marks committed imports as index_stale without clearing committed state", () => {
    const snapshot = dispatchImportStages([
      "selected",
      "grant_requested",
      "granted",
      "raw_committed",
      "parse_running",
      "parsed",
      "patch_proposed",
      "approval_pending",
      "committed",
      "index_stale",
    ]);

    expect(snapshot.imports[0]).toMatchObject({
      committed: true,
      stage: "index_stale",
    });
    expect(snapshot.tasks.task_import_1).toMatchObject({
      stage: "index_stale",
    });
  });

  it("consumes approval review, decision, applying, and committed events", () => {
    const store = createRuntimeStore();

    store.dispatch(
      event(1, "approval.reviewed", {
        request: approvalRequest(),
      }),
    );
    store.dispatch(
      event(2, "approval.decided", {
        result: approvalResult("approved"),
      }),
    );
    store.dispatch(
      event(3, "approval.applying", {
        approval_id: "approval_1",
        patch_id: "patch_1",
      }),
    );
    store.dispatch(
      event(4, "approval.committed", {
        result: approvalResult("committed"),
      }),
    );

    expect(store.getSnapshot()).toMatchObject({
      approvals: [
        {
          approvalId: "approval_1",
          claimDecisions: [
            {
              claim_id: "claim_1",
              decision: "approve",
            },
          ],
          committed: true,
          committedRevision: "wiki_rev_1",
          patchId: "patch_1",
          status: "committed",
          taskId: "task_approval_1",
          workspaceId: "ws_1",
        },
      ],
      tasks: {
        task_approval_1: {
          kind: "approval",
          lastEventSeq: 4,
          stage: "committed",
        },
      },
    });
  });

  it("tracks rejected, conflict, and expired approval terminal states", () => {
    const rejected = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "approval.rejected", {
        approval_id: "approval_rejected",
        patch_id: "patch_rejected",
      }),
    );
    const conflict = reduceRuntimeState(
      rejected,
      event(2, "approval.conflict", {
        approval_id: "approval_conflict",
        patch_id: "patch_conflict",
      }),
    );
    const expired = reduceRuntimeState(
      conflict,
      event(3, "approval.expired", {
        approval_id: "approval_expired",
        patch_id: "patch_expired",
      }),
    );

    expect(expired.approvals.map((approval) => approval.status)).toEqual([
      "rejected",
      "conflict",
      "expired",
    ]);
  });

  it("maps core approval decision vocabulary into approval state", () => {
    const approved = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "approval.decided", {
        approval_id: "approval_approved",
        decision: "approved",
        patch_id: "patch_approved",
      }),
    );
    const denied = reduceRuntimeState(
      approved,
      event(2, "approval.decided", {
        approval_id: "approval_denied",
        decision: "denied",
        patch_id: "patch_denied",
      }),
    );

    expect(denied.approvals.map((approval) => approval.status)).toEqual(["approved", "rejected"]);
  });

  it("does not promote draft approval commits into authoritative committed state", () => {
    const reviewed = reduceRuntimeState(
      createInitialRuntimeState(),
      event(1, "approval.reviewed", {
        request: approvalRequest("pending"),
      }),
    );
    const approved = reduceRuntimeState(
      reviewed,
      event(2, "approval.decided", {
        result: approvalResult("approved"),
      }),
    );
    const draftCommitted = reduceRuntimeState(
      approved,
      event(3, "approval.committed", {
        draft: true,
        result: approvalResult("committed"),
      }),
    );

    expect(draftCommitted.approvals[0]).toMatchObject({
      committed: false,
      status: "approved",
    });
    expect(draftCommitted.approvals[0]?.committedRevision).toBeUndefined();
  });

  it("does not retain committed result from temporary approval commit events", () => {
    const approved = reduceRuntimeState(
      reduceRuntimeState(
        createInitialRuntimeState(),
        event(1, "approval.reviewed", {
          request: approvalRequest("pending"),
        }),
      ),
      event(2, "approval.decided", {
        result: approvalResult("approved"),
      }),
    );
    const temporaryCommitted = reduceRuntimeState(
      approved,
      event(3, "approval.committed", {
        result: approvalResult("committed"),
        temporary: true,
      }),
    );

    expect(temporaryCommitted.approvals[0]).toMatchObject({
      committed: false,
      status: "approved",
    });
    expect(temporaryCommitted.approvals[0]?.committedRevision).toBeUndefined();
    expect(temporaryCommitted.approvals[0]?.result?.status).toBe("approved");
  });
});
