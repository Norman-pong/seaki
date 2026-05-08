import React from "react";
import { CheckCircle, AlertCircle, Loader2, Circle, SkipForward } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { PipelineStep } from "@/models/pipelineModel";

interface PipelineStepCardProps {
  readonly step: PipelineStep;
  readonly index: number;
  readonly isActive: boolean;
}

const STATUS_ICON: Record<PipelineStep["status"], React.ReactNode> = {
  pending: <Circle size={16} className="text-muted-foreground" />,
  running: <Loader2 size={16} className="animate-spin text-blue-500" />,
  completed: <CheckCircle size={16} className="text-green-500" />,
  failed: <AlertCircle size={16} className="text-red-500" />,
  skipped: <SkipForward size={16} className="text-muted-foreground" />,
};

const STATUS_RING: Record<PipelineStep["status"], string> = {
  pending: "ring-border",
  running: "ring-blue-400",
  completed: "ring-green-400",
  failed: "ring-red-400",
  skipped: "ring-border",
};

export const PipelineStepCard = React.memo(function PipelineStepCard({
  step,
  index,
  isActive,
}: PipelineStepCardProps) {
  const outputPreview = step.output ? step.output.slice(0, 80) + (step.output.length > 80 ? "…" : "") : undefined;

  return (
    <Card
      size="sm"
      className={cn(
        "shadow-none transition-all",
        isActive && "ring-1 ring-blue-400 bg-blue-50/40",
      )}
      data-testid={`pipeline-step-${step.stepId}`}
    >
      <CardContent className="py-3">
        <div className="flex items-start gap-3">
          <div
            className={cn(
              "flex-shrink-0 w-6 h-6 rounded-full flex items-center justify-center text-xs font-semibold ring-1",
              STATUS_RING[step.status],
              step.status === "running" && "bg-blue-50",
              step.status === "completed" && "bg-green-50",
              step.status === "failed" && "bg-red-50",
            )}
            aria-label={`步骤 ${index + 1}`}
          >
            {index + 1}
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center justify-between gap-2">
              <span className="font-medium text-sm truncate">{step.name}</span>
              <span className="flex-shrink-0" aria-label={`状态: ${step.status}`}>
                {STATUS_ICON[step.status]}
              </span>
            </div>
            <code className="block mt-0.5 text-[11px] text-muted-foreground font-mono truncate">
              {step.cmd} {step.args.join(" ")}
            </code>
            <div className="flex flex-wrap items-center gap-1.5 mt-1.5">
              {step.requiredCapabilities.map((cap) => (
                <Badge key={cap} variant="secondary" className="text-[10px] h-4 px-1">
                  {cap}
                </Badge>
              ))}
              <Badge variant="outline" className="text-[10px] h-4 px-1 ml-auto">
                {step.estimatedCost.toLocaleString("zh-CN")} tokens
              </Badge>
            </div>
            {outputPreview && (
              <p className="mt-2 text-xs text-muted-foreground leading-relaxed">
                {outputPreview}
              </p>
            )}
            {step.error && (
              <p className="mt-2 text-xs text-red-500 leading-relaxed">{step.error}</p>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
});
