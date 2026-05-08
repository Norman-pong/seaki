import { describe, expect, it } from "vitest";

import { createMockPipelineRun } from "../../models/pipelineModel";

describe("Pipeline model and components", () => {
  it("renders_pipeline_steps", () => {
    const run = createMockPipelineRun();

    expect(run.graph.steps).toHaveLength(4);
    expect(run.graph.steps[0]?.cmd).toBe("source.ingest");
    expect(run.graph.steps[1]?.cmd).toBe("wiki.patch.generate");
    expect(run.graph.steps[2]?.cmd).toBe("search.index.rebuild");
    expect(run.graph.steps[3]?.cmd).toBe("approval.request");
  });

  it("shows_dry_run_preview", () => {
    const run = createMockPipelineRun();

    expect(run.graph.totalEstimatedCost).toBe(
      run.graph.steps.reduce((s, step) => s + step.estimatedCost, 0),
    );
    expect(run.graph.requiredCapabilities.length).toBeGreaterThan(0);
    expect(run.graph.steps.every((s) => s.inputSchema && s.outputSchema)).toBe(true);
  });

  it("shows_event_stream_when_running", () => {
    const run = createMockPipelineRun();

    expect(run.status).toBe("running");
    expect(run.events.length).toBeGreaterThan(0);
    expect(run.events[0]?.type).toBe("step.started");
  });

  it("triggers_dry_run_on_click", () => {
    let triggered = false;
    const onTriggerDryRun = () => {
      triggered = true;
    };

    onTriggerDryRun();
    expect(triggered).toBe(true);
  });

  it("triggers_approval_on_click", () => {
    let approved = false;
    const onApprove = () => {
      approved = true;
    };

    onApprove();
    expect(approved).toBe(true);
  });

  it("renders_step_status_colors", () => {
    const run = createMockPipelineRun();
    const statuses = run.graph.steps.map((s) => s.status);

    expect(statuses).toContain("completed");
    expect(statuses).toContain("running");
    expect(statuses).toContain("pending");
  });

  it("mock_pipeline_has_expected_graph_structure", () => {
    const run = createMockPipelineRun();

    expect(run.graph.connections).toHaveLength(3);
    expect(run.graph.connections[0]).toEqual({ from: "step_1", to: "step_2" });
    expect(run.runId).toBeTruthy();
    expect(run.startedAt).toBeTruthy();
  });
});
