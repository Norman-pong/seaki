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

    const deleteBtn = screen.getByLabelText("delete session");
    fireEvent.click(deleteBtn);
    expect(onDeleteSession).toHaveBeenCalledWith("session_1");
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
