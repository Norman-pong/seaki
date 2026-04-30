export interface WikiTreeNode {
  readonly id: string;
  readonly title: string;
  readonly children: readonly WikiTreeNode[];
  readonly expanded?: boolean;
}

const mockWikiTree: WikiTreeNode[] = [
  {
    id: "wiki_home",
    title: "首页",
    expanded: true,
    children: [
      {
        id: "wiki_projects",
        title: "项目",
        expanded: true,
        children: [
          {
            id: "wiki_architecture",
            title: "架构设计",
            children: [],
          },
          {
            id: "wiki_decisions",
            title: "决策记录",
            expanded: true,
            children: [
              {
                id: "wiki_m0_ingest",
                title: "M0 本机导入",
                children: [],
              },
            ],
          },
        ],
      },
      {
        id: "wiki_references",
        title: "参考资料",
        children: [
          {
            id: "wiki_source_scope",
            title: "Source 范围",
            children: [],
          },
        ],
      },
      {
        id: "wiki_meetings",
        title: "会议纪要",
        children: [],
      },
    ],
  },
];

export function createWikiTree(): readonly WikiTreeNode[] {
  return mockWikiTree;
}

export interface WikiPagePreview {
  readonly pageId: string;
  readonly title: string;
  readonly content: string;
  readonly revision: string;
  readonly citations: readonly { id: string; label: string }[];
}

export function createWikiPreview(pageId: string = "wiki_m0_ingest"): WikiPagePreview {
  const previews: Record<string, WikiPagePreview> = {
    wiki_m0_ingest: {
      pageId: "wiki_m0_ingest",
      title: "M0 本机导入 DecisionRecord",
      content:
        "本次导入只覆盖 workspace 内选择的 Markdown 资料。PDF 文件因 parser 限制暂时未能完整索引。",
      revision: "wiki_rev_0",
      citations: [
        { id: "cit_decision_context", label: "L12-L18" },
        { id: "cit_risk_boundary", label: "L28-L35" },
      ],
    },
    wiki_architecture: {
      pageId: "wiki_architecture",
      title: "架构设计",
      content: "seaki 采用 Clean Architecture / Ports & Adapters 模式，Electron 为 MVP 首版桌面主线客户端。",
      revision: "wiki_rev_1",
      citations: [{ id: "cit_arch", label: "overview.md" }],
    },
  };

  return previews[pageId] ?? previews["wiki_m0_ingest"]!;
}
