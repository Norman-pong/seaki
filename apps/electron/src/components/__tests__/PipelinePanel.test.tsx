import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { PipelinePanel } from "../PipelinePanel";
import type { PipelineRun } from "@/models/pipelineModel";

function createMockPipelineRun(overrides?: Partial<PipelineRun>): PipelineRun {
  const steps = [
    {
      stepId: "step_1",
      name: "导入 Markdown 文件",
      cmd: "source.ingest",
      args: ["--format=markdown", "--workspace=/data/ws"],
      inputSchema: "SourceIngestInput",
      outputSchema: "SourceIngestOutput",
      requiredCapabilities: ["source.read"],
      estimatedCost: 1200,
      status: "completed" as const,
      output: "成功导入 3 个 Markdown 文件",
    },
    {
      stepId: "step_2",
      name: "生成 wiki patch",
      cmd: "wiki.patch.generate",
      args: ["--strategy=merge"],
      inputSchema: "WikiPatchInput",
      outputSchema: "WikiPatchOutput",
      requiredCapabilities: ["wiki.write", "citation.validate"],
      estimatedCost: 3500,
      status: "completed" as const,
      output: "生成 2 条 wiki patch",
    },
    {
      stepId: "step_3",
      name: "重建索引",
      cmd: "search.index.rebuild",
      args: ["--incremental"],
      inputSchema: "IndexRebuildInput",
      outputSchema: "IndexRebuildOutput",
      requiredCapabilities: ["index.write", "search.admin"],
      estimatedCost: 800,
      status: "running" as const,
    },
    {
      stepId: "step_4",
      name: "请求审批",
      cmd: "approval.request",
      args: ["--policy=requires_approval"],
      inputSchema: "ApprovalRequestInput",
      outputSchema: "ApprovalRequestOutput",
      requiredCapabilities: ["approval.request", "policy.read"],
      estimatedCost: 200,
      status: "pending" as const,
    },
  ];

  const allCapabilities = Array.from(new Set(steps.flatMap((s) => s.requiredCapabilities)));
  const totalEstimatedCost = steps.reduce((sum, s) => sum + s.estimatedCost, 0);

  const base: PipelineRun = {
    runId: "run_mock_001",
    graph: {
      pipelineId: "pipeline_mock_001",
      name: "Wiki 导入与索引 Pipeline",
      steps,
      connections: [
        { from: "step_1", to: "step_2" },
        { from: "step_2", to: "step_3" },
        { from: "step_3", to: "step_4" },
      ],
      totalEstimatedCost,
      requiredCapabilities: allCapabilities,
    },
    status: "running",
    events: [
      {
        seq: 1,
        type: "step.started",
        stepId: "step_1",
        payload: { cmd: "source.ingest" },
        timestamp: "2026-05-08T10:00:00.000+08:00",
      },
      {
        seq: 2,
        type: "frame",
        stepId: "step_1",
        payload: { progress: 0.3 },
        timestamp: "2026-05-08T10:00:02.000+08:00",
      },
    ],
    startedAt: "2026-05-08T10:00:00.000+08:00",
  };

  return { ...base, ...overrides };
}

describe("PipelinePanel", () => {
  it("renders_pipeline_panel", () => {
    const pipeline = createMockPipelineRun();
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-panel")).toBeInTheDocument();
    expect(screen.getByText("Wiki 导入与索引 Pipeline")).toBeInTheDocument();
  });

  it("renders_step_cards", () => {
    const pipeline = createMockPipelineRun();
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-step-step_1")).toBeInTheDocument();
    expect(screen.getByTestId("pipeline-step-step_2")).toBeInTheDocument();
    expect(screen.getByTestId("pipeline-step-step_3")).toBeInTheDocument();
    expect(screen.getByTestId("pipeline-step-step_4")).toBeInTheDocument();
  });

  it("shows_running_status_badge", () => {
    const pipeline = createMockPipelineRun({ status: "running" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("运行中")).toBeInTheDocument();
  });

  it("shows_event_stream_when_running", () => {
    const pipeline = createMockPipelineRun({ status: "running" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-event-stream")).toBeInTheDocument();
  });

  it("shows_dry_run_preview_when_dry_running", () => {
    const pipeline = createMockPipelineRun({ status: "dry_running" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-dry-run-preview")).toBeInTheDocument();
    expect(screen.getByText("Dry-run 预览")).toBeInTheDocument();
  });

  it("shows_approval_section_when_awaiting_approval", () => {
    const pipeline = createMockPipelineRun({ status: "awaiting_approval" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-approval")).toBeInTheDocument();
    expect(screen.getByTestId("pipeline-approve-btn")).toBeInTheDocument();
  });

  it("triggers_dry_run_on_click", () => {
    const onTriggerDryRun = vi.fn<() => void>();
    const pipeline = createMockPipelineRun({ status: "idle" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={onTriggerDryRun}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    fireEvent.click(screen.getByTestId("pipeline-dry-run-btn"));
    expect(onTriggerDryRun).toHaveBeenCalledTimes(1);
  });

  it("triggers_run_on_click", () => {
    const onTriggerRun = vi.fn<() => void>();
    const pipeline = createMockPipelineRun({ status: "idle" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={onTriggerRun}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    fireEvent.click(screen.getByTestId("pipeline-run-btn"));
    expect(onTriggerRun).toHaveBeenCalledTimes(1);
  });

  it("triggers_cancel_on_click", () => {
    const onCancel = vi.fn<() => void>();
    const pipeline = createMockPipelineRun({ status: "running" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={onCancel}
      />,
    );

    fireEvent.click(screen.getByTestId("pipeline-cancel-btn"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("triggers_approve_on_click", () => {
    const onApprove = vi.fn<() => void>();
    const pipeline = createMockPipelineRun({ status: "awaiting_approval" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={onApprove}
        onCancel={vi.fn<() => void>()}
      />,
    );

    fireEvent.click(screen.getByTestId("pipeline-approve-btn"));
    expect(onApprove).toHaveBeenCalledTimes(1);
  });

  it("disables_buttons_when_running", () => {
    const pipeline = createMockPipelineRun({ status: "running" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-dry-run-btn")).toBeDisabled();
    expect(screen.getByTestId("pipeline-run-btn")).toBeDisabled();
    expect(screen.getByTestId("pipeline-cancel-btn")).toBeEnabled();
  });

  it("disables_cancel_when_idle", () => {
    const pipeline = createMockPipelineRun({ status: "idle" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("pipeline-cancel-btn")).toBeDisabled();
  });

  it("shows_idle_status_badge", () => {
    const pipeline = createMockPipelineRun({ status: "idle" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("空闲")).toBeInTheDocument();
  });

  it("shows_failed_status_badge", () => {
    const pipeline = createMockPipelineRun({ status: "failed" });
    render(
      <PipelinePanel
        pipeline={pipeline}
        onTriggerDryRun={vi.fn<() => void>()}
        onTriggerRun={vi.fn<() => void>()}
        onApprove={vi.fn<() => void>()}
        onCancel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("失败")).toBeInTheDocument();
  });
});
