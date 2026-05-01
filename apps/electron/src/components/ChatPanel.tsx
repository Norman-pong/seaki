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
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
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

function ChatCardItem({ card }: { readonly card: ChatCard }) {

  const isDone = card.status === "committed" || card.status === "ready";

  return (
    <Card size="sm" className="mt-2 chat-card">
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
}

function ChatMessageItem({ message }: { readonly message: ChatMessage }) {
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
            "px-4 py-3 rounded-2xl text-sm leading-relaxed shadow-sm",
            isUser
              ? "bg-primary text-primary-foreground rounded-br-sm"
              : "bg-card ring-1 ring-border rounded-bl-sm"
          )}
        >
          <p>{message.content}</p>
          {message.cards && message.cards.length > 0 && (
            <div className="flex flex-col gap-2 mt-3">
              {message.cards.map((card, index) => (
                <ChatCardItem key={`${card.title}_${index}`} card={card} />
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
}

export function ChatPanel({ session }: ChatPanelProps) {
  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session.messages]);

  return (
    <section className="flex flex-col h-full bg-background" aria-label="chat flow">
      <div className="flex items-center gap-3 px-5 py-3 border-b bg-background">
        <h2 className="text-[15px] font-semibold">{session.title}</h2>
        <span className="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded-full">
          {session.messages.length} 条消息
        </span>
      </div>

      <div className="flex-1 overflow-y-auto p-5 flex flex-col gap-4" aria-live="polite" aria-atomic="false">
        {session.messages.map((message) => (
          <ChatMessageItem key={message.id} message={message} />
        ))}
        <div ref={messagesEndRef} />
      </div>

      <div className="px-5 py-4 border-t bg-background">
        <div className="flex items-end gap-2.5 bg-muted rounded-2xl px-4 py-2.5 border border-transparent focus-within:border-primary/20 focus-within:bg-background focus-within:ring-2 focus-within:ring-primary/5 transition-all">
          <textarea
            className="flex-1 bg-transparent resize-none text-sm leading-relaxed outline-none min-h-[24px] max-h-[120px] placeholder:text-muted-foreground chat-textarea"
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
            className="h-8 w-8 rounded-lg flex-shrink-0"
            onClick={() => setInput("")}
            aria-label="send"
          >
            <Send size={15} />
          </Button>
        </div>
      </div>
    </section>
  );
}
