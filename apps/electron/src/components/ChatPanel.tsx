import React, { useState, useRef, useEffect } from "react";
import {
  Send,
  FileText,
  Search,
  AlertCircle,
  Link2,
  CheckCircle2,
  Terminal,
  ChevronDown,
  ChevronRight,
  Bot,
  User,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { ChatSession, ChatMessage, ChatCard } from "@/models/chatModel";

interface ChatPanelProps {
  readonly session: ChatSession;
}

function ChatCardItem({ card }: { readonly card: ChatCard }) {
  const icons: Record<ChatCard["type"], React.ReactNode> = {
    wiki: <FileText size={14} />,
    search: <Search size={14} />,
    approval: <AlertCircle size={14} />,
    citation: <Link2 size={14} />,
    link: <Link2 size={14} />,
  };

  const statusIcons: Record<string, React.ReactNode> = {
    committed: <CheckCircle2 size={12} className="status-done" />,
    ready: <CheckCircle2 size={12} className="status-done" />,
  };

  return (
    <div className="chat-card">
      <div className="chat-card-header">
        <span className="chat-card-icon">{icons[card.type]}</span>
        <span className="chat-card-title">{card.title}</span>
        {card.status ? (
          <span className="chat-card-status">
            {statusIcons[card.status] ?? null}
            <Badge variant="outline" className="chat-card-badge">
              {card.status}
            </Badge>
          </span>
        ) : null}
      </div>
      {card.content || card.snippet ? (
        <p className="chat-card-body">{card.content ?? card.snippet}</p>
      ) : null}
      {card.citationRefs && card.citationRefs.length > 0 ? (
        <div className="chat-card-citations">
          {card.citationRefs.map((ref) => (
            <Badge key={ref.id} variant="secondary" className="citation-chip">
              {ref.label}
            </Badge>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function ThinkingBlock({ content }: { readonly content: string }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="thinking-block">
      <button
        type="button"
        className="thinking-header"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
      >
        {expanded ? (
          <ChevronDown size={14} />
        ) : (
          <ChevronRight size={14} />
        )}
        <span>思考</span>
      </button>
      {expanded ? (
        <div className="thinking-body">
          <p>{content}</p>
        </div>
      ) : null}
    </div>
  );
}

function CommandBlock({
  command,
  output,
}: {
  readonly command: string;
  readonly output: string;
}) {
  return (
    <div className="command-block">
      <div className="command-header">
        <Terminal size={13} />
        <span>命令已执行</span>
        <code className="command-code">{command}</code>
      </div>
      <pre className="command-output">{output}</pre>
    </div>
  );
}

function ChatMessageItem({ message }: { readonly message: ChatMessage }) {
  const isUser = message.role === "user";

  return (
    <div className={`chat-message ${isUser ? "user" : "assistant"}`}>
      <div className="chat-message-avatar">
        {isUser ? <User size={14} /> : <Bot size={14} />}
      </div>
      <div className="chat-message-content">
        <div className="chat-message-bubble">
          <p className="chat-message-text">{message.content}</p>
          {message.cards && message.cards.length > 0 ? (
            <div className="chat-cards">
              {message.cards.map((card, index) => (
                <ChatCardItem key={`${card.title}_${index}`} card={card} />
              ))}
            </div>
          ) : null}
        </div>
        <span className="chat-message-time">
          {new Date(message.timestamp).toLocaleTimeString("zh-CN", {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </span>
      </div>
    </div>
  );
}

export function ChatPanel({ session }: ChatPanelProps) {
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session.messages]);

  return (
    <section className="chat-panel" aria-label="chat flow">
      <div className="chat-header">
        <div className="chat-header-left">
          <h2 className="chat-session-title">{session.title}</h2>
          <span className="chat-session-meta">
            {session.messages.length} 条消息
          </span>
        </div>
      </div>

      <div className="chat-messages">
        {/* Mock thinking block for first assistant message */}
        {session.messages.some((m) => m.role === "assistant") ? (
          <ThinkingBlock content="正在分析 workspace 结构和导入队列状态，准备生成可视化报告..." />
        ) : null}

        {/* Mock command block */}
        <CommandBlock
          command="git status --porcelain"
          output={` M apps/electron/src/App.tsx
A  apps/electron/src/components/ChatPanel.tsx
D  apps/electron/src/components/OldPanel.tsx`}
        />

        {session.messages.map((message) => (
          <ChatMessageItem key={message.id} message={message} />
        ))}
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-input-area">
        <div className="chat-input-box">
          <textarea
            className="chat-textarea"
            placeholder="输入消息..."
            rows={2}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                setInput("");
              }
            }}
          />
          <Button
            size="icon"
            type="button"
            className="chat-send-btn"
            onClick={() => setInput("")}
            aria-label="send"
          >
            <Send size={16} />
          </Button>
        </div>
      </div>
    </section>
  );
}
