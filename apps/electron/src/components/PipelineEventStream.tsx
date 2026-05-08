import React, { useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import type { PipelineEvent } from "@/models/pipelineModel";

interface PipelineEventStreamProps {
  readonly events: readonly PipelineEvent[];
}

const EVENT_LABEL: Record<PipelineEvent["type"], { label: string; color: string }> = {
  "step.started": { label: "[START]", color: "text-blue-500" },
  frame: { label: "[FRAME]", color: "text-muted-foreground" },
  checkpoint: { label: "[CHKPT]", color: "text-green-500" },
  "step.completed": { label: "[DONE]", color: "text-green-500" },
  "approval.requested": { label: "[APPRV]", color: "text-yellow-500" },
  error: { label: "[ERROR]", color: "text-red-500" },
};

export const PipelineEventStream = React.memo(function PipelineEventStream({
  events,
}: PipelineEventStreamProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [events.length]);

  return (
    <div
      ref={scrollRef}
      className="max-h-48 overflow-y-auto rounded-lg border bg-muted/30 p-3"
      aria-live="polite"
      aria-atomic="false"
      data-testid="pipeline-event-stream"
    >
      {events.length === 0 && (
        <p className="text-xs text-muted-foreground font-mono">等待事件…</p>
      )}
      <div className="flex flex-col gap-1">
        {events.map((evt) => {
          const meta = EVENT_LABEL[evt.type];
          const time = new Date(evt.timestamp).toLocaleTimeString("zh-CN", {
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
          });
          return (
            <div key={evt.seq} className="flex items-start gap-2 font-mono text-xs">
              <span className="text-muted-foreground flex-shrink-0 w-12 text-right">
                {evt.seq}
              </span>
              <span className={cn("flex-shrink-0 w-14", meta.color)}>{meta.label}</span>
              <span className="text-muted-foreground flex-shrink-0 w-16">{time}</span>
              <span className="truncate text-foreground">
                {evt.stepId && <span className="text-muted-foreground mr-1">{evt.stepId}</span>}
                {Object.entries(evt.payload)
                  .map(([k, v]) => `${k}=${JSON.stringify(v)}`)
                  .join(" ")}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
});
