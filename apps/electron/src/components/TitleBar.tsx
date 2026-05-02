import {
  ExternalLink,
  Layout,
  Maximize2,
  PanelLeft,
  PanelRight,
  Search,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ChatSession } from "@/models/chatModel";

interface TitleBarProps {
  readonly session: ChatSession | undefined;
  readonly leftCollapsed?: boolean;
  readonly onToggleLeft?: () => void;
  readonly rightCollapsed?: boolean;
  readonly onToggleRight?: () => void;
  readonly onOpenCommandPalette?: () => void;
}

export function TitleBar({
  session,
  leftCollapsed,
  onToggleLeft,
  rightCollapsed,
  onToggleRight,
  onOpenCommandPalette,
}: TitleBarProps) {
  return (
    <header className="title-bar">
      <div className="flex items-center gap-2">
        {onToggleLeft && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className={cn(
              "size-7",
              leftCollapsed
                ? "text-primary bg-primary/10"
                : "text-muted-foreground hover:bg-muted"
            )}
            aria-label={leftCollapsed ? "展开左侧面板" : "收起左侧面板"}
            onClick={onToggleLeft}
          >
            <PanelLeft data-icon="icon" />
          </Button>
        )}
        <span className="title-product">seaki</span>
      </div>

      <button
        type="button"
        className="title-command"
        onClick={onOpenCommandPalette}
        aria-label="打开命令面板"
        aria-haspopup="dialog"
        aria-controls="command-palette"
      >
        <Search data-icon="inline-start" />
        <span className="truncate">{session?.title ?? "AI Wiki 工作站"}</span>
        <kbd>⌘K</kbd>
      </button>

      <div className="flex items-center gap-1.5">
        {onToggleRight && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className={cn(
              "size-7",
              rightCollapsed
                ? "text-primary bg-primary/10"
                : "text-muted-foreground hover:bg-muted"
            )}
            aria-label={rightCollapsed ? "展开右侧面板" : "收起右侧面板"}
            onClick={onToggleRight}
          >
            <PanelRight data-icon="icon" />
          </Button>
        )}
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 gap-1.5 text-xs text-muted-foreground"
        >
          <ExternalLink data-icon="inline-start" />
          <span>在编辑器中打开</span>
        </Button>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="size-7 text-muted-foreground"
          aria-label="layout"
        >
          <Layout data-icon="icon" />
        </Button>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="size-7 text-muted-foreground"
          aria-label="fullscreen"
        >
          <Maximize2 data-icon="icon" />
        </Button>
      </div>
    </header>
  );
}
