import { useEffect, useState } from "react";

import {
  applyApprovedClaimsViaDomain,
  approveClaimViaDomain,
  approvePendingClaimsViaDomain,
  createApprovalDiffModel,
  createElectronAppModel,
  markApprovedClaimsApplyingViaDomain,
  rejectClaimViaDomain,
  updateRejectionDraft,
} from "./appModel";
import "./styles.css";
import type {
  ApprovalClaimModel,
  ApprovalDiffModel,
  ApprovalResult,
  ElectronAppModel,
} from "./appModel";

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

const initialModel: ElectronAppModel = {
  approval: createApprovalDiffModel(),
  importStage: "selected",
  workspaceStage: "initializing",
  workspaceTitle: "ws_local_preview",
};

export function App() {
  const [model, setModel] = useState<ElectronAppModel>(initialModel);

  function replaceApproval(nextApproval: Promise<ApprovalDiffModel>) {
    void nextApproval.then((approval) => {
      setModel((current) => ({
        ...current,
        approval,
      }));
    });
  }

  useEffect(() => {
    let active = true;

    void createElectronAppModel().then((nextModel) => {
      if (active) {
        setModel(nextModel);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  const approval = model.approval;
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
    <main className="shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">Workspace / Approval</p>
          <h1>{model.workspaceTitle}</h1>
        </div>
        <dl className="topMetrics" aria-label="workspace status">
          <div>
            <dt>Workspace</dt>
            <dd>{model.workspaceStage}</dd>
          </div>
          <div>
            <dt>Import</dt>
            <dd>{model.importStage}</dd>
          </div>
          <div>
            <dt>Approval</dt>
            <dd>{STATUS_LABEL[approval.status]}</dd>
          </div>
        </dl>
      </header>

      <section className="approvalToolbar" aria-label="approval actions">
        <div>
          <p className="label">Patch</p>
          <strong>{approval.patch.patch_id}</strong>
          <span>{approval.patch.base_revision}</span>
        </div>
        <div className="actionGroup">
          <button
            type="button"
            onClick={() => {
              replaceApproval(approvePendingClaimsViaDomain(approval));
            }}
            disabled={batchApprovalCount === 0}
          >
            批量批准
          </button>
          <button
            type="button"
            onClick={() => {
              replaceApproval(markApprovedClaimsApplyingViaDomain(approval));
            }}
            disabled={!canApply}
          >
            应用
          </button>
          <button
            type="button"
            onClick={() => {
              replaceApproval(applyApprovedClaimsViaDomain(approval));
            }}
            disabled={!canCommit}
          >
            提交
          </button>
        </div>
      </section>

      <section className="approvalGrid" aria-label="approval diff">
        <article className="pane sourcePane">
          <div className="paneHeader">
            <div>
              <p className="label">Source</p>
              <h2>{approval.source.title}</h2>
            </div>
            <span className="badge">{approval.source.visibility}</span>
          </div>
          <p className="origin">{approval.source.origin_display}</p>
          <div className="sourcePreview">
            {approval.claims.map((claim) => (
              <section key={claim.claimId} className="rangeBlock">
                <div className="rangeMeta">
                  <span>{claim.sourceRange}</span>
                  <span className={`citationState ${claim.citationState}`}>
                    {CITATION_LABEL[claim.citationState]}
                  </span>
                </div>
                <p>{claim.sourceExcerpt}</p>
              </section>
            ))}
          </div>
        </article>

        <article className="pane diffPane">
          <div className="paneHeader">
            <div>
              <p className="label">Patch diff</p>
              <h2>{approval.approvalRequest.approval_id}</h2>
            </div>
            <span className="badge">{approval.approvalRequest.policy_decision}</span>
          </div>
          <pre className="diffBlock" aria-label="patch diff">
            {approval.patchLines.map((line) => (
              <code key={line.id} className={`diffLine ${line.kind}`}>
                {line.text}
              </code>
            ))}
          </pre>
        </article>
      </section>

      <section className="claimsPanel" aria-labelledby="claims-title">
        <div className="panelTitle">
          <div>
            <p className="label">Claims</p>
            <h2 id="claims-title">Citation validation / risk / taint</h2>
          </div>
          <div className="statusStrip" aria-label="approval result counts">
            {Object.entries(approval.statusCounts).map(([status, count]) => (
              <span key={status}>
                {STATUS_LABEL[status as ApprovalResult]} {count}
              </span>
            ))}
          </div>
        </div>

        <div className="claimList">
          {approval.claims.map((claim) => {
            const draft = approval.rejectionDrafts[claim.claimId] ?? "";
            const canApprove = claim.status === "pending" && claim.citationState !== "invalid";
            const canReject = claim.status !== "committed";

            return (
              <article key={claim.claimId} className={`claimCard ${claim.status}`}>
                <div className="claimMain">
                  <div>
                    <div className="claimTitle">
                      <h3>{claim.title}</h3>
                      <span className={`statusBadge ${claim.status}`}>
                        {STATUS_LABEL[claim.status]}
                      </span>
                    </div>
                    <p>{claim.statement}</p>
                  </div>
                  <dl className="claimFacts">
                    <div>
                      <dt>Citation</dt>
                      <dd>
                        {claim.citationId} · {CITATION_LABEL[claim.citationState]}
                      </dd>
                    </div>
                    <div>
                      <dt>Risk</dt>
                      <dd>
                        {claim.riskLevel} · {claim.riskSummary}
                      </dd>
                    </div>
                    <div>
                      <dt>Taint</dt>
                      <dd>{claim.taintFlags.join(", ")}</dd>
                    </div>
                    <div>
                      <dt>Security</dt>
                      <dd>{claim.securityFlags.join(", ")}</dd>
                    </div>
                  </dl>
                  {claim.citationReason ? (
                    <p className="reasonLine">{claim.citationReason}</p>
                  ) : null}
                  {claim.rejectionReason ? (
                    <p className="rejectLine">拒绝：{claim.rejectionReason}</p>
                  ) : null}
                </div>

                <div className="rejectBox">
                  <label htmlFor={`reject-${claim.claimId}`}>拒绝原因</label>
                  <textarea
                    id={`reject-${claim.claimId}`}
                    value={draft}
                    onChange={(event) => {
                      const reason = event.currentTarget.value;

                      setModel((current) => ({
                        ...current,
                        approval: updateRejectionDraft(current.approval, claim.claimId, reason),
                      }));
                    }}
                    disabled={!canReject}
                  />
                  <button
                    type="button"
                    className="approveButton"
                    disabled={!canApprove}
                    onClick={() => {
                      replaceApproval(approveClaimViaDomain(approval, claim.claimId));
                    }}
                  >
                    批准
                  </button>
                  <button
                    type="button"
                    className="rejectButton"
                    disabled={!canReject || draft.trim().length === 0}
                    onClick={() => {
                      replaceApproval(rejectClaimViaDomain(approval, claim.claimId));
                    }}
                  >
                    拒绝
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      </section>
    </main>
  );
}
