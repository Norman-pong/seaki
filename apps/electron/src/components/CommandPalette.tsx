import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { FilePlus2, GitCompare, RefreshCw, Search, Sparkles, Terminal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export type CommandPaletteAction =
  | "index-rebuild"
  | "approval-review"
  | "attach-source"
  | "pipe-dry-run"
  | "wiki-search"
  | "compose-answer";

interface CommandPaletteProps {
  readonly open: boolean;
  readonly onClose: () => void;
  readonly onSelectCommand: (action: CommandPaletteAction) => void;
}

interface CommandItem {
  readonly id: CommandPaletteAction;
  readonly title: string;
  readonly detail: string;
  readonly icon: typeof RefreshCw;
  readonly shortcut: string;
}

const ALL_COMMANDS: readonly CommandItem[] = [
  {
    id: "index-rebuild",
    title: "重建 stale workspace index",
    detail: "index.rebuild · 后台安全任务",
    icon: RefreshCw,
    shortcut: "Enter",
  },
  {
    id: "approval-review",
    title: "打开 approval diff",
    detail: "approval.reviewPatch · 需要人工决策",
    icon: GitCompare,
    shortcut: "⌘R",
  },
  {
    id: "attach-source",
    title: "附加本机 source",
    detail: "files.prepareUserSelected · 仅创建 opaque ref",
    icon: FilePlus2,
    shortcut: "⌘O",
  },
  {
    id: "pipe-dry-run",
    title: "运行 pipeline dry-run",
    detail: "pipe.dryRun · 不产生副作用",
    icon: Terminal,
    shortcut: "⌘↵",
  },
  {
    id: "wiki-search",
    title: "搜索 committed wiki",
    detail: "search.query · daemon visibility checked",
    icon: Search,
    shortcut: "/",
  },
  {
    id: "compose-answer",
    title: "AI 问答（Citation-backed）",
    detail: "compose.answer · LLM 生成带引用标注的回答",
    icon: Sparkles,
    shortcut: "⌘⇧A",
  },
];

export function CommandPalette({ open, onClose, onSelectCommand }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const commands = useMemo(() => {
    if (!query.trim()) return ALL_COMMANDS;
    const q = query.toLowerCase();
    return ALL_COMMANDS.filter(
      (cmd) =>
        cmd.title.toLowerCase().includes(q) || cmd.detail.toLowerCase().includes(q),
    );
  }, [query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSelectedIndex(0);
    inputRef.current?.focus();
  }, [open]);

  useEffect(() => {
    setSelectedIndex((prev) => (prev >= commands.length ? 0 : prev));
  }, [commands.length]);

  const handleSelect = useCallback(
    (action: CommandPaletteAction) => {
      onSelectCommand(action);
      onClose();
    },
    [onSelectCommand, onClose],
  );

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelectedIndex((prev) => (prev + 1 >= commands.length ? 0 : prev + 1));
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((prev) => (prev - 1 < 0 ? commands.length - 1 : prev - 1));
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        const cmd = commands[selectedIndex];
        if (cmd) {
          handleSelect(cmd.id);
        }
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key === "Tab") {
        const container = containerRef.current;
        if (!container) return;
        const focusables = container.querySelectorAll<HTMLElement>(
          "button, input, [tabindex]:not([tabindex='-1'])",
        );
        if (focusables.length === 0) return;
        const first = focusables[0]!;
        const last = focusables[focusables.length - 1]!;
        if (!first || !last) return;
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    },
    [commands, selectedIndex, handleSelect, onClose],
  );

  if (!open) return null;

  return (
    <div
      id="command-palette"
      className="fixed inset-0 z-50 flex items-start justify-center bg-[oklch(0.08_0.004_247/34%)] pt-24"
      role="dialog"
      aria-modal="true"
      aria-labelledby="command-palette-title"
      onMouseDown={onClose}
    >
      <Card
        ref={containerRef}
        size="sm"
        className="w-[min(600px,calc(100vw-2rem))] rounded-[0.875rem] bg-card shadow-[0_28px_80px_oklch(0_0_0/22%),0_8px_24px_oklch(0_0_0/10%)]"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={handleKeyDown}
        data-testid="command-palette-card"
      >
        <CardHeader className="gap-1 pb-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <h2 id="command-palette-title" className="text-base font-semibold">
                Command Palette
              </h2>
              <p className="mt-1 text-xs text-muted-foreground">
                选择命令、打开审批任务，或把本机 source 接入当前 workspace。
              </p>
            </div>
            <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={onClose}>
              Esc
            </Button>
          </div>
          <div className="flex h-10 items-center gap-2.5 border border-border rounded-[0.625rem] bg-background px-3 text-foreground text-sm font-medium" role="search">
            <Search data-icon="inline-start" />
            <input
              ref={inputRef}
              type="text"
              className="flex-1 bg-transparent outline-none text-sm"
              placeholder="搜索命令…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              data-testid="command-palette-input"
              aria-label="搜索命令"
            />
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-1.5 pt-0" role="listbox">
          {commands.length === 0 && (
            <p className="text-sm text-muted-foreground py-4 text-center">无匹配命令</p>
          )}
          {commands.map((command, index) => {
            const Icon = command.icon;
            const isSelected = index === selectedIndex;
            return (
              <div
                key={command.id}
                role="option"
                tabIndex={0}
                className={cn(
                  "flex w-full items-center gap-3 border border-transparent rounded-[0.625rem] bg-transparent px-3 py-2.5 text-foreground transition-colors duration-150",
                  "hover:border-border hover:bg-muted",
                  isSelected && "border-border bg-muted",
                )}
                data-selected={isSelected ? "true" : undefined}
                aria-selected={isSelected ? "true" : "false"}
                data-testid={`command-row-${command.id}`}
                onClick={() => handleSelect(command.id)}
                onMouseEnter={() => setSelectedIndex(index)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    handleSelect(command.id);
                  }
                }}
              >
                <span className="inline-flex w-[1.875rem] h-[1.875rem] shrink-0 items-center justify-center border border-border rounded-lg bg-card text-muted-foreground">
                  <Icon data-icon="icon" />
                </span>
                <span className="min-w-0 flex-1 text-left">
                  <span className="block truncate text-sm font-semibold">
                    {command.title}
                  </span>
                  <span className="block truncate text-xs text-muted-foreground">
                    {command.detail}
                  </span>
                </span>
                <Badge variant="outline" className="h-5 text-[11px]">
                  {command.shortcut}
                </Badge>
              </div>
            );
          })}
        </CardContent>
      </Card>
    </div>
  );
}
