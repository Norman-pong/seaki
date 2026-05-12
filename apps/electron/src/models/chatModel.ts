export type ChatCardType =
  | "wiki"
  | "search"
  | "approval"
  | "citation"
  | "link"
  | "pipeline"
  | "skill";

export type SkillType =
  | "wiki-search"
  | "source-ingest"
  | "pipeline-run"
  | "memory-review"
  | "channel-send";

export interface SkillOption {
  readonly id: SkillType;
  readonly name: string;
  readonly description: string;
  readonly icon: string; // lucide icon name
}

export const SKILLS: readonly SkillOption[] = [
  {
    id: "wiki-search",
    name: "Wiki 搜索",
    description: "搜索已提交的 wiki 页面",
    icon: "Search",
  },
  {
    id: "source-ingest",
    name: "资料导入",
    description: "导入本机资料到 workspace",
    icon: "FilePlus",
  },
  {
    id: "pipeline-run",
    name: "Pipeline",
    description: "运行 pipeline 执行任务",
    icon: "Zap",
  },
  {
    id: "memory-review",
    name: "记忆复习",
    description: "查看和复习记忆卡片",
    icon: "Brain",
  },
  {
    id: "channel-send",
    name: "频道发送",
    description: "通过 IM 频道发送消息",
    icon: "Send",
  },
];

export interface CitationRef {
  readonly id: string;
  readonly label: string;
  readonly sourceId?: string;
  readonly citationId?: string;
  readonly previewTarget?: "source_range" | "wiki_anchor" | "none";
}

export interface ChatCard {
  readonly type: ChatCardType;
  readonly title: string;
  readonly content?: string;
  readonly snippet?: string;
  readonly status?: string;
  readonly citationRefs?: readonly CitationRef[];
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
  readonly messages: readonly ChatMessage[];
}

const mockSessions: ChatSession[] = [
  {
    id: "session_1",
    title: "Wiki 导入讨论",
    timestamp: "2026-04-30T09:30:00+08:00",
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
            content:
              "写入 typed wiki page，引用本机 source range；需要人工确认降级 citation。",
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
  {
    id: "session_4",
    title: "Pipeline 执行",
    timestamp: "2026-05-08T10:00:00+08:00",
    messages: [
      {
        id: "msg_4_1",
        role: "user",
        content: "执行 wiki 导入 pipeline",
        timestamp: "2026-05-08T10:00:00+08:00",
      },
      {
        id: "msg_4_2",
        role: "assistant",
        content:
          "已启动 Pipeline: Wiki 导入与索引 Pipeline。当前正在重建索引。",
        timestamp: "2026-05-08T10:00:01+08:00",
        cards: [
          {
            type: "pipeline",
            title: "Wiki 导入与索引 Pipeline",
            content:
              "包含 4 个步骤：source ingest、wiki patch、index rebuild、approval request",
            status: "running",
          },
        ],
      },
    ],
  },
  {
    id: "session_5",
    title: "Skill 使用示例",
    timestamp: "2026-05-08T11:00:00+08:00",
    messages: [
      {
        id: "msg_5_1",
        role: "user",
        content: "[@wiki-search] 搜索架构决策相关 wiki",
        timestamp: "2026-05-08T11:00:00+08:00",
      },
      {
        id: "msg_5_2",
        role: "assistant",
        content: "已使用 Wiki 搜索 skill，找到 3 条相关 wiki 页面。",
        timestamp: "2026-05-08T11:00:01+08:00",
        cards: [
          {
            type: "skill",
            title: "wiki-search",
            content: "搜索关键词：架构决策",
            status: "completed",
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

/** Whether LLM mode (non-mock) is enabled */
export function isLlmEnabled(): boolean {
  return (
    typeof import.meta.env.SEAKI_LLM_ENABLED === "string" &&
    import.meta.env.SEAKI_LLM_ENABLED !== "false"
  );
}
