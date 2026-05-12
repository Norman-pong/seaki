import { FolderOpen, MessageSquare, Plus, X, Zap } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ChatSession } from "@/models/chatModel";
import { useVirtualList } from "@/hooks/useVirtualList";

interface SessionSidebarProps {
  readonly sessions: readonly ChatSession[];
  readonly activeSessionId: string;
  readonly onSelectSession: (sessionId: string) => void;
  readonly onNewSession: () => void;
  readonly onDeleteSession?: (sessionId: string) => void;
  readonly isCollapsed?: boolean;
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
  isCollapsed,
}: SessionSidebarProps) {
  const { containerRef, visibleItems, totalHeight, offsetTop } = useVirtualList(
    sessions,
    64,
    3,
  );

  return (
    <aside
      className={cn(
        "flex flex-col h-full sidebar-surface border-r transition-transform duration-300 ease-out",
        isCollapsed && "-translate-x-full"
      )}
      aria-label="session history"
    >
      <div className="flex flex-col gap-1 px-3 py-3">
        <Button
          variant="ghost"
          size="sm"
          className="h-8 justify-start gap-2 text-sm"
          onClick={onNewSession}
        >
          <Plus data-icon="inline-start" />
          新建任务
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-8 justify-start gap-2 text-sm"
        >
          <Zap data-icon="inline-start" />
          技能
        </Button>
      </div>

      <div className="px-3 pb-2">
        <div className="flex items-center gap-2 px-2.5 py-2 text-xs font-semibold text-muted-foreground uppercase tracking-wide">
          <FolderOpen size={14} />
          项目列表
        </div>
      </div>

      <div
        ref={containerRef}
        className="flex-1 overflow-y-auto px-2 pb-2 flex flex-col gap-1"
      >
        <div style={{ height: totalHeight }}>
          <div style={{ paddingTop: offsetTop }}>
            {visibleItems.map((session) => {
              const isActive = session.id === activeSessionId;
              return (
                <div
                  key={session.id}
                  className="relative group"
                  style={{ contain: "content", willChange: "transform" }}
                >
                  <button
                    type="button"
                    data-testid="session-item"
                    className={cn(
                      "session-item w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm text-left transition-colors pr-9",
                      isActive
                        ? "bg-primary/10 text-primary"
                        : "hover:bg-muted text-foreground"
                    )}
                    onClick={() => onSelectSession(session.id)}
                    aria-current={isActive ? "true" : undefined}
                  >
                    <MessageSquare
                      size={15}
                      className={cn(
                        "flex-shrink-0",
                        isActive ? "text-primary" : "text-muted-foreground"
                      )}
                    />
                    <div className="flex flex-col min-w-0 flex-1">
                      <span className="font-medium truncate">{session.title}</span>
                      <span className="flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
                        <span>{formatTime(session.timestamp)}</span>
                        <span>{session.messages.length} 条</span>
                      </span>
                    </div>
                  </button>
                  {onDeleteSession && (
                    <button
                      type="button"
                      className="absolute right-1.5 top-1/2 -translate-y-1/2 flex items-center justify-center w-5 h-5 rounded opacity-0 group-hover:opacity-100 focus-visible:opacity-100 transition-opacity hover:bg-destructive/10 hover:text-destructive"
                      aria-label="delete session"
                      onClick={(e) => {
                        e.stopPropagation();
                        onDeleteSession(session.id);
                      }}
                    >
                      <X size={13} />
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </aside>
  );
}
