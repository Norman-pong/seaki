import { BookOpen, FileText, Puzzle, Settings } from "lucide-react";

interface ContextTag {
  readonly id: string;
  readonly label: string;
  readonly icon: "skill" | "rule" | "file" | "other";
  readonly active?: boolean;
}

const contextTags: ContextTag[] = [
  { id: "ctx_1", label: "git-commit", icon: "skill", active: true },
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
    <div className="context-panel" aria-label="context panel">
      <h3 className="panel-section-title">上下文</h3>
      <div className="context-tags">
        {contextTags.map((tag) => (
          <span
            key={tag.id}
            className={`context-tag ${tag.active ? "active" : ""}`}
          >
            {iconMap[tag.icon]}
            {tag.label}
          </span>
        ))}
      </div>

      <div className="context-progress">
        <div className="context-progress-header">
          <span>上下文占用</span>
          <span className="context-progress-value">29%</span>
        </div>
        <div className="context-progress-bar">
          <div
            className="context-progress-fill"
            style={{ width: "29%" }}
          />
        </div>
      </div>
    </div>
  );
}
