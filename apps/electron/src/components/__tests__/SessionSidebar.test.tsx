import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { SessionSidebar } from "../SessionSidebar";
import type { ChatSession } from "@/models/chatModel";

const mockSessions: readonly ChatSession[] = [
  {
    id: "session_1",
    title: "Wiki 导入讨论",
    timestamp: "2026-05-08T10:00:00+08:00",
    messages: [
      { id: "msg_1", role: "user", content: "hello", timestamp: "2026-05-08T10:00:00+08:00" },
    ],
  },
  {
    id: "session_2",
    title: "架构决策审查",
    timestamp: "2026-05-07T16:00:00+08:00",
    messages: [],
  },
];

describe("SessionSidebar", () => {
  it("renders_session_list", () => {
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("Wiki 导入讨论")).toBeInTheDocument();
    expect(screen.getByText("架构决策审查")).toBeInTheDocument();
  });

  it("calls_onSelectSession_when_session_clicked", () => {
    const onSelectSession = vi.fn<() => void>();
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={onSelectSession}
        onNewSession={vi.fn<() => void>()}
      />,
    );

    fireEvent.click(screen.getByText("架构决策审查"));
    expect(onSelectSession).toHaveBeenCalledWith("session_2");
  });

  it("calls_onNewSession_when_new_session_button_clicked", () => {
    const onNewSession = vi.fn<() => void>();
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={onNewSession}
      />,
    );

    fireEvent.click(screen.getByText("新建任务"));
    expect(onNewSession).toHaveBeenCalled();
  });

  it("marks_active_session_with_aria_current", () => {
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
      />,
    );

    const activeBtn = screen.getByText("Wiki 导入讨论").closest("button");
    expect(activeBtn).toHaveAttribute("aria-current", "true");
  });

  it("shows_delete_button_for_active_session_on_hover", () => {
    const onDeleteSession = vi.fn<() => void>();
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
        onDeleteSession={onDeleteSession}
      />,
    );

    // 找到活跃会话（session_1）对应的删除按钮
    const session1Row = screen.getByText("Wiki 导入讨论").closest(".group")!;
    const deleteBtn = session1Row.querySelector('[aria-label="delete session"]') as HTMLElement;
    fireEvent.click(deleteBtn);
    expect(onDeleteSession).toHaveBeenCalledWith("session_1");
  });

  it("shows delete button for inactive session on hover", () => {
    const onDeleteSession = vi.fn<() => void>();
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
        onDeleteSession={onDeleteSession}
      />,
    );

    // 非活跃会话（session_2）也应有删除按钮
    const session2Row = screen.getByText("架构决策审查").closest(".group")!;
    const deleteBtn = session2Row.querySelector('[aria-label="delete session"]');
    expect(deleteBtn).toBeInTheDocument();
  });

  it("calls onDeleteSession when inactive session delete button clicked", () => {
    const onDeleteSession = vi.fn<() => void>();
    render(
      <SessionSidebar
        sessions={mockSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
        onDeleteSession={onDeleteSession}
      />,
    );

    // 找到非活跃会话的删除按钮
    const deleteButtons = screen.getAllByLabelText("delete session");
    // session_2 是非活跃会话，它的删除按钮是第二个
    const inactiveDeleteBtn = deleteButtons.find((btn) =>
      btn.closest(".group")?.querySelector("button[aria-current]") === null,
    )!;
    fireEvent.click(inactiveDeleteBtn);
    expect(onDeleteSession).toHaveBeenCalledWith("session_2");
  });

  it("shows delete button for every session", () => {
    const onDeleteSession = vi.fn<() => void>();
    const threeSessions: readonly ChatSession[] = [
      ...mockSessions,
      {
        id: "session_3",
        title: "第三个会话",
        timestamp: "2026-05-06T08:00:00+08:00",
        messages: [],
      },
    ];
    render(
      <SessionSidebar
        sessions={threeSessions}
        activeSessionId="session_1"
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
        onDeleteSession={onDeleteSession}
      />,
    );

    const deleteButtons = screen.getAllByLabelText("delete session");
    expect(deleteButtons).toHaveLength(3);
  });

  it("renders_empty_sessions", () => {
    render(
      <SessionSidebar
        sessions={[]}
        activeSessionId=""
        onSelectSession={vi.fn<() => void>()}
        onNewSession={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("新建任务")).toBeInTheDocument();
    expect(screen.getByText("项目列表")).toBeInTheDocument();
  });
});
