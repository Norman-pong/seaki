export interface ChatCard {
  readonly type: "wiki" | "search" | "approval" | "citation" | "link";
  readonly title: string;
  readonly content?: string;
  readonly snippet?: string;
  readonly status?: string;
  readonly citationRefs?: readonly { id: string; label: string }[];
}

export interface ChatMessage {
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly content: string;
  readonly timestamp: string;
  readonly cards?: readonly ChatCard[];
}

export interface ChatSession {
  readonly id: string;
  readonly title: string;
  readonly timestamp: string;
  readonly active: boolean;
  readonly messages: readonly ChatMessage[];
}

const mockSessions: ChatSession[] = [
  {
    id: "session_1",
    title: "Wiki 导入讨论",
    timestamp: "2026-04-30T09:30:00+08:00",
    active: true,
    messages: [
      {
        id: "msg_1_1",
        role: "user",
        content: "帮我查看最近导入的 Markdown 资料状态",
        timestamp: "2026-04-30T09:30:00+08:00",
      },
      {
        id: "msg_1_2",
        role: "assistant",
        content:
          "当前 workspace 有两条导入记录。Markdown 文件已提交但索引需要重建，PDF 解析失败可重试。",
        timestamp: "2026-04-30T09:31:00+08:00",
        cards: [
          {
            type: "wiki",
            title: "M0 本机导入 DecisionRecord",
            content: "本机导入范围限制在当前 workspace 选择文件。",
            status: "committed",
            citationRefs: [
              { id: "cit_decision_context", label: "source scope" },
              { id: "cit_risk_boundary", label: "approval boundary" },
            ],
          },
        ],
      },
    ],
  },
  {
    id: "session_2",
    title: "架构决策审查",
    timestamp: "2026-04-29T16:45:00+08:00",
    active: false,
    messages: [
      {
        id: "msg_2_1",
        role: "user",
        content: "审查 pipeline dry-run 的结果",
        timestamp: "2026-04-29T16:45:00+08:00",
      },
      {
        id: "msg_2_2",
        role: "assistant",
        content:
          "Pipeline dry-run 已完成。包含 3 条命令：source ingest、wiki patch、search query。写入操作需要审批。",
        timestamp: "2026-04-29T16:46:00+08:00",
        cards: [
          {
            type: "approval",
            title: "Patch: patch_decision_record_import",
            content: "写入 typed wiki page，引用本机 source range；需要人工确认降级 citation。",
            status: "requires_approval",
          },
        ],
      },
    ],
  },
  {
    id: "session_3",
    title: "搜索结果整理",
    timestamp: "2026-04-28T11:20:00+08:00",
    active: false,
    messages: [
      {
        id: "msg_3_1",
        role: "user",
        content: "搜索 workspace source boundary 相关资料",
        timestamp: "2026-04-28T11:20:00+08:00",
      },
      {
        id: "msg_3_2",
        role: "assistant",
        content: "找到 2 条结果，其中 1 条因权限被过滤。",
        timestamp: "2026-04-28T11:21:00+08:00",
        cards: [
          {
            type: "search",
            title: "source scope",
            snippet: "本机导入范围限制在当前 workspace 选择文件。",
            status: "stale",
          },
          {
            type: "citation",
            title: "restricted source hidden",
            snippet: "权限不足，无法查看摘要。",
            status: "filtered",
          },
        ],
      },
    ],
  },
];

export function createChatSessions(): readonly ChatSession[] {
  return mockSessions;
}

export function createInitialSession(): ChatSession {
  const first = mockSessions[0];
  if (!first) {
    throw new Error("createInitialSession: mockSessions is empty");
  }
  return first;
}
