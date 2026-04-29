import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
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
import { createMvpScreenModel } from "./mvpScreenModel";
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

const IMPORT_ACTION_LABEL: Record<ElectronAppModel["importQueue"][number]["action"], string> = {
  authorize: "重新授权",
  inspect: "查看",
  none: "无操作",
  rebuild_index: "重建索引",
  retry_parse: "重试解析",
};

const PREVIEW_STATUS_LABEL: Record<ElectronAppModel["citationPreview"]["status"], string> = {
  degraded: "降级",
  no_access: "无权限",
  open_source_range: "可打开 source range",
  open_wiki_anchor: "可打开 wiki anchor",
  resolving: "解析中",
};

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

function citationBadgeVariant(state: ApprovalClaimModel["citationState"]): BadgeVariant {
  if (state === "invalid") {
    return "destructive";
  }

  return state === "valid" ? "secondary" : "outline";
}

const initialModel: ElectronAppModel = {
  approval: createApprovalDiffModel(),
  ...createMvpScreenModel(),
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

      <section className="screenGrid" aria-label="electron mvp screens">
        <article className="screenPanel">
          <div className="paneHeader compact">
            <div>
              <p className="label">DaemonStatus</p>
              <h2>{model.daemonStatus.status}</h2>
            </div>
            <Badge variant="outline">{model.daemonStatus.auditMode}</Badge>
          </div>
          <p className="screenDetail">{model.daemonStatus.detail}</p>
          <div className="miniActions">
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!model.daemonStatus.canReconnect}
            >
              重连
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!model.daemonStatus.canOpenLogs}
            >
              日志
            </Button>
          </div>
        </article>

        <article className="screenPanel">
          <div className="paneHeader compact">
            <div>
              <p className="label">WorkspaceShell</p>
              <h2>{model.workspaceShell.workspaceId}</h2>
            </div>
            <Badge variant="outline">{model.workspaceShell.indexStatus.state}</Badge>
          </div>
          <dl className="compactFacts">
            <div>
              <dt>Revision</dt>
              <dd>{model.workspaceShell.currentRevision}</dd>
            </div>
            <div>
              <dt>Audit</dt>
              <dd>{model.workspaceShell.auditHead}</dd>
            </div>
            <div>
              <dt>Recover</dt>
              <dd>{model.workspaceShell.degradedReasons.join(", ") || "none"}</dd>
            </div>
          </dl>
          <div className="miniActions">
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!model.workspaceShell.canInitWorkspace}
            >
              初始化
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={!model.workspaceShell.canRebuildIndex}
            >
              重建索引
            </Button>
          </div>
        </article>

        <article className="screenPanel wide">
          <div className="paneHeader compact">
            <div>
              <p className="label">ImportQueue</p>
              <h2>{model.importQueue.length} tasks</h2>
            </div>
            <Badge variant="outline">{model.importStage}</Badge>
          </div>
          <div className="queueList">
            {model.importQueue.map((item) => (
              <section key={item.taskId} className="queueItem">
                <div>
                  <strong>{item.displayName}</strong>
                  <p>{item.detail}</p>
                </div>
                <div className="queueMeta">
                  <Badge variant={item.committed ? "secondary" : "outline"}>{item.stage}</Badge>
                  <Button
                    variant="outline"
                    size="sm"
                    type="button"
                    disabled={item.action === "none"}
                  >
                    {IMPORT_ACTION_LABEL[item.action]}
                  </Button>
                </div>
              </section>
            ))}
          </div>
        </article>

        <article className="screenPanel wide">
          <div className="paneHeader compact">
            <div>
              <p className="label">WikiReader</p>
              <h2>{model.wikiReader.title}</h2>
            </div>
            <Badge variant={model.wikiReader.status === "committed" ? "secondary" : "outline"}>
              {model.wikiReader.status}
            </Badge>
          </div>
          <p className="screenDetail">
            {model.wikiReader.committedRevision}
            {model.wikiReader.warning ? ` · ${model.wikiReader.warning}` : ""}
          </p>
          <div className="citationChips">
            {model.wikiReader.citationRefs.map((citation) => (
              <Badge
                key={citation.citation_id}
                variant={citation.degraded_reason ? "outline" : "secondary"}
              >
                {citation.citation_id}
                {citation.degraded_reason ? ` · ${citation.degraded_reason}` : ""}
              </Badge>
            ))}
          </div>
        </article>

        <article className="screenPanel wide">
          <div className="paneHeader compact">
            <div>
              <p className="label">SearchResults</p>
              <h2>{model.searchResults.query}</h2>
            </div>
            <Badge variant={model.searchResults.status === "ready" ? "secondary" : "outline"}>
              {model.searchResults.status}
            </Badge>
          </div>
          <div className="searchList">
            {model.searchResults.results.map((result) => (
              <section key={result.result_id} className="searchItem">
                <div>
                  <strong>{result.title}</strong>
                  <p>{result.snippet ?? "snippet hidden by permission"}</p>
                </div>
                <Badge variant="outline">{result.index_status.state}</Badge>
              </section>
            ))}
          </div>
          <p className="screenDetail">
            filtered_by_permission {model.searchResults.filteredByPermission}
          </p>
        </article>

        <article className="screenPanel wide">
          <div className="paneHeader compact">
            <div>
              <p className="label">Answer</p>
              <h2>{model.answer.answerId}</h2>
            </div>
            <Badge variant={model.answer.status === "composed" ? "secondary" : "outline"}>
              {model.answer.status}
            </Badge>
          </div>
          <p className="screenDetail">{model.answer.text}</p>
          <div className="citationChips">
            {model.answer.citationRefs.map((citation) => (
              <Badge
                key={citation.citation_id}
                variant={citation.degraded_reason ? "outline" : "secondary"}
              >
                {citation.citation_id}
                {citation.degraded_reason ? ` · ${citation.degraded_reason}` : ""}
              </Badge>
            ))}
          </div>
        </article>

        <article className="screenPanel">
          <div className="paneHeader compact">
            <div>
              <p className="label">CitationPreview</p>
              <h2>{model.citationPreview.citation.citation_id}</h2>
            </div>
            <Badge
              variant={
                model.citationPreview.status === "open_source_range" ? "secondary" : "outline"
              }
            >
              {PREVIEW_STATUS_LABEL[model.citationPreview.status]}
            </Badge>
          </div>
          <p className="screenDetail">
            {model.citationPreview.preview
              ? model.citationPreview.preview.summary
              : "source preview hidden"}
          </p>
          <div className="miniActions">
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={model.citationPreview.status !== "open_source_range"}
            >
              打开
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              disabled={model.citationPreview.recoverability !== "request_access"}
            >
              授权
            </Button>
          </div>
        </article>
      </section>

      <section className="approvalToolbar" aria-label="approval actions">
        <div>
          <p className="label">Patch</p>
          <strong>{approval.patch.patch_id}</strong>
          <span>{approval.patch.base_revision}</span>
        </div>
        <div className="actionGroup">
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={() => {
              replaceApproval(approvePendingClaimsViaDomain(approval));
            }}
            disabled={batchApprovalCount === 0}
          >
            批量批准
          </Button>
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={() => {
              replaceApproval(markApprovedClaimsApplyingViaDomain(approval));
            }}
            disabled={!canApply}
          >
            应用
          </Button>
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={() => {
              replaceApproval(applyApprovedClaimsViaDomain(approval));
            }}
            disabled={!canCommit}
          >
            提交
          </Button>
        </div>
      </section>

      <section className="approvalGrid" aria-label="approval diff">
        <article className="pane sourcePane">
          <div className="paneHeader">
            <div>
              <p className="label">Source</p>
              <h2>{approval.source.title}</h2>
            </div>
            <Badge variant="outline">{approval.source.visibility}</Badge>
          </div>
          <p className="origin">{approval.source.origin_display}</p>
          <div className="sourcePreview">
            {approval.claims.map((claim) => (
              <section key={claim.claimId} className="rangeBlock">
                <div className="rangeMeta">
                  <span>{claim.sourceRange}</span>
                  <Badge variant={citationBadgeVariant(claim.citationState)}>
                    {CITATION_LABEL[claim.citationState]}
                  </Badge>
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
            <Badge variant="outline">{approval.approvalRequest.policy_decision}</Badge>
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
              <Badge key={status} variant={approvalBadgeVariant(status as ApprovalResult)}>
                {STATUS_LABEL[status as ApprovalResult]} {count}
              </Badge>
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
                      <Badge variant={approvalBadgeVariant(claim.status)}>
                        {STATUS_LABEL[claim.status]}
                      </Badge>
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
                  <Textarea
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
                  <Button
                    variant="secondary"
                    size="sm"
                    type="button"
                    disabled={!canApprove}
                    onClick={() => {
                      replaceApproval(approveClaimViaDomain(approval, claim.claimId));
                    }}
                  >
                    批准
                  </Button>
                  <Button
                    variant="destructive"
                    size="sm"
                    type="button"
                    disabled={!canReject || draft.trim().length === 0}
                    onClick={() => {
                      replaceApproval(rejectClaimViaDomain(approval, claim.claimId));
                    }}
                  >
                    拒绝
                  </Button>
                </div>
              </article>
            );
          })}
        </div>
      </section>
    </main>
  );
}
