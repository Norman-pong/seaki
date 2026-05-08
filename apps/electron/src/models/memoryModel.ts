export interface ReviewCardDTO {
  readonly cardId: string;
  readonly question: string;
  readonly answer: string;
  readonly source?: string;
  readonly stabilityDays: number;
  readonly nextReviewAt: string;
  readonly reviewCount: number;
  readonly difficulty: "easy" | "medium" | "hard" | "critical";
}

export interface ChannelConnectionDTO {
  readonly channelId: string;
  readonly provider: "feishu" | "slack" | "wecom";
  readonly name: string;
  readonly status: "connected" | "disconnected" | "error";
  readonly workspaceId: string;
  readonly webhookUrl?: string | undefined;
  readonly lastEventAt?: string;
}

export interface ChannelEventDTO {
  readonly eventId: string;
  readonly channelId: string;
  readonly eventType:
    | "message.received"
    | "message.sent"
    | "attachment.quarantined"
    | "error";
  readonly summary: string;
  readonly timestamp: string;
  readonly status: "success" | "failed" | "pending";
}

export function createMockMemoryCards(): readonly ReviewCardDTO[] {
  return [
    {
      cardId: "card_001",
      question: "Seaki 的 source ingest 范围限制是什么？",
      answer:
        "本机导入范围限制在当前 workspace 选择文件，不支持跨 workspace 导入。",
      source: "M0 本机导入 DecisionRecord",
      stabilityDays: 12,
      nextReviewAt: "2026-05-09T09:00:00+08:00",
      reviewCount: 3,
      difficulty: "easy",
    },
    {
      cardId: "card_002",
      question: "Pipeline 执行前需要哪些权限检查？",
      answer:
        "需要检查 actor 的 requiredCapabilities，包括 source.read、wiki.write、citation.validate、index.write 等。",
      source: "架构决策审查",
      stabilityDays: 5,
      nextReviewAt: "2026-05-08T14:00:00+08:00",
      reviewCount: 1,
      difficulty: "hard",
    },
    {
      cardId: "card_003",
      question: "citation 降级的处理流程是什么？",
      answer:
        "当 citation 状态为 degraded 时，需要人工确认降级原因，并在审批通过后应用 patch。",
      source: "Wiki 导入讨论",
      stabilityDays: 2,
      nextReviewAt: "2026-05-08T16:00:00+08:00",
      reviewCount: 0,
      difficulty: "critical",
    },
  ];
}

export function createMockChannels(): readonly ChannelConnectionDTO[] {
  return [
    {
      channelId: "ch_feishu_01",
      provider: "feishu",
      name: "Seaki 研发群",
      status: "connected",
      workspaceId: "ws_feishu_001",
      lastEventAt: "2026-05-08T12:30:00+08:00",
    },
    {
      channelId: "ch_slack_01",
      provider: "slack",
      name: "seaki-dev",
      status: "disconnected",
      workspaceId: "ws_slack_001",
      lastEventAt: "2026-05-07T18:00:00+08:00",
    },
  ];
}

export function createMockChannelEvents(): readonly ChannelEventDTO[] {
  return [
    {
      eventId: "evt_001",
      channelId: "ch_feishu_01",
      eventType: "message.received",
      summary: "收到来自 @alice 的消息：Pipeline 执行结果如何？",
      timestamp: "2026-05-08T12:30:00+08:00",
      status: "success",
    },
    {
      eventId: "evt_002",
      channelId: "ch_feishu_01",
      eventType: "message.sent",
      summary: "发送消息：Pipeline Wiki 导入与索引 Pipeline 正在运行中。",
      timestamp: "2026-05-08T12:28:00+08:00",
      status: "success",
    },
    {
      eventId: "evt_003",
      channelId: "ch_feishu_01",
      eventType: "attachment.quarantined",
      summary: "附件 report.pdf 因大小超限被隔离，等待审批。",
      timestamp: "2026-05-08T12:25:00+08:00",
      status: "pending",
    },
    {
      eventId: "evt_004",
      channelId: "ch_slack_01",
      eventType: "error",
      summary: "连接超时：无法连接到 slack workspace ws_slack_001",
      timestamp: "2026-05-07T18:00:00+08:00",
      status: "failed",
    },
    {
      eventId: "evt_005",
      channelId: "ch_feishu_01",
      eventType: "message.received",
      summary: "收到来自 @bob 的消息：请查看新的架构决策记录。",
      timestamp: "2026-05-08T11:00:00+08:00",
      status: "success",
    },
  ];
}
