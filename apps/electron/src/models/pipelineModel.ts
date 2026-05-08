export type PipelineStatus =
  | "idle"
  | "designing"
  | "dry_running"
  | "awaiting_approval"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export interface PipelineStep {
  readonly stepId: string;
  readonly name: string;
  readonly cmd: string;
  readonly args: readonly string[];
  readonly inputSchema: string;
  readonly outputSchema: string;
  readonly requiredCapabilities: readonly string[];
  readonly estimatedCost: number; // tokens
  readonly status: "pending" | "running" | "completed" | "failed" | "skipped";
  readonly output?: string;
  readonly error?: string;
}

export interface PipelineGraph {
  readonly pipelineId: string;
  readonly name: string;
  readonly steps: readonly PipelineStep[];
  readonly connections: readonly { from: string; to: string }[];
  readonly totalEstimatedCost: number;
  readonly requiredCapabilities: readonly string[];
}

export interface PipelineEvent {
  readonly seq: number;
  readonly type: "step.started" | "frame" | "checkpoint" | "step.completed" | "approval.requested" | "error";
  readonly stepId?: string;
  readonly payload: Record<string, unknown>;
  readonly timestamp: string;
}

export interface PipelineRun {
  readonly runId: string;
  readonly graph: PipelineGraph;
  readonly status: PipelineStatus;
  readonly events: readonly PipelineEvent[];
  readonly startedAt?: string;
  readonly completedAt?: string;
}


