import React, { useState, useRef, useEffect } from "react";
import {
  Send,
  FileText,
  Search,
  AlertCircle,
  Link2,
  CheckCircle2,
  Bot,
  User,
  Sparkles,
  Paperclip,
  SlidersHorizontal,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { ChatSession, ChatMessage, ChatCard } from "@/models/chatModel";

const ICON_MAP: Record<ChatCard["type"], React.ReactNode> = {
  wiki: <FileText size={14} />,
  search: <Search size={14} />,
  approval: <AlertCircle size={14} />,
  citation: <Link2 size={14} />,
  link: <Link2 size={14} />,
};

interface ChatPanelProps {
  readonly session: ChatSession;
}

const ChatCardItem = React.memo(function ChatCardItem({ card }: { readonly card: ChatCard }) {
  const isDone = card.status === "committed" || card.status === "ready";

  return (
    <Card size="sm" className="mt-2 chat-card shadow-none">
      <CardHeader className="flex flex-row items-center gap-2 py-2">
        <span className="text-muted-foreground">{ICON_MAP[card.type]}</span>
        <span className="font-medium text-sm flex-1">{card.title}</span>
        {card.status && (
          <div className="flex items-center gap-1.5">
            {isDone && <CheckCircle2 size={12} className="text-green-500" />}
            <Badge variant="outline" className="text-xs h-5">
              {card.status}
            </Badge>
          </div>
        )}
      </CardHeader>
      {(card.content || card.snippet) && (
        <CardContent className="pt-0 pb-3">
          <p className="text-sm text-muted-foreground leading-relaxed">
            {card.content ?? card.snippet}
          </p>
          {card.citationRefs && card.citationRefs.length > 0 && (
            <div className="flex flex-wrap gap-1.5 mt-2.5">
              {card.citationRefs.map((ref) => (
                <Badge key={ref.id} variant="secondary" className="text-xs h-5">
                  {ref.label}
                </Badge>
              ))}
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
});

const ChatMessageItem = React.memo(function ChatMessageItem({ message }: { readonly message: ChatMessage }) {
  const isUser = message.role === "user";

  return (
    <div
      className={cn(
        "chat-message flex gap-3 max-w-[92%]",
        isUser ? "self-end flex-row-reverse" : "self-start"
      )}
    >
      <div
        className={cn(
          "flex-shrink-0 w-7 h-7 rounded-full flex items-center justify-center",
          isUser ? "bg-primary text-primary-foreground" : "bg-secondary"
        )}
        aria-label={isUser ? "用户" : "助手"}
      >
        {isUser ? <User size={14} /> : <Bot size={14} />}
      </div>
      <div className="flex flex-col gap-1 min-w-0">
        <div
          className={cn(
            "px-4 py-3 rounded-2xl text-sm leading-relaxed",
            isUser
              ? "bg-primary text-primary-foreground rounded-br-sm"
              : "bg-card ring-1 ring-border rounded-bl-sm"
          )}
        >
          <p>{message.content}</p>
          {message.cards && message.cards.length > 0 && (
            <div className="flex flex-col gap-2 mt-3">
              {message.cards.map((card, index) => (
                <ChatCardItem key={`${message.id}_card_${card.type}_${index}`} card={card} />
              ))}
            </div>
          )}
        </div>
        <span
          className={cn(
            "text-[11px] text-muted-foreground px-1",
            isUser && "text-right"
          )}
        >
          {new Date(message.timestamp).toLocaleTimeString("zh-CN", {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </span>
      </div>
    </div>
  );
});

export function ChatPanel({ session }: ChatPanelProps) {
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session.messages]);

  return (
    <section className="flex flex-col h-full bg-background" aria-label="chat flow">
      <div className="chat-panel-header">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="truncate text-[15px] font-semibold">{session.title}</h2>
            <Badge variant="secondary" className="h-5 text-[11px]">
              {session.messages.length} 条消息
            </Badge>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground">
            本轮输出默认进入 proposal，写入 workspace 前需要 citation 与审批校验。
          </p>
        </div>
        <div className="chat-panel-header__metrics">
          <span>context 29%</span>
          <span>citations 2</span>
          <span>risk low</span>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-5 flex flex-col gap-4" aria-live="polite" aria-atomic="false">
        {session.messages.map((message) => (
          <ChatMessageItem key={message.id} message={message} />
        ))}
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-composer">
        <div className="composer-shell">
          <div className="flex items-center justify-between gap-2 border-b px-3 py-2">
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Sparkles size={13} />
              <span>agent draft</span>
            </div>
            <div className="flex items-center gap-1">
              <Button variant="ghost" size="icon" className="size-7 text-muted-foreground" aria-label="attach source">
                <Paperclip data-icon="icon" />
              </Button>
              <Button variant="ghost" size="icon" className="size-7 text-muted-foreground" aria-label="adjust context">
                <SlidersHorizontal data-icon="icon" />
              </Button>
            </div>
          </div>
          <div className="flex items-end gap-2.5 px-3 py-2.5">
            <Textarea
              className="chat-textarea min-h-12 max-h-[120px] flex-1 resize-none border-0 bg-transparent px-0 py-0 text-sm leading-relaxed shadow-none outline-none focus-visible:ring-0"
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
              className="size-8 flex-shrink-0 rounded-lg"
              onClick={() => setInput("")}
              aria-label="send"
            >
              <Send data-icon="icon" />
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}
