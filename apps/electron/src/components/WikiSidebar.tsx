import { useState, useMemo } from "react";
import { ChevronRight, ChevronDown, FileText, FolderOpen, Folder } from "lucide-react";
import type { WikiTreeNode, WikiPagePreview } from "@/models/wikiTreeModel";
import { createWikiPreview } from "@/models/wikiTreeModel";
import { Badge } from "@/components/ui/badge";
import { TodoPanel } from "./TodoPanel";
import { ContextPanel } from "./ContextPanel";

interface WikiPageTreeProps {
  readonly nodes: readonly WikiTreeNode[];
  readonly onSelectPage: (pageId: string) => void;
  readonly selectedPageId: string;
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
  const hasChildren = node.children.length > 0;
  const isSelected = node.id === selectedPageId;

  return (
    <div className="tree-node">
      <button
        type="button"
        className={`tree-node-row ${isSelected ? "selected" : ""}`}
        style={{ paddingLeft: `${12 + depth * 16}px` }}
        aria-expanded={hasChildren ? expanded : undefined}
        onClick={() => {
          if (hasChildren) {
            setExpanded(!expanded);
          }
          onSelectPage(node.id);
        }}
      >
        {hasChildren ? (
          expanded ? (
            <ChevronDown size={14} className="tree-chevron" />
          ) : (
            <ChevronRight size={14} className="tree-chevron" />
          )
        ) : (
          <span className="tree-chevron-placeholder" />
        )}
        {hasChildren ? (
          expanded ? (
            <FolderOpen size={14} className="tree-icon" />
          ) : (
            <Folder size={14} className="tree-icon" />
          )
        ) : (
          <FileText size={14} className="tree-icon" />
        )}
        <span className="tree-label">{node.title}</span>
      </button>
      {hasChildren && expanded ? (
        <div className="tree-children">
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
      ) : null}
    </div>
  );
}

function WikiPageTree({ nodes, onSelectPage, selectedPageId }: WikiPageTreeProps) {
  return (
    <div className="wiki-tree" aria-label="wiki page tree">
      <div className="wiki-panel-header">
        <h3 className="wiki-panel-title">页面</h3>
      </div>
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
    <div className="wiki-preview" aria-label="wiki page preview">
      <div className="wiki-panel-header">
        <h3 className="wiki-panel-title">预览</h3>
      </div>
      <div className="wiki-preview-content">
        <h4 className="wiki-preview-title">{preview.title}</h4>
        <p className="wiki-preview-revision">修订: {preview.revision}</p>
        <p className="wiki-preview-body">{preview.content}</p>
        {preview.citations.length > 0 ? (
          <div className="wiki-preview-citations">
            {preview.citations.map((citation) => (
              <Badge key={citation.id} variant="secondary" className="citation-chip">
                {citation.label}
              </Badge>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

interface WikiSidebarProps {
  readonly tree: readonly WikiTreeNode[];
  readonly selectedPageId: string;
  readonly onSelectPage: (pageId: string) => void;
}

export function WikiSidebar({ tree, selectedPageId, onSelectPage }: WikiSidebarProps) {
  const preview = useMemo(() => createWikiPreview(selectedPageId), [selectedPageId]);

  return (
    <aside className="wiki-sidebar" aria-label="wiki sidebar">
      <TodoPanel />
      <div className="wiki-sidebar-divider" />
      <ContextPanel />
      <div className="wiki-sidebar-divider" />
      <WikiPageTree
        nodes={tree}
        onSelectPage={onSelectPage}
        selectedPageId={selectedPageId}
      />
      <div className="wiki-sidebar-divider" />
      <WikiPreview preview={preview} />
    </aside>
  );
}
