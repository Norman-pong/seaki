import React, { useState, useRef, useEffect, useCallback } from "react";
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
  Zap,
  Brain,
  FilePlus,
  MessageCircle,
  GitCompare,
  ThumbsUp,
  XCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { ChatSession, ChatMessage, ChatCard, SkillType } from "@/models/chatModel";
import { SKILLS } from "@/models/chatModel";
import { PipelinePanel } from "./PipelinePanel";
import type { PipelineRun } from "@/models/pipelineModel";
import { useVirtualList } from "@/hooks/useVirtualList";

const SKILL_ICON: Record<SkillType, React.ReactNode> = {
  "wiki-search": <Search size={12} />,
  "source-ingest": <FilePlus size={12} />,
  "pipeline-run": <Zap size={12} />,
  "memory-review": <Brain size={12} />,
  "channel-send": <MessageCircle size={12} />,
};

const ICON_MAP: Record<ChatCard["type"], React.ReactNode> = {
  wiki: <FileText size={14} />,
  search: <Search size={14} />,
  approval: <AlertCircle size={14} />,
  citation: <Link2 size={14} />,
  link: <Link2 size={14} />,
  pipeline: <Zap size={14} />,
  skill: <Sparkles size={14} />,
};

interface ChatPanelProps {
  readonly session: ChatSession;
  readonly onSendMessage?: (sessionId: string, content: string, skill?: string) => void;
  readonly onOpenReviewTab?: () => void;
  readonly onApprovalAction?: (sessionId: string, messageId: string, action: "approve" | "reject") => void;
}

const ChatCardItem = React.memo(function ChatCardItem({
  card,
  onOpenReviewTab,
  onApprove,
  onReject,
}: {
  readonly card: ChatCard;
  readonly onOpenReviewTab?: (() => void) | undefined;
  readonly onApprove?: (() => void) | undefined;
  readonly onReject?: (() => void) | undefined;
}) {
  const isDone = card.status === "committed" || card.status === "ready";
  const isApproval = card.type === "approval";

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
          {isApproval && (
            <div className="flex items-center gap-2 mt-3">
              <Button
                variant="secondary"
                size="sm"
                className="h-7 text-xs"
                onClick={onApprove}
                data-testid="approval-approve-btn"
              >
                <ThumbsUp size={12} className="mr-1" />
                批准
              </Button>
              <Button
                variant="destructive"
                size="sm"
                className="h-7 text-xs"
                onClick={onReject}
                data-testid="approval-reject-btn"
              >
                <XCircle size={12} className="mr-1" />
                拒绝
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={onOpenReviewTab}
                data-testid="approval-view-diff-btn"
              >
                <GitCompare size={12} className="mr-1" />
                查看 diff
              </Button>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
});

const ChatMessageItem = React.memo(function ChatMessageItem({
  message,
  onOpenReviewTab,
  onApprovalAction,
}: {
  readonly message: ChatMessage;
  readonly onOpenReviewTab?: (() => void) | undefined;
  readonly onApprovalAction?: ((messageId: string, action: "approve" | "reject") => void) | undefined;
}) {
  const isUser = message.role === "user";

  return (
    <div
      className={cn(
        "chat-message flex gap-3 max-w-[92%]",
        isUser ? "self-end flex-row-reverse" : "self-start"
      )}
      style={{ contain: "content", willChange: "transform" }}
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
          <p className="whitespace-pre-wrap">{message.content}</p>
          {message.cards && message.cards.length > 0 && (
            <div className="flex flex-col gap-2 mt-3">
              {message.cards.map((card, index) => (
                <ChatCardItem
                  key={`${message.id}_card_${card.type}_${index}`}
                  card={card}
                  onOpenReviewTab={onOpenReviewTab}
                  onApprove={card.type === "approval" ? () => onApprovalAction?.(message.id, "approve") : undefined}
                  onReject={card.type === "approval" ? () => onApprovalAction?.(message.id, "reject") : undefined}
                />
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

export function ChatPanel({ session, onSendMessage, onOpenReviewTab, onApprovalAction }: ChatPanelProps) {
  const [input, setInput] = useState("");
  const [selectedSkill, setSelectedSkill] = useState<SkillType | null>(null);
  const [showPipeline, setShowPipeline] = useState(false);
  const [devMockPipeline, setDevMockPipeline] = useState<PipelineRun | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const handleApprovalAction = useCallback(
    (messageId: string, action: "approve" | "reject") => {
      onApprovalAction?.(session.id, messageId, action);
    },
    [onApprovalAction, session.id],
  );

  useEffect(() => {
    if (import.meta.env.DEV) {
      import("@/__mocks__/pipelineModel").then((mod) => {
        setDevMockPipeline(mod.createMockPipelineRun());
      });
    }
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [session.messages]);

  function handleSend() {
    if (!input.trim()) return;
    if (onSendMessage) {
      onSendMessage(session.id, input, selectedSkill ?? undefined);
    }
    setInput("");
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  function toggleSkill(skillId: SkillType) {
    setSelectedSkill((prev) => (prev === skillId ? null : skillId));
  }

  const placeholder = selectedSkill === "pipeline-run"
    ? "输入 pipeline 意图..."
    : "输入消息...";

  const { containerRef, visibleItems, totalHeight, offsetTop } = useVirtualList(
    session.messages,
    80,
    3,
  );

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
          <button
            type="button"
            className="inline-flex items-center gap-1 border border-border rounded-md bg-card px-2 py-0.5 text-[11px] font-semibold text-muted-foreground hover:bg-muted transition-colors"
            onClick={() => setShowPipeline((prev) => !prev)}
            aria-expanded={showPipeline}
            aria-label="切换 Pipeline 面板"
          >
            <Zap size={11} />
            Pipeline
          </button>
        </div>
      </div>

      {showPipeline && import.meta.env.DEV && devMockPipeline && (
        <PipelinePanel
          pipeline={devMockPipeline}
          onTriggerDryRun={() => {
            /* TODO: wire to backend */
          }}
          onTriggerRun={() => {
            /* TODO: wire to backend */
          }}
          onApprove={() => {
            /* TODO: wire to backend */
          }}
          onCancel={() => {
            /* TODO: wire to backend */
          }}
        />
      )}
      {showPipeline && !import.meta.env.DEV && (
        <div className="flex items-center justify-center gap-2 border-b bg-background px-5 py-4 text-sm text-muted-foreground" data-testid="pipeline-panel-placeholder">
          <Zap size={14} />
          Pipeline 数据需要在生产环境中接入后端
        </div>
      )}

      <div
        ref={containerRef}
        className="flex-1 overflow-y-auto p-5 flex flex-col gap-4"
        aria-live="polite"
        aria-atomic="false"
      >
        <div style={{ height: totalHeight }}>
          <div style={{ paddingTop: offsetTop }}>
            {visibleItems.map((message) => (
              <ChatMessageItem
                key={message.id}
                message={message}
                onOpenReviewTab={onOpenReviewTab}
                onApprovalAction={handleApprovalAction}
              />
            ))}
          </div>
        </div>
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-composer">
        <div className="composer-shell">
          {/* Skill selector */}
          <div className="flex items-center gap-1.5 px-3 py-2 border-b overflow-x-auto">
            {SKILLS.map((skill) => {
              const isSelected = selectedSkill === skill.id;
              return (
                <button
                  key={skill.id}
                  type="button"
                  onClick={() => toggleSkill(skill.id)}
                  data-testid={`skill-btn-${skill.id}`}
                  className={cn(
                    "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-[11px] font-medium transition-colors border",
                    isSelected
                      ? "bg-primary text-primary-foreground border-primary"
                      : "bg-background text-muted-foreground border-border hover:bg-muted"
                  )}
                >
                  {SKILL_ICON[skill.id]}
                  <span>{skill.name}</span>
                </button>
              );
            })}
          </div>

          <div className="flex items-center justify-between gap-2 border-b px-3 py-2">
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Sparkles size={13} />
              <span>agent draft</span>
              {selectedSkill && (
                <Badge variant="outline" className="text-[10px] h-5 ml-1">
                  @{selectedSkill}
                </Badge>
              )}
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
              placeholder={placeholder}
              rows={2}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              data-testid="chat-input"
            />
            <Button
              size="icon"
              type="button"
              className="size-8 flex-shrink-0 rounded-lg"
              onClick={handleSend}
              aria-label="send"
              data-testid="chat-send-btn"
            >
              <Send data-icon="icon" />
            </Button>
          </div>
        </div>
      </div>
    </section>
  );
}
