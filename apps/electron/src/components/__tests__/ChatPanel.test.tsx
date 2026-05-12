import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useState } from "react";
import "@testing-library/jest-dom/vitest";

import { ChatPanel } from "../ChatPanel";
import type { ChatSession, ChatMessage } from "@/models/chatModel";

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

const mockEmptySession: ChatSession = {
  id: "session_empty",
  title: "空会话",
  timestamp: "2026-05-08T10:00:00+08:00",
  messages: [],
};

const mockSessionWithMultiline: ChatSession = {
  id: "session_multiline",
  title: "换行测试",
  timestamp: "2026-05-08T10:00:00+08:00",
  messages: [
    {
      id: "msg_ml1",
      role: "assistant",
      content: "第一行\n第二行\n第三行",
      timestamp: "2026-05-08T10:01:00+08:00",
    },
  ],
};

const mockSessionWithCitations: ChatSession = {
  id: "session_citations",
  title: "Citation 测试",
  timestamp: "2026-05-08T10:00:00+08:00",
  messages: [
    {
      id: "msg_cit1",
      role: "assistant",
      content: "以下是带引用的回答",
      timestamp: "2026-05-08T10:01:00+08:00",
      cards: [
        {
          type: "wiki",
          title: "引用测试卡片",
          content: "带引用标注的内容",
          status: "committed",
          citationRefs: [
            { id: "cit_1", label: "source scope", citationId: "cit_decision_context" },
            { id: "cit_2", label: "approval boundary", sourceId: "src_42" },
          ],
        },
      ],
    },
  ],
};

function ChatPanelWithApprovalState({ initialSession }: { readonly initialSession: ChatSession }) {
  const [session, setSession] = useState(initialSession);

  function handleApprovalAction(_sessionId: string, messageId: string, action: "approve" | "reject") {
    setSession((prev) => ({
      ...prev,
      messages: prev.messages.map((msg): ChatMessage => {
        if (msg.id !== messageId) return msg;
        const updatedCards = msg.cards?.map((card) =>
          card.type === "approval"
            ? { ...card, status: action === "approve" ? "approved" : "rejected" }
            : card,
        );
        return {
          ...msg,
          ...(updatedCards !== undefined ? { cards: updatedCards } : {}),
        };
      }),
    }));
  }

  return <ChatPanel session={session} onApprovalAction={handleApprovalAction} />;
}

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

  it("updates_card_status_to_approved_when_approve_clicked", () => {
    render(<ChatPanelWithApprovalState initialSession={mockSession} />);

    expect(screen.getByText("requires_approval")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("approval-approve-btn"));

    expect(screen.getByText("approved")).toBeInTheDocument();
    expect(screen.queryByText("requires_approval")).not.toBeInTheDocument();
  });

  it("updates_card_status_to_rejected_when_reject_clicked", () => {
    render(<ChatPanelWithApprovalState initialSession={mockSession} />);

    expect(screen.getByText("requires_approval")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("approval-reject-btn"));

    expect(screen.getByText("rejected")).toBeInTheDocument();
    expect(screen.queryByText("requires_approval")).not.toBeInTheDocument();
  });

  it("calls_onApprovalAction_with_correct_arguments", () => {
    const onApprovalAction = vi.fn<() => void>();
    render(
      <ChatPanel
        session={mockSession}
        onApprovalAction={onApprovalAction}
      />,
    );

    fireEvent.click(screen.getByTestId("approval-approve-btn"));
    expect(onApprovalAction).toHaveBeenCalledWith("session_test", "msg_1", "approve");

    fireEvent.click(screen.getByTestId("approval-reject-btn"));
    expect(onApprovalAction).toHaveBeenCalledWith("session_test", "msg_1", "reject");
  });

  it("renders_empty_session_without_crashing", () => {
    render(<ChatPanel session={mockEmptySession} />);

    expect(screen.getByText("空会话")).toBeInTheDocument();
    expect(screen.getByText("0 条消息")).toBeInTheDocument();
  });

  it("preserves_line_breaks_in_message_content", () => {
    render(<ChatPanel session={mockSessionWithMultiline} />);

    const message = screen.getByText(/第一行/);
    expect(message).toBeInTheDocument();
    // The parent should preserve whitespace
    expect(message.closest("p")).toHaveClass("whitespace-pre-wrap");
  });

  it("renders_citation_badges_as_clickable_buttons", () => {
    render(<ChatPanel session={mockSessionWithCitations} />);

    const badge1 = screen.getByTestId("citation-ref-cit_1");
    const badge2 = screen.getByTestId("citation-ref-cit_2");

    expect(badge1).toBeInTheDocument();
    expect(badge2).toBeInTheDocument();
    expect(badge1.tagName).toBe("BUTTON");
    expect(badge2.tagName).toBe("BUTTON");
    expect(badge1).toHaveTextContent("source scope");
    expect(badge2).toHaveTextContent("approval boundary");
  });

  it("calls_onCitationClick_when_citation_badge_clicked", () => {
    const onCitationClick = vi.fn<(citationId: string) => void>();
    render(
      <ChatPanel
        session={mockSessionWithCitations}
        onCitationClick={onCitationClick}
      />,
    );

    const badge1 = screen.getByTestId("citation-ref-cit_1");
    fireEvent.click(badge1);
    // Should use citationId when available (falls back to id)
    expect(onCitationClick).toHaveBeenCalledWith("cit_decision_context");

    const badge2 = screen.getByTestId("citation-ref-cit_2");
    fireEvent.click(badge2);
    // No citationId on this one, so falls back to id
    expect(onCitationClick).toHaveBeenCalledWith("cit_2");
  });
});
