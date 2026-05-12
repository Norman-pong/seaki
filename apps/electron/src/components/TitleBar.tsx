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
    <header className="relative flex h-[46px] shrink-0 items-center justify-between border-b border-[color-mix(in_oklch,var(--border)_72%,transparent)] bg-background px-4 select-none">
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
        <span className="text-muted-foreground text-[0.8125rem] font-medium">seaki</span>
      </div>

      <button
        type="button"
        data-testid="title-command"
        className="absolute left-1/2 flex w-[min(36vw,440px)] max-[900px]:w-[min(34vw,320px)] -translate-x-1/2 items-center gap-2 rounded-lg border border-[color-mix(in_oklch,var(--border)_76%,transparent)] bg-[color-mix(in_oklch,var(--card)_74%,var(--background))] px-2.5 py-1.5 text-muted-foreground text-[0.8125rem] leading-none shadow-none transition-[border-color,box-shadow,background] duration-150 hover:border-[color-mix(in_oklch,var(--border)_72%,var(--foreground))] hover:bg-[color-mix(in_oklch,var(--card)_94%,var(--muted))] focus-visible:border-ring focus-visible:shadow-[0_0_0_3px_color-mix(in_oklch,var(--ring)_20%,transparent)] focus-visible:outline-none"
        onClick={onOpenCommandPalette}
        aria-label="打开命令面板"
        aria-haspopup="dialog"
        aria-controls="command-palette"
      >
        <Search data-icon="inline-start" />
        <span className="truncate">{session?.title ?? "AI Wiki 工作站"}</span>
        <kbd className="ml-auto rounded-md bg-muted px-[0.35rem] py-0.5 text-[0.6875rem] font-semibold">⌘K</kbd>
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
          disabled
        >
          <Layout data-icon="icon" />
        </Button>
        <Button
          type="button"
          variant="outline"
          size="icon"
          className="size-7 text-muted-foreground"
          aria-label="fullscreen"
          disabled
        >
          <Maximize2 data-icon="icon" />
        </Button>
      </div>
    </header>
  );
}
