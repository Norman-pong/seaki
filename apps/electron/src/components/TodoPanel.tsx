import { CheckCircle2, Circle, Clock } from "lucide-react";

export interface TodoItem {
  readonly id: string;
  readonly text: string;
  readonly done: boolean;
  readonly time?: string;
}

const mockTodos: TodoItem[] = [
  { id: "todo_1", text: "整理 Sunclaw 应用架构", done: true, time: "10:29" },
  { id: "todo_2", text: "收敛开发侧提示词入口", done: true, time: "10:25" },
  { id: "todo_3", text: "Review 架构约束文档", done: false },
  { id: "todo_4", text: "更新 AGENTS.md 索引", done: false },
];

export function TodoPanel() {
  const doneCount = mockTodos.filter((t) => t.done).length;
  const total = mockTodos.length;

  return (
    <div className="todo-panel" aria-label="todo list">
      <div className="todo-header">
        <h3 className="panel-section-title">待办</h3>
        <span className="todo-progress">
          {doneCount}/{total}
        </span>
      </div>
      <div className="todo-list">
        {mockTodos.map((todo) => (
          <div
            key={todo.id}
            className={`todo-item ${todo.done ? "done" : ""}`}
          >
            {todo.done ? (
              <CheckCircle2 size={14} className="todo-icon done" />
            ) : (
              <Circle size={14} className="todo-icon" />
            )}
            <span className="todo-text">{todo.text}</span>
            {todo.time ? (
              <span className="todo-time">
                <Clock size={10} />
                {todo.time}
              </span>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}
