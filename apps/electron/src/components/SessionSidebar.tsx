import { MessageSquare, Plus, X, Zap, FolderOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ChatSession } from "@/models/chatModel";

interface SessionSidebarProps {
  readonly sessions: readonly ChatSession[];
  readonly activeSessionId: string;
  readonly onSelectSession: (sessionId: string) => void;
  readonly onNewSession: () => void;
  readonly onDeleteSession?: (sessionId: string) => void;
}

function formatTime(timestamp: string): string {
  const date = new Date(timestamp);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  if (isToday) {
    return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  }

  return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
}

export function SessionSidebar({
  sessions,
  activeSessionId,
  onSelectSession,
  onNewSession,
  onDeleteSession,
}: SessionSidebarProps) {
  return (
    <aside className="session-sidebar" aria-label="session history">
      <div className="session-toolbar">
        <Button
          variant="ghost"
          size="sm"
          type="button"
          className="session-toolbar-btn"
          onClick={onNewSession}
        >
          <Plus size={14} />
          <span>新建任务</span>
        </Button>
        <Button
          variant="ghost"
          size="sm"
          type="button"
          className="session-toolbar-btn"
        >
          <Zap size={14} />
          <span>技能</span>
        </Button>
      </div>

      <div className="session-projects">
        <div className="session-project-header">
          <FolderOpen size={14} />
          <span>项目列表</span>
        </div>
      </div>

      <div className="session-list">
        {sessions.map((session) => (
          <button
            key={session.id}
            type="button"
            className={`session-item ${session.id === activeSessionId ? "active" : ""}`}
            onClick={() => onSelectSession(session.id)}
            aria-current={session.id === activeSessionId ? "true" : undefined}
          >
            <MessageSquare size={15} className="session-icon" />
            <div className="session-info">
              <span className="session-name">{session.title}</span>
              <span className="session-time">{formatTime(session.timestamp)}</span>
            </div>
            {onDeleteSession && session.id === activeSessionId && (
              <button
                type="button"
                className="session-delete"
                aria-label="delete session"
                onClick={(e) => {
                  e.stopPropagation();
                  onDeleteSession(session.id);
                }}
              >
                <X size={13} />
              </button>
            )}
          </button>
        ))}
      </div>
    </aside>
  );
}
