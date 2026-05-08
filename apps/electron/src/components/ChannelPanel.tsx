import { useMemo, useState } from "react";
import {
  MessageCircle,
  Hash,
  Building,
  CheckCircle,
  AlertCircle,
  Loader2,
  ChevronDown,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type {
  ChannelConnectionDTO,
  ChannelEventDTO,
} from "@/models/memoryModel";

interface ChannelPanelProps {
  readonly channels: readonly ChannelConnectionDTO[];
  readonly events: readonly ChannelEventDTO[];
  readonly onToggleChannel: (channelId: string) => void;
}

const PROVIDER_ICON: Record<
  ChannelConnectionDTO["provider"],
  React.ReactNode
> = {
  feishu: <MessageCircle size={14} />,
  slack: <Hash size={14} />,
  wecom: <Building size={14} />,
};

const STATUS_ICON: Record<ChannelEventDTO["status"], React.ReactNode> = {
  success: <CheckCircle size={12} className="text-green-500" />,
  failed: <AlertCircle size={12} className="text-red-500" />,
  pending: <Loader2 size={12} className="animate-spin text-yellow-500" />,
};

const STATUS_LABEL: Record<ChannelConnectionDTO["status"], string> = {
  connected: "已连接",
  disconnected: "未连接",
  error: "错误",
};

const STATUS_VARIANT: Record<
  ChannelConnectionDTO["status"],
  "default" | "secondary" | "destructive" | "outline"
> = {
  connected: "secondary",
  disconnected: "outline",
  error: "destructive",
};

const EVENT_TYPE_LABEL: Record<ChannelEventDTO["eventType"], string> = {
  "message.received": "收到消息",
  "message.sent": "发送消息",
  "attachment.quarantined": "附件隔离",
  error: "错误",
};

function ChannelItem({
  channel,
  onToggle,
}: {
  readonly channel: ChannelConnectionDTO;
  readonly onToggle: (channelId: string) => void;
}) {
  return (
    <div
      className="flex items-center justify-between gap-2 px-2 py-2 rounded-md hover:bg-muted/60 transition-colors"
      data-testid={`channel-item-${channel.channelId}`}
    >
      <div className="flex items-center gap-2 min-w-0">
        <span className="text-muted-foreground flex-shrink-0">
          {PROVIDER_ICON[channel.provider]}
        </span>
        <span className="text-sm truncate">{channel.name}</span>
      </div>
      <div className="flex items-center gap-2 flex-shrink-0">
        <Badge
          variant={STATUS_VARIANT[channel.status]}
          className="text-[10px] h-5 px-1.5"
        >
          {STATUS_LABEL[channel.status]}
        </Badge>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs px-2"
          onClick={() => onToggle(channel.channelId)}
          data-testid={`channel-toggle-${channel.channelId}`}
        >
          {channel.status === "connected" ? "断开" : "连接"}
        </Button>
      </div>
    </div>
  );
}

function EventItem({ event }: { readonly event: ChannelEventDTO }) {
  const time = new Date(event.timestamp).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  return (
    <div
      className="flex items-start gap-2 px-2 py-1.5 rounded-md hover:bg-muted/40 transition-colors"
      data-testid={`channel-event-${event.eventId}`}
    >
      <span className="flex-shrink-0 mt-0.5">{STATUS_ICON[event.status]}</span>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="text-[11px] font-mono text-muted-foreground">
            {time}
          </span>
          <span className="text-[11px] text-muted-foreground">
            {EVENT_TYPE_LABEL[event.eventType]}
          </span>
        </div>
        <p className="text-xs text-foreground leading-relaxed truncate">
          {event.summary}
        </p>
      </div>
    </div>
  );
}

export function ChannelPanel({
  channels,
  events,
  onToggleChannel,
}: ChannelPanelProps) {
  const [visibleCount, setVisibleCount] = useState(50);

  const sortedEvents = useMemo(() => {
    return [...events].sort(
      (a, b) =>
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
    );
  }, [events]);

  const displayedEvents = sortedEvents.slice(0, visibleCount);
  const hasMore = sortedEvents.length > visibleCount;

  return (
    <div className="flex flex-col h-full" data-testid="channel-panel">
      {/* Channel List */}
      <div className="px-3 py-2">
        <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide px-2 pb-1">
          频道
        </h3>
        <div className="flex flex-col gap-0.5">
          {channels.map((channel) => (
            <ChannelItem
              key={channel.channelId}
              channel={channel}
              onToggle={onToggleChannel}
            />
          ))}
        </div>
      </div>

      {/* Event Log */}
      <div className="flex-1 flex flex-col min-h-0 border-t">
        <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide px-5 pt-2 pb-1">
          事件日志
        </h3>
        <div className="flex-1 overflow-y-auto px-3 pb-2">
          <div className="flex flex-col gap-0.5">
            {displayedEvents.map((event) => (
              <EventItem key={event.eventId} event={event} />
            ))}
          </div>
          {hasMore && (
            <Button
              variant="ghost"
              size="sm"
              className="w-full h-7 text-xs mt-2"
              onClick={() => setVisibleCount((prev) => prev + 50)}
              data-testid="channel-load-more"
            >
              <ChevronDown size={12} className="mr-1" />
              加载更多
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
