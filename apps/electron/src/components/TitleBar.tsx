import { Code, ExternalLink, Layout, Maximize2 } from "lucide-react";
import type { ChatSession } from "@/models/chatModel";

interface TitleBarProps {
  readonly session: ChatSession | undefined;
}

export function TitleBar({ session }: TitleBarProps) {
  return (
    <header className="title-bar">
      <div className="title-bar-left">
        <div className="window-controls">
          <span className="window-dot close" />
          <span className="window-dot minimize" />
          <span className="window-dot maximize" />
        </div>
        <div className="title-app-icon">
          <Code size={14} />
          <span>seaki</span>
        </div>
      </div>

      <div className="title-bar-center">
        <h1 className="title-session-name">
          {session?.title ?? "AI Wiki 工作站"}
        </h1>
      </div>

      <div className="title-bar-right">
        <button type="button" className="title-action" aria-label="open in editor">
          <ExternalLink size={14} />
          <span>在编辑器中打开</span>
        </button>
        <button type="button" className="title-action" aria-label="layout">
          <Layout size={14} />
        </button>
        <button type="button" className="title-action" aria-label="fullscreen">
          <Maximize2 size={14} />
        </button>
      </div>
    </header>
  );
}
