import { FilePlus2, GitCompare, RefreshCw, Search, Terminal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";

export type CommandPaletteAction =
  | "index-rebuild"
  | "approval-review"
  | "attach-source"
  | "pipe-dry-run"
  | "wiki-search";

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
  readonly selected?: boolean;
}

const commands: readonly CommandItem[] = [
  {
    id: "index-rebuild",
    title: "重建 stale workspace index",
    detail: "index.rebuild · 后台安全任务",
    icon: RefreshCw,
    shortcut: "Enter",
    selected: true,
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
];

export function CommandPalette({ open, onClose, onSelectCommand }: CommandPaletteProps) {
  if (!open) return null;

  return (
    <div
      id="command-palette"
      className="command-palette-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="command-palette-title"
      onMouseDown={onClose}
    >
      <Card
        size="sm"
        className="command-palette-card"
        onMouseDown={(event) => event.stopPropagation()}
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
          <div className="command-palette-input" role="search">
            <Search data-icon="inline-start" />
            <span>rebuild index</span>
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-1.5 pt-0">
          {commands.map((command) => {
            const Icon = command.icon;
            return (
              <button
                key={command.id}
                type="button"
                className="command-row"
                data-selected={command.selected ? "true" : undefined}
                onClick={() => onSelectCommand(command.id)}
              >
                <span className="command-row__icon">
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
              </button>
            );
          })}
        </CardContent>
      </Card>
    </div>
  );
}
