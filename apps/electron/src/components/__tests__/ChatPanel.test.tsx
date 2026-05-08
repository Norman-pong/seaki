import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { ChatPanel } from "../ChatPanel";
import type { ChatSession } from "@/models/chatModel";

const mockSession: ChatSession = {
  id: "session_test",
  title: "测试会话",
  timestamp: "2026-05-08T10:00:00+08:00",
  messages: [
    {
      id: "msg_1",
      role: "assistant",
      content: "Pipeline dry-run 已完成",
      timestamp: "2026-05-08T10:01:00+08:00",
      cards: [
        {
          type: "approval",
          title: "Patch: test_patch",
          content: "需要人工确认",
          status: "requires_approval",
        },
      ],
    },
  ],
};

const mockSessionWithPipeline: ChatSession = {
  id: "session_pipeline",
  title: "Pipeline 测试",
  timestamp: "2026-05-08T10:00:00+08:00",
  messages: [
    {
      id: "msg_p1",
      role: "assistant",
      content: "已启动 Pipeline",
      timestamp: "2026-05-08T10:01:00+08:00",
      cards: [
        {
          type: "pipeline",
          title: "Wiki 导入与索引 Pipeline",
          content: "包含 4 个步骤",
          status: "running",
        },
      ],
    },
  ],
};

describe("ChatPanel", () => {
  it("selects_skill_on_click", () => {
    render(<ChatPanel session={mockSession} />);

    const skillBtn = screen.getByTestId("skill-btn-wiki-search");
    expect(skillBtn).toBeInTheDocument();

    fireEvent.click(skillBtn);
    expect(screen.getByText("@wiki-search")).toBeInTheDocument();

    fireEvent.click(skillBtn);
    expect(screen.queryByText("@wiki-search")).not.toBeInTheDocument();
  });

  it("sends_message_with_skill_prefix", () => {
    const onSendMessage = vi.fn<() => void>();
    render(
      <ChatPanel
        session={mockSession}
        onSendMessage={onSendMessage}
      />,
    );

    fireEvent.click(screen.getByTestId("skill-btn-pipeline-run"));

    const input = screen.getByTestId("chat-input");
    fireEvent.change(input, { target: { value: "执行导入" } });
    fireEvent.click(screen.getByTestId("chat-send-btn"));

    expect(onSendMessage).toHaveBeenCalledWith(
      "session_test",
      "执行导入",
      "pipeline-run",
    );
  });

  it("sends_message_without_skill", () => {
    const onSendMessage = vi.fn<() => void>();
    render(
      <ChatPanel
        session={mockSession}
        onSendMessage={onSendMessage}
      />,
    );

    const input = screen.getByTestId("chat-input");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.click(screen.getByTestId("chat-send-btn"));

    expect(onSendMessage).toHaveBeenCalledWith(
      "session_test",
      "hello",
      undefined,
    );
  });

  it("shows_pipeline_card", () => {
    render(<ChatPanel session={mockSessionWithPipeline} />);

    expect(screen.getByText("Wiki 导入与索引 Pipeline")).toBeInTheDocument();
    expect(screen.getByText("包含 4 个步骤")).toBeInTheDocument();
  });

  it("shows_approval_buttons_for_approval_card", () => {
    render(<ChatPanel session={mockSession} />);

    expect(screen.getByTestId("approval-approve-btn")).toBeInTheDocument();
    expect(screen.getByTestId("approval-reject-btn")).toBeInTheDocument();
    expect(screen.getByTestId("approval-view-diff-btn")).toBeInTheDocument();
  });

  it("calls_onOpenReviewTab_when_view_diff_clicked", () => {
    const onOpenReviewTab = vi.fn<() => void>();
    render(
      <ChatPanel
        session={mockSession}
        onOpenReviewTab={onOpenReviewTab}
      />,
    );

    fireEvent.click(screen.getByTestId("approval-view-diff-btn"));
    expect(onOpenReviewTab).toHaveBeenCalled();
  });

  it("shows_pipeline_placeholder_when_pipeline_skill_selected", () => {
    render(<ChatPanel session={mockSession} />);

    fireEvent.click(screen.getByTestId("skill-btn-pipeline-run"));
    const input = screen.getByTestId("chat-input");
    expect(input).toHaveAttribute("placeholder", "输入 pipeline 意图...");
  });
});
