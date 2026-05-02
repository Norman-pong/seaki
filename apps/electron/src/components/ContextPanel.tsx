import { BookOpen, FileText, Puzzle, Settings } from "lucide-react";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface ContextTag {
  readonly id: string;
  readonly label: string;
  readonly icon: "skill" | "rule" | "file" | "other";
  readonly active?: boolean;
}

const contextTags: ContextTag[] = [
  { id: "ctx_1", label: "pipe command", icon: "skill", active: true },
  { id: "ctx_2", label: "llm-wiki", icon: "skill", active: true },
  { id: "ctx_3", label: "lore-commit", icon: "rule" },
  { id: "ctx_4", label: "AGENTS.md", icon: "file", active: true },
  { id: "ctx_5", label: "architecture.md", icon: "file" },
  { id: "ctx_6", label: "frontend.md", icon: "file" },
];

const iconMap = {
  skill: <Puzzle size={12} />,
  rule: <Settings size={12} />,
  file: <FileText size={12} />,
  other: <BookOpen size={12} />,
};

export function ContextPanel() {
  return (
    <Card size="sm" className="m-3 border-0 bg-transparent shadow-none">
      <CardHeader className="py-2">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide">
            上下文
          </h3>
          <span className="rounded-md bg-muted px-2 py-0.5 text-[11px] text-muted-foreground">
            6 sources
          </span>
        </div>
      </CardHeader>
      <CardContent className="pt-0 space-y-3">
        <div className="flex flex-wrap gap-2">
          {contextTags.map((tag) => (
            <span
              key={tag.id}
              className={cn(
                "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs transition-colors",
                tag.active
                  ? "bg-primary/10 text-primary border border-primary/20"
                  : "bg-muted text-muted-foreground hover:bg-muted/80"
              )}
            >
              {iconMap[tag.icon]}
              {tag.label}
            </span>
          ))}
        </div>

        <div className="space-y-1.5">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>上下文占用</span>
            <span className="font-semibold text-primary">29%</span>
          </div>
          <div className="h-1.5 bg-muted rounded-full overflow-hidden">
            <div
              className="h-full context-meter rounded-full transition-all duration-300"
              style={{ width: "29%" }}
            />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
