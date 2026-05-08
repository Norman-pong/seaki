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

export function createMockPipelineRun(): PipelineRun {
  const steps: PipelineStep[] = [
    {
      stepId: "step_1",
      name: "导入 Markdown 文件",
      cmd: "source.ingest",
      args: ["--format=markdown", "--workspace=/data/ws"],
      inputSchema: "SourceIngestInput",
      outputSchema: "SourceIngestOutput",
      requiredCapabilities: ["source.read"],
      estimatedCost: 1200,
      status: "completed",
      output: "成功导入 3 个 Markdown 文件，共 42 个段落",
    },
    {
      stepId: "step_2",
      name: "生成 wiki patch",
      cmd: "wiki.patch.generate",
      args: ["--strategy=merge", "--validate-citations"],
      inputSchema: "WikiPatchInput",
      outputSchema: "WikiPatchOutput",
      requiredCapabilities: ["wiki.write", "citation.validate"],
      estimatedCost: 3500,
      status: "completed",
      output: "生成 2 条 wiki patch，包含 5 个 citation 引用",
    },
    {
      stepId: "step_3",
      name: "重建索引",
      cmd: "search.index.rebuild",
      args: ["--incremental", "--commit-threshold=1000"],
      inputSchema: "IndexRebuildInput",
      outputSchema: "IndexRebuildOutput",
      requiredCapabilities: ["index.write", "search.admin"],
      estimatedCost: 800,
      status: "running",
    },
    {
      stepId: "step_4",
      name: "请求审批",
      cmd: "approval.request",
      args: ["--policy=requires_approval", "--scope=workspace"],
      inputSchema: "ApprovalRequestInput",
      outputSchema: "ApprovalRequestOutput",
      requiredCapabilities: ["approval.request", "policy.read"],
      estimatedCost: 200,
      status: "pending",
    },
  ];

  const connections = [
    { from: "step_1", to: "step_2" },
    { from: "step_2", to: "step_3" },
    { from: "step_3", to: "step_4" },
  ];

  const allCapabilities = Array.from(
    new Set(steps.flatMap((s) => s.requiredCapabilities)),
  );

  const totalEstimatedCost = steps.reduce((sum, s) => sum + s.estimatedCost, 0);

  return {
    runId: "run_mock_001",
    graph: {
      pipelineId: "pipeline_mock_001",
      name: "Wiki 导入与索引 Pipeline",
      steps,
      connections,
      totalEstimatedCost,
      requiredCapabilities: allCapabilities,
    },
    status: "running",
    events: [
      {
        seq: 1,
        type: "step.started",
        stepId: "step_1",
        payload: { cmd: "source.ingest", args: ["--format=markdown"] },
        timestamp: "2026-05-08T10:00:00.000+08:00",
      },
      {
        seq: 2,
        type: "frame",
        stepId: "step_1",
        payload: { progress: 0.3, files_scanned: 1 },
        timestamp: "2026-05-08T10:00:02.000+08:00",
      },
      {
        seq: 3,
        type: "checkpoint",
        stepId: "step_1",
        payload: { checkpoint_id: "chk_1", status: "ok" },
        timestamp: "2026-05-08T10:00:05.000+08:00",
      },
      {
        seq: 4,
        type: "step.completed",
        stepId: "step_1",
        payload: { output_size: 42 },
        timestamp: "2026-05-08T10:00:06.000+08:00",
      },
      {
        seq: 5,
        type: "step.started",
        stepId: "step_2",
        payload: { cmd: "wiki.patch.generate" },
        timestamp: "2026-05-08T10:00:07.000+08:00",
      },
      {
        seq: 6,
        type: "step.completed",
        stepId: "step_2",
        payload: { patches: 2 },
        timestamp: "2026-05-08T10:00:12.000+08:00",
      },
      {
        seq: 7,
        type: "step.started",
        stepId: "step_3",
        payload: { cmd: "search.index.rebuild" },
        timestamp: "2026-05-08T10:00:13.000+08:00",
      },
      {
        seq: 8,
        type: "frame",
        stepId: "step_3",
        payload: { progress: 0.6, docs_indexed: 25 },
        timestamp: "2026-05-08T10:00:15.000+08:00",
      },
    ],
    startedAt: "2026-05-08T10:00:00.000+08:00",
  };
}
