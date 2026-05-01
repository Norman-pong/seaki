import {
  Code,
  ExternalLink,
  Layout,
  Maximize2,
  PanelLeft,
  PanelRight,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ChatSession } from "@/models/chatModel";

interface TitleBarProps {
  readonly session: ChatSession | undefined;
  readonly leftCollapsed?: boolean;
  readonly onToggleLeft?: () => void;
  readonly rightCollapsed?: boolean;
  readonly onToggleRight?: () => void;
}

export function TitleBar({
  session,
  leftCollapsed,
  onToggleLeft,
  rightCollapsed,
  onToggleRight,
}: TitleBarProps) {
  return (
    <header className="h-10 flex items-center justify-between px-3.5 bg-background border-b flex-shrink-0 select-none">
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded-full bg-red-400 border border-red-500/50" />
          <span className="w-3 h-3 rounded-full bg-yellow-400 border border-yellow-500/50" />
          <span className="w-3 h-3 rounded-full bg-green-400 border border-green-500/50" />
        </div>
        <div className="flex items-center gap-1.5 text-muted-foreground text-xs font-medium px-2.5 py-1 bg-muted rounded-md">
          <Code size={14} />
          <span>seaki</span>
        </div>
        {onToggleLeft && (
          <button
            type="button"
            className={cn(
              "flex items-center justify-center w-7 h-7 rounded-md transition-colors",
              leftCollapsed
                ? "text-primary bg-primary/10"
                : "text-muted-foreground hover:bg-muted"
            )}
            aria-label={leftCollapsed ? "展开左侧面板" : "收起左侧面板"}
            onClick={onToggleLeft}
          >
            <PanelLeft size={14} />
          </button>
        )}
      </div>

      <h1
        className={cn(
          "absolute left-1/2 -translate-x-1/2 text-sm font-medium truncate max-w-md"
        )}
      >
        {session?.title ?? "AI Wiki 工作站"}
      </h1>

      <div className="flex items-center gap-1.5">
        {onToggleRight && (
          <button
            type="button"
            className={cn(
              "flex items-center justify-center w-7 h-7 rounded-md transition-colors",
              rightCollapsed
                ? "text-primary bg-primary/10"
                : "text-muted-foreground hover:bg-muted"
            )}
            aria-label={rightCollapsed ? "展开右侧面板" : "收起右侧面板"}
            onClick={onToggleRight}
          >
            <PanelRight size={14} />
          </button>
        )}
        <button
          type="button"
          className="flex items-center gap-1.5 px-2.5 py-1 text-xs text-muted-foreground border rounded-md hover:bg-muted transition-colors"
        >
          <ExternalLink size={13} />
          <span>在编辑器中打开</span>
        </button>
        <button
          type="button"
          className="flex items-center justify-center w-7 h-7 text-muted-foreground border rounded-md hover:bg-muted transition-colors"
          aria-label="layout"
        >
          <Layout size={13} />
        </button>
        <button
          type="button"
          className="flex items-center justify-center w-7 h-7 text-muted-foreground border rounded-md hover:bg-muted transition-colors"
          aria-label="fullscreen"
        >
          <Maximize2 size={13} />
        </button>
      </div>
    </header>
  );
}
