import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  type ApprovalDiffModel,
  type ApprovalClaimModel,
  type ApprovalResult,
  approvePendingClaims,
  markApprovedClaimsApplying,
  applyApprovedClaims,
  approveClaim,
  rejectClaim,
  updateRejectionDraft,
} from "@/appModel";

type BadgeVariant = "default" | "secondary" | "destructive" | "outline";

function approvalBadgeVariant(status: ApprovalResult): BadgeVariant {
  if (status === "rejected" || status === "conflict") {
    return "destructive";
  }
  if (status === "approved" || status === "committed") {
    return "secondary";
  }
  return "outline";
}

const STATUS_LABEL: Record<ApprovalResult, string> = {
  approved: "已批准",
  applying: "应用中",
  committed: "已提交",
  conflict: "冲突",
  expired: "已过期",
  pending: "待审",
  rejected: "已拒绝",
};

const CITATION_LABEL: Record<ApprovalClaimModel["citationState"], string> = {
  degraded: "降级",
  invalid: "无效",
  valid: "有效",
};

interface ApprovalWidgetProps {
  readonly model: ApprovalDiffModel;
  readonly onChange: (model: ApprovalDiffModel) => void;
}

export function ApprovalWidget({ model, onChange }: ApprovalWidgetProps) {
  const [expanded, setExpanded] = useState(true);
  const approval = model;
  const canApply = approval.claims.some((claim) => claim.status === "approved");
  const canCommit = approval.claims.some((claim) => claim.status === "applying");
  const batchApprovalCount = approval.claims.filter(
    (claim) =>
      claim.status === "pending" &&
      claim.citationState === "valid" &&
      claim.riskLevel !== "high" &&
      !claim.securityFlags.includes("manual_review"),
  ).length;

  return (
    <div className="approval-widget" aria-label="approval widget">
      <button
        type="button"
        className="approval-widget-header"
        onClick={() => setExpanded(!expanded)}
      >
        <div>
          <span className="approval-widget-label">审批</span>
          <span className="approval-widget-id">{approval.patch.patch_id}</span>
        </div>
        <Badge variant={approvalBadgeVariant(approval.status)}>
          {STATUS_LABEL[approval.status]}
        </Badge>
      </button>

      {expanded ? (
        <div className="approval-widget-body">
          <div className="approval-widget-actions">
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={batchApprovalCount === 0}
              onClick={() => onChange(approvePendingClaims(approval))}
            >
              批量批准 ({batchApprovalCount})
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!canApply}
              onClick={() => onChange(markApprovedClaimsApplying(approval))}
            >
              应用
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!canCommit}
              onClick={() => onChange(applyApprovedClaims(approval))}
            >
              提交
            </Button>
          </div>

          <div className="approval-claims-list">
            {approval.claims.map((claim) => {
              const draft = approval.rejectionDrafts[claim.claimId] ?? "";
              const canApprove = claim.status === "pending" && claim.citationState !== "invalid";
              const canReject = claim.status !== "committed";

              return (
                <div key={claim.claimId} className={`approval-claim-item ${claim.status}`}>
                  <div className="approval-claim-main">
                    <div className="approval-claim-title-row">
                      <span className="approval-claim-name">{claim.title}</span>
                      <Badge variant={approvalBadgeVariant(claim.status)}>
                        {STATUS_LABEL[claim.status]}
                      </Badge>
                    </div>
                    <p className="approval-claim-statement">{claim.statement}</p>
                    <div className="approval-claim-meta">
                      <span className="meta-tag">
                        引用: {claim.citationId} · {CITATION_LABEL[claim.citationState]}
                      </span>
                      <span className="meta-tag">风险: {claim.riskLevel}</span>
                    </div>
                  </div>
                  <div className="approval-claim-actions">
                    <Textarea
                      className="approval-reject-input"
                      placeholder="拒绝原因"
                      rows={1}
                      value={draft}
                      onChange={(event) => {
                        onChange(updateRejectionDraft(approval, claim.claimId, event.currentTarget.value));
                      }}
                      disabled={!canReject}
                    />
                    <div className="approval-action-btns">
                      <Button
                        variant="secondary"
                        size="sm"
                        type="button"
                        disabled={!canApprove}
                        onClick={() => onChange(approveClaim(approval, claim.claimId))}
                      >
                        批准
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        type="button"
                        disabled={!canReject || draft.trim().length === 0}
                        onClick={() => onChange(rejectClaim(approval, claim.claimId))}
                      >
                        拒绝
                      </Button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ) : null}
    </div>
  );
}
