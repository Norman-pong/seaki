import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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
  const canApply = model.claims.some((claim) => claim.status === "approved");
  const canCommit = model.claims.some((claim) => claim.status === "applying");
  const batchApprovalCount = model.claims.filter(
    (claim) =>
      claim.status === "pending" &&
      claim.citationState === "valid" &&
      claim.riskLevel !== "high" &&
      !claim.securityFlags.includes("manual_review"),
  ).length;

  return (
    <Card size="sm" className="m-3 border-0 bg-transparent shadow-none" aria-label="approval widget">
      <button
        type="button"
        className="w-full flex items-center justify-between px-2 py-2 text-sm hover:bg-muted/60 rounded-md transition-colors"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-controls="approval-claims-list"
      >
        <div className="flex items-center gap-2">
          <span className="font-semibold">审批</span>
          <span className="font-mono text-xs text-muted-foreground">
            {model.patch.patch_id}
          </span>
        </div>
        <Badge
          variant={
            model.status === "rejected" || model.status === "conflict"
              ? "destructive"
              : model.status === "approved" || model.status === "committed"
                ? "secondary"
                : "outline"
          }
          className="text-xs h-5"
        >
          {STATUS_LABEL[model.status]}
        </Badge>
      </button>

      {expanded && (
        <CardContent id="approval-claims-list" className="pt-2 pb-0 px-2 space-y-3">
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              disabled={batchApprovalCount === 0}
              onClick={() => onChange(approvePendingClaims(approval))}
            >
              批量批准 ({batchApprovalCount})
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              disabled={!canApply}
              onClick={() => onChange(markApprovedClaimsApplying(approval))}
            >
              应用
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              disabled={!canCommit}
              onClick={() => onChange(applyApprovedClaims(approval))}
            >
              提交
            </Button>
          </div>

          <div className="space-y-2 max-h-[260px] overflow-y-auto pr-1">
            {model.claims.map((claim) => {
              const draft = model.rejectionDrafts[claim.claimId] ?? "";
              const canApprove = claim.status === "pending" && claim.citationState !== "invalid";
              const canReject = claim.status !== "committed";

              return (
                <Card key={claim.claimId} size="sm" className="approval-claim-item">
                  <CardContent className="py-3 space-y-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-semibold text-sm truncate">
                        {claim.title}
                      </span>
                      <Badge
                        variant={
                          claim.status === "rejected" || claim.status === "conflict"
                            ? "destructive"
                            : claim.status === "approved" || claim.status === "committed"
                              ? "secondary"
                              : "outline"
                        }
                        className="text-[11px] h-5 flex-shrink-0"
                      >
                        {STATUS_LABEL[claim.status]}
                      </Badge>
                    </div>
                    <p className="text-sm text-muted-foreground leading-relaxed">
                      {claim.statement}
                    </p>
                    <div className="flex flex-wrap gap-2">
                      <span className="text-[11px] text-muted-foreground bg-muted px-2 py-0.5 rounded">
                        引用: {claim.citationId} · {CITATION_LABEL[claim.citationState]}
                      </span>
                      <span className="text-[11px] text-muted-foreground bg-muted px-2 py-0.5 rounded">
                        风险: {claim.riskLevel}
                      </span>
                    </div>
                    <div className="space-y-2 pt-1">
                      <Textarea
                        className="min-h-[32px] text-xs py-1.5 px-2 resize-none"
                        placeholder="拒绝原因"
                        rows={1}
                        value={draft}
                        onChange={(event) => {
                          onChange(
                            updateRejectionDraft(
                              approval,
                              claim.claimId,
                              event.currentTarget.value
                            )
                          );
                        }}
                        disabled={!canReject}
                      />
                      <div className="flex gap-2">
                        <Button
                          variant="secondary"
                          size="sm"
                          className="h-7 text-xs"
                          disabled={!canApprove}
                          onClick={() =>
                            onChange(approveClaim(approval, claim.claimId))
                          }
                        >
                          批准
                        </Button>
                        <Button
                          variant="destructive"
                          size="sm"
                          className="h-7 text-xs"
                          disabled={!canReject || draft.trim().length === 0}
                          onClick={() =>
                            onChange(rejectClaim(approval, claim.claimId))
                          }
                        >
                          拒绝
                        </Button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </CardContent>
      )}
    </Card>
  );
}
