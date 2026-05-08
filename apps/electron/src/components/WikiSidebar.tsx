import { useState, useMemo, useEffect } from "react";
import {
  ChevronRight,
  ChevronDown,
  FileText,
  FolderOpen,
  Folder,
  Eye,
  ShieldCheck,
  Brain,
  MessageCircle,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import type { WikiTreeNode, WikiPagePreview } from "@/models/wikiTreeModel";
import { createWikiPreview } from "@/models/wikiTreeModel";
import type { ApprovalDiffModel } from "@/appModel";
import type { ReviewCardDTO, ChannelConnectionDTO, ChannelEventDTO } from "@/models/memoryModel";
import { TodoPanel } from "./TodoPanel";
import { ContextPanel } from "./ContextPanel";
import { ApprovalWidget } from "./ApprovalWidget";
import { MemoryReviewPanel } from "./MemoryReviewPanel";
import { ChannelPanel } from "./ChannelPanel";

interface WikiSidebarProps {
  readonly tree: readonly WikiTreeNode[];
  readonly selectedPageId: string;
  readonly onSelectPage: (pageId: string) => void;
  readonly approval?: ApprovalDiffModel | null;
  readonly onApprovalChange?: (model: ApprovalDiffModel) => void;
  readonly isCollapsed?: boolean;
  readonly memoryCards?: readonly ReviewCardDTO[];
  readonly onGradeCard?: (cardId: string, grade: string) => void;
  readonly channels?: readonly ChannelConnectionDTO[];
  readonly channelEvents?: readonly ChannelEventDTO[];
  readonly onToggleChannel?: (channelId: string) => void;
  readonly defaultActiveTab?: "overview" | "pages" | "review" | "memory" | "channel";
}

function TreeNodeItem({
  node,
  depth,
  onSelectPage,
  selectedPageId,
}: {
  readonly node: WikiTreeNode;
  readonly depth: number;
  readonly onSelectPage: (pageId: string) => void;
  readonly selectedPageId: string;
}) {
  const [expanded, setExpanded] = useState(node.expanded ?? false);
  useEffect(() => {
    setExpanded(node.expanded ?? false);
  }, [node.expanded]);
  const hasChildren = node.children.length > 0;
  const isSelected = node.id === selectedPageId;

  return (
    <div>
      <div
        className={cn(
          "tree-node-row w-full flex items-center gap-1 py-1.5 rounded-md text-sm",
        )}
        style={{ paddingLeft: `${8 + depth * 14}px` }}
      >
        {hasChildren ? (
          <button
            type="button"
            className="flex-shrink-0 p-0.5 rounded hover:bg-muted transition-colors"
            onClick={(e) => {
              e.stopPropagation();
              setExpanded(!expanded);
            }}
            aria-label={expanded ? "折叠" : "展开"}
            aria-expanded={expanded}
            data-testid={`tree-toggle-${node.id}`}
          >
            {expanded ? (
              <ChevronDown size={13} className="text-muted-foreground" />
            ) : (
              <ChevronRight size={13} className="text-muted-foreground" />
            )}
          </button>
        ) : (
          <span className="w-[21px] flex-shrink-0" />
        )}
        <button
          type="button"
          className={cn(
            "flex-1 flex items-center gap-1.5 py-1 px-1.5 rounded-md text-sm text-left transition-colors min-w-0",
            isSelected
              ? "bg-primary/10 text-primary"
              : "hover:bg-muted text-foreground"
          )}
          onClick={() => onSelectPage(node.id)}
          aria-current={isSelected ? "true" : undefined}
          data-testid={`tree-select-${node.id}`}
        >
          {hasChildren ? (
            expanded ? (
              <FolderOpen size={13} className="flex-shrink-0" />
            ) : (
              <Folder size={13} className="flex-shrink-0" />
            )
          ) : (
            <FileText size={13} className="flex-shrink-0" />
          )}
          <span className="truncate">{node.title}</span>
        </button>
      </div>
      {hasChildren && expanded && (
        <div>
          {node.children.map((child) => (
            <TreeNodeItem
              key={child.id}
              node={child}
              depth={depth + 1}
              onSelectPage={onSelectPage}
              selectedPageId={selectedPageId}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function WikiPageTree({
  nodes,
  selectedPageId,
  onSelectPage,
}: {
  readonly nodes: readonly WikiTreeNode[];
  readonly selectedPageId: string;
  readonly onSelectPage: (pageId: string) => void;
}) {
  return (
    <div className="px-3 py-2" aria-label="wiki page tree">
      <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide px-2 pb-1">
        页面
      </h3>
      {nodes.map((node) => (
        <TreeNodeItem
          key={node.id}
          node={node}
          depth={0}
          onSelectPage={onSelectPage}
          selectedPageId={selectedPageId}
        />
      ))}
    </div>
  );
}

function WikiPreview({ preview }: { readonly preview: WikiPagePreview }) {
  return (
    <Card size="sm" className="m-3 border-0 bg-transparent shadow-none" aria-label="wiki page preview">
      <CardHeader className="py-2">
        <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide">
          预览
        </h3>
      </CardHeader>
      <CardContent className="pt-0">
        <h4 className="text-sm font-semibold">{preview.title}</h4>
        <p className="text-[11px] text-muted-foreground font-mono mt-1">
          修订: {preview.revision}
        </p>
        <p className="text-sm text-muted-foreground leading-relaxed mt-2">
          {preview.content}
        </p>
        {preview.citations.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-3">
            {preview.citations.map((citation) => (
              <Badge key={citation.id} variant="secondary" className="text-xs h-5">
                {citation.label}
              </Badge>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export function WikiSidebar({
  tree,
  selectedPageId,
  onSelectPage,
  approval,
  onApprovalChange,
  isCollapsed,
  memoryCards,
  onGradeCard,
  channels,
  channelEvents,
  onToggleChannel,
  defaultActiveTab = "overview",
}: WikiSidebarProps) {
  const [activeTab, setActiveTab] = useState<"overview" | "pages" | "review" | "memory" | "channel">(defaultActiveTab);

  const preview = useMemo(() => createWikiPreview(selectedPageId), [selectedPageId]);

  return (
    <aside
      className={cn(
        "flex flex-col h-full sidebar-surface border-l overflow-hidden transition-transform duration-300 ease-out",
        isCollapsed && "translate-x-full"
      )}
      aria-label="wiki sidebar"
    >
      <Tabs
        value={activeTab}
        onValueChange={(v) => setActiveTab(v as typeof activeTab)}
        className="flex flex-col h-full"
      >
        <TabsList className="mx-3 mt-3 mb-1 h-7 bg-background/70 gap-1 p-1 self-start shadow-none">
          <TabsTrigger
            value="overview"
            className="text-[11px] px-2 py-0.5 h-6 gap-1"
            data-tab="overview"
          >
            <Eye size={11} /> 概览
          </TabsTrigger>
          <TabsTrigger
            value="pages"
            className="text-[11px] px-2 py-0.5 h-6 gap-1"
            data-tab="pages"
          >
            <FileText size={11} /> 页面
          </TabsTrigger>
          <TabsTrigger
            value="review"
            className="text-[11px] px-2 py-0.5 h-6 gap-1"
            disabled={!approval || !onApprovalChange}
            data-tab="review"
          >
            <ShieldCheck size={11} /> 审查
          </TabsTrigger>
          <TabsTrigger
            value="memory"
            className="text-[11px] px-2 py-0.5 h-6 gap-1"
            data-tab="memory"
          >
            <Brain size={11} /> 记忆
          </TabsTrigger>
          <TabsTrigger
            value="channel"
            className="text-[11px] px-2 py-0.5 h-6 gap-1"
            data-tab="channel"
          >
            <MessageCircle size={11} /> 频道
          </TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="flex-1 overflow-y-auto min-h-0 mt-0">
          <TodoPanel />
          <Separator className="mx-3 w-[calc(100%-1.5rem)]" />
          <ContextPanel />
        </TabsContent>
        <TabsContent value="pages" className="flex-1 overflow-y-auto min-h-0 mt-0">
          <WikiPageTree
            nodes={tree}
            selectedPageId={selectedPageId}
            onSelectPage={onSelectPage}
          />
          <Separator className="mx-3 w-[calc(100%-1.5rem)]" />
          <WikiPreview preview={preview} />
        </TabsContent>
        <TabsContent value="review" className="flex-1 overflow-y-auto min-h-0 mt-0">
          {approval && onApprovalChange ? (
            <ApprovalWidget model={approval} onChange={onApprovalChange} />
          ) : (
            <div className="flex items-center justify-center h-full text-sm text-muted-foreground">
              暂无审查内容
            </div>
          )}
        </TabsContent>
        <TabsContent value="memory" className="flex-1 overflow-y-auto min-h-0 mt-0">
          <MemoryReviewPanel
            dueCards={memoryCards ?? []}
            onGrade={(cardId, grade) => onGradeCard?.(cardId, grade)}
            onViewAll={() => {/* TODO: navigate to full memory view */}}
          />
        </TabsContent>
        <TabsContent value="channel" className="flex-1 overflow-y-auto min-h-0 mt-0">
          <ChannelPanel
            channels={channels ?? []}
            events={channelEvents ?? []}
            onToggleChannel={(channelId) => onToggleChannel?.(channelId)}
          />
        </TabsContent>
      </Tabs>
    </aside>
  );
}
