import React from "react";
import {
  Play,
  RotateCcw,
  CheckCircle,
  AlertTriangle,
  Clock,
  Zap,
  Lock,
  XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import { PipelineStepCard } from "./PipelineStepCard";
import { PipelineEventStream } from "./PipelineEventStream";
import type { PipelineRun, PipelineStatus } from "@/models/pipelineModel";

interface PipelinePanelProps {
  readonly pipeline: PipelineRun;
  readonly onTriggerDryRun: () => void;
  readonly onTriggerRun: () => void;
  readonly onApprove: () => void;
  readonly onCancel: () => void;
}

const STATUS_BADGE: Record<PipelineStatus, { label: string; variant: BadgeVariant }> = {
  idle: { label: "空闲", variant: "secondary" },
  designing: { label: "设计中", variant: "secondary" },
  dry_running: { label: "Dry-run", variant: "outline" },
  awaiting_approval: { label: "等待审批", variant: "outline" },
  running: { label: "运行中", variant: "default" },
  completed: { label: "已完成", variant: "outline" },
  failed: { label: "失败", variant: "destructive" },
  cancelled: { label: "已取消", variant: "secondary" },
};

type BadgeVariant = React.ComponentProps<typeof Badge>["variant"];

export function PipelinePanel({
  pipeline,
  onTriggerDryRun,
  onTriggerRun,
  onApprove,
  onCancel,
}: PipelinePanelProps) {
  const statusMeta = STATUS_BADGE[pipeline.status];
  const isRunning = pipeline.status === "running";
  const isDryRunning = pipeline.status === "dry_running";
  const showDryRunPreview = isDryRunning || pipeline.status === "completed" || pipeline.status === "awaiting_approval";
  const showEventStream = isRunning;
  const showApproval = pipeline.status === "awaiting_approval";

  const activeStepIndex = pipeline.graph.steps.findIndex((s) => s.status === "running");

  const actorCapabilities = ["source.read", "wiki.write", "citation.validate", "index.write"];
  const missingCapabilities = pipeline.graph.requiredCapabilities.filter(
    (cap) => !actorCapabilities.includes(cap),
  );
  const hasMissingCapabilities = missingCapabilities.length > 0;

  return (
    <div
      className="flex flex-col gap-3 border-b bg-background px-5 py-4"
      data-testid="pipeline-panel"
    >
      {/* Toolbar */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <Zap size={15} className="text-muted-foreground flex-shrink-0" />
          <span className="font-medium text-sm truncate">{pipeline.graph.name}</span>
          <Badge variant={statusMeta.variant} className="text-[10px] h-4 px-1.5 flex-shrink-0">
            {statusMeta.label}
          </Badge>
        </div>
        <div className="flex items-center gap-1.5 flex-shrink-0">
          <Button
            size="xs"
            variant="outline"
            onClick={onTriggerDryRun}
            disabled={isRunning || isDryRunning}
            data-testid="pipeline-dry-run-btn"
          >
            <RotateCcw size={12} className="mr-1" />
            Dry-run
          </Button>
          <Button
            size="xs"
            onClick={onTriggerRun}
            disabled={isRunning || isDryRunning}
            data-testid="pipeline-run-btn"
          >
            <Play size={12} className="mr-1" />
            运行
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={onCancel}
            disabled={!isRunning && !isDryRunning}
            data-testid="pipeline-cancel-btn"
          >
            <XCircle size={12} className="mr-1" />
            取消
          </Button>
        </div>
      </div>

      {/* Steps */}
      <div className="flex flex-col gap-2">
        {pipeline.graph.steps.map((step, idx) => (
          <React.Fragment key={step.stepId}>
            <PipelineStepCard
              step={step}
              index={idx}
              isActive={activeStepIndex === idx}
            />
            {idx < pipeline.graph.steps.length - 1 && (
              <div className="flex justify-center">
                <div
                  className={cn(
                    "w-px h-3 border-l",
                    step.status === "completed"
                      ? "border-green-300 border-dashed"
                      : "border-border border-dashed",
                  )}
                  aria-hidden="true"
                />
              </div>
            )}
          </React.Fragment>
        ))}
      </div>

      {/* Dry-run preview */}
      {showDryRunPreview && (
        <div className="rounded-lg border bg-card p-3 flex flex-col gap-2" data-testid="pipeline-dry-run-preview">
          <div className="flex items-center gap-1.5 text-sm font-medium">
            <Clock size={13} className="text-muted-foreground" />
            Dry-run 预览
          </div>
          <div className="flex flex-col gap-1.5 text-xs text-muted-foreground">
            {pipeline.graph.steps.map((step) => (
              <div key={step.stepId} className="flex items-center justify-between gap-2">
                <span className="truncate">
                  {step.name}: {step.inputSchema} → {step.outputSchema}
                </span>
                <span className="flex-shrink-0">{step.estimatedCost.toLocaleString("zh-CN")} tokens</span>
              </div>
            ))}
            <Separator className="my-1" />
            <div className="flex items-center justify-between gap-2 font-medium text-foreground">
              <span>预估成本总计</span>
              <span>{pipeline.graph.totalEstimatedCost.toLocaleString("zh-CN")} tokens</span>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-1.5 mt-1">
            <Lock size={11} className="text-muted-foreground" />
            <span className="text-[11px] text-muted-foreground">所需权限:</span>
            {pipeline.graph.requiredCapabilities.map((cap) => (
              <Badge
                key={cap}
                variant={missingCapabilities.includes(cap) ? "destructive" : "secondary"}
                className="text-[10px] h-4 px-1"
              >
                {cap}
              </Badge>
            ))}
          </div>
          {hasMissingCapabilities && (
            <div className="flex items-center gap-1.5 text-xs text-yellow-600 bg-yellow-50 rounded-md px-2 py-1.5">
              <AlertTriangle size={12} />
              <span>当前 actor 缺少权限: {missingCapabilities.join(", ")}</span>
            </div>
          )}
        </div>
      )}

      {/* Approval */}
      {showApproval && (
        <div className="flex items-center gap-2 rounded-lg border bg-card p-3" data-testid="pipeline-approval">
          <AlertTriangle size={14} className="text-yellow-500 flex-shrink-0" />
          <span className="text-sm flex-1">Pipeline 需要审批后才能继续执行</span>
          <Button size="xs" onClick={onApprove} data-testid="pipeline-approve-btn">
            <CheckCircle size={12} className="mr-1" />
            审批通过
          </Button>
        </div>
      )}

      {/* Event stream */}
      {showEventStream && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1.5 text-sm font-medium">
            <Zap size={13} className="text-muted-foreground" />
            执行事件流
          </div>
          <PipelineEventStream events={pipeline.events} />
        </div>
      )}
    </div>
  );
}
