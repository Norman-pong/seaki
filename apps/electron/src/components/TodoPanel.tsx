import { CheckCircle2, Circle, Clock } from "lucide-react";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export interface TodoItem {
  readonly id: string;
  readonly text: string;
  readonly done: boolean;
  readonly time?: string;
}

const mockTodos: TodoItem[] = [
  { id: "todo_1", text: "整理 seaki 应用架构", done: true, time: "10:29" },
  { id: "todo_2", text: "收敛开发侧提示词入口", done: true, time: "10:25" },
  { id: "todo_3", text: "Review 架构约束文档", done: false },
  { id: "todo_4", text: "更新 AGENTS.md 索引", done: false },
];

export function TodoPanel() {
  const doneCount = mockTodos.filter((t) => t.done).length;
  const total = mockTodos.length;

  return (
    <Card size="sm" className="m-3 border-0 bg-transparent shadow-none">
      <CardHeader className="flex flex-row items-center justify-between py-2">
        <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide">
          待办
        </h3>
        <span className="text-xs text-muted-foreground font-medium">
          {doneCount}/{total}
        </span>
      </CardHeader>
      <CardContent className="pt-0 space-y-1">
        {mockTodos.map((todo) => (
          <div
            key={todo.id}
            className={cn(
              "flex items-center gap-2.5 px-2 py-1.5 rounded-md text-sm transition-colors hover:bg-muted/60",
              todo.done && "text-muted-foreground"
            )}
          >
            {todo.done ? (
              <CheckCircle2 size={14} className="text-green-500 flex-shrink-0" />
            ) : (
              <Circle size={14} className="text-muted-foreground flex-shrink-0" />
            )}
            <span className={cn("flex-1 truncate", todo.done && "line-through")}>
              {todo.text}
            </span>
            {todo.time && (
              <span className="flex items-center gap-1 text-[11px] text-muted-foreground flex-shrink-0">
                <Clock size={10} />
                {todo.time}
              </span>
            )}
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
