import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { ApprovalWidget } from "../ApprovalWidget";
import type { ApprovalDiffModel, ApprovalClaimModel } from "@/appModel";

const mockPatch = {
  patch_id: "patch_test_001",
  base_revision: "rev_0",
  citation_validation: [],
  claim_ids: ["claim_1"],
  claims: [],
  diff: { text: "", added_lines: 0, removed_lines: 0, affected_paths: [], format: "unified" as const },
  risk_summary: { level: "low" as const, summary: "", factors: [], requires_manual_approval: false },
  security_flags: [],
  taint_flags: [],
};

const mockClaims: readonly ApprovalClaimModel[] = [
  {
    claimId: "claim_1",
    title: "Test Claim",
    statement: "This is a test statement.",
    status: "pending",
    citationId: "cit_1",
    citationState: "valid",
    citationReason: null,
    sourceRange: "L1-L5",
    sourceExcerpt: "Excerpt",
    riskLevel: "low",
    riskSummary: "Low risk",
    taintFlags: [],
    securityFlags: [],
    rejectionReason: null,
  },
];

function createMockModel(rejectionDrafts: Record<string, string> = {}): ApprovalDiffModel {
  return {
    approvalRequest: {
      approval_id: "appr_001",
      audit_id: null,
      claim_decisions: [],
      expires_at: "2026-05-08T18:00:00.000+08:00",
      patch_id: mockPatch.patch_id,
      policy_decision: "requires_approval",
      proposal: mockPatch,
      rejection_reason: null,
      required_by: "test",
      status: "pending",
      wal_entry_id: null,
    },
    claims: mockClaims,
    patch: mockPatch,
    patchLines: [],
    rejectionDrafts,
    source: {
      citation_refs: [],
      origin_display: "",
      range: { end: 0, label: "", start: 0, unit: "line" },
      source_id: "src_1",
      summary: "",
      title: "",
      visibility: "visible",
    },
    status: "pending",
    statusCounts: { pending: 1, approved: 0, applying: 0, committed: 0, rejected: 0, conflict: 0, expired: 0 },
  };
}

function ApprovalWidgetWithState({ initialModel }: { readonly initialModel: ApprovalDiffModel }) {
  const [model, setModel] = useState(initialModel);
  return <ApprovalWidget model={model} onChange={setModel} />;
}

describe("ApprovalWidget", () => {
  it("renders_widget_header", () => {
    render(
      <ApprovalWidget
        model={createMockModel()}
        onChange={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("审批")).toBeInTheDocument();
    expect(screen.getByText("patch_test_001")).toBeInTheDocument();
  });

  it("toggles_expansion_on_header_click", () => {
    render(
      <ApprovalWidget
        model={createMockModel()}
        onChange={vi.fn<() => void>()}
      />,
    );

    const header = screen.getByText("审批").closest("button");
    expect(header).toHaveAttribute("aria-expanded", "true");

    if (header) fireEvent.click(header);

    // After collapse, claims list should not be visible
    expect(screen.queryByText("Test Claim")).not.toBeInTheDocument();
  });

  it("renders_claim_details", () => {
    render(
      <ApprovalWidget
        model={createMockModel()}
        onChange={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("Test Claim")).toBeInTheDocument();
    expect(screen.getByText("This is a test statement.")).toBeInTheDocument();
    // Use getAllByText because both widget header and claim have status badges
    expect(screen.getAllByText("待审").length).toBeGreaterThanOrEqual(1);
  });

  it("has_rejection_textarea_with_aria_invalid", () => {
    render(
      <ApprovalWidget
        model={createMockModel()}
        onChange={vi.fn<() => void>()}
      />,
    );

    const textarea = screen.getByLabelText("拒绝原因");
    expect(textarea).toHaveAttribute("aria-invalid", "true");
  });

  it("enables_reject_button_after_entering_reason", () => {
    render(<ApprovalWidgetWithState initialModel={createMockModel()} />);

    const textarea = screen.getByLabelText("拒绝原因");
    fireEvent.change(textarea, { target: { value: "不符合要求" } });

    expect(textarea).toHaveAttribute("aria-invalid", "false");

    const rejectBtn = screen.getByText("拒绝");
    expect(rejectBtn).not.toBeDisabled();
  });

  it("calls_onChange_on_batch_approve_click", () => {
    const onChange = vi.fn<() => void>();
    render(
      <ApprovalWidget
        model={createMockModel()}
        onChange={onChange}
      />,
    );

    const batchBtn = screen.getByText("批量批准 (1)");
    fireEvent.click(batchBtn);

    expect(onChange).toHaveBeenCalled();
  });

  it("calls_onChange_on_single_approve_click", () => {
    const onChange = vi.fn<() => void>();
    render(
      <ApprovalWidget
        model={createMockModel()}
        onChange={onChange}
      />,
    );

    const approveBtn = screen.getByText("批准");
    fireEvent.click(approveBtn);

    expect(onChange).toHaveBeenCalled();
  });
});
