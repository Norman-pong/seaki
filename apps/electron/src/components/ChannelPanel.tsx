import { useMemo, useState } from "react";
import {
  MessageCircle,
  Hash,
  Building,
  CheckCircle,
  AlertCircle,
  Loader2,
  ChevronDown,
  Plus,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import type {
  ChannelConnectionDTO,
  ChannelEventDTO,
} from "@/models/memoryModel";

interface ChannelPanelProps {
  readonly channels: readonly ChannelConnectionDTO[];
  readonly events: readonly ChannelEventDTO[];
  readonly onToggleChannel: (channelId: string) => void;
  readonly onAddChannel?: (channel: Omit<ChannelConnectionDTO, "channelId" | "status">) => void;
  readonly onEditChannel?: (channel: ChannelConnectionDTO) => void;
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

type FormMode = "closed" | "add" | "edit";

interface FormErrors {
  name?: string;
  provider?: string;
  webhookUrl?: string;
  workspaceId?: string;
}

function isValidUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

function ChannelItem({
  channel,
  onToggle,
  onEdit,
}: {
  readonly channel: ChannelConnectionDTO;
  readonly onToggle: (channelId: string) => void;
  readonly onEdit: (channel: ChannelConnectionDTO) => void;
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
        <button
          type="button"
          className="text-sm truncate text-left hover:underline"
          onClick={() => onEdit(channel)}
          data-testid={`channel-edit-${channel.channelId}`}
        >
          {channel.name}
        </button>
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
  onAddChannel,
  onEditChannel,
}: ChannelPanelProps) {
  const [visibleCount, setVisibleCount] = useState(50);
  const [formMode, setFormMode] = useState<FormMode>("closed");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [formName, setFormName] = useState("");
  const [formProvider, setFormProvider] = useState<ChannelConnectionDTO["provider"]> ("feishu");
  const [formWebhookUrl, setFormWebhookUrl] = useState("");
  const [formWorkspaceId, setFormWorkspaceId] = useState("");
  const [errors, setErrors] = useState<FormErrors>({});

  const sortedEvents = useMemo(() => {
    return [...events].sort(
      (a, b) =>
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
    );
  }, [events]);

  const displayedEvents = sortedEvents.slice(0, visibleCount);
  const hasMore = sortedEvents.length > visibleCount;

  function resetForm() {
    setFormName("");
    setFormProvider("feishu");
    setFormWebhookUrl("");
    setFormWorkspaceId("");
    setErrors({});
    setEditingId(null);
    setFormMode("closed");
  }

  function openAddForm() {
    resetForm();
    setFormMode("add");
  }

  function openEditForm(channel: ChannelConnectionDTO) {
    setFormMode("edit");
    setEditingId(channel.channelId);
    setFormName(channel.name);
    setFormProvider(channel.provider);
    setFormWebhookUrl(channel.webhookUrl ?? "");
    setFormWorkspaceId(channel.workspaceId);
    setErrors({});
  }

  function validate(): boolean {
    const nextErrors: FormErrors = {};
    if (!formName.trim()) {
      nextErrors.name = "频道名称不能为空";
    }
    if (!formWorkspaceId.trim()) {
      nextErrors.workspaceId = "Workspace ID 不能为空";
    }
    if (formWebhookUrl.trim() && !isValidUrl(formWebhookUrl.trim())) {
      nextErrors.webhookUrl = "Webhook URL 格式不正确";
    }
    setErrors(nextErrors);
    return Object.keys(nextErrors).length === 0;
  }

  function handleSubmit() {
    if (!validate()) return;

    if (formMode === "add" && onAddChannel) {
      onAddChannel({
        name: formName.trim(),
        provider: formProvider,
        webhookUrl: formWebhookUrl.trim(),
        workspaceId: formWorkspaceId.trim(),
      });
    } else if (formMode === "edit" && onEditChannel && editingId) {
      const existing = channels.find((c) => c.channelId === editingId);
      if (existing) {
        onEditChannel({
          ...existing,
          name: formName.trim(),
          provider: formProvider,
          webhookUrl: formWebhookUrl.trim(),
          workspaceId: formWorkspaceId.trim(),
        });
      }
    }

    resetForm();
  }

  const showForm = formMode === "add" || formMode === "edit";

  return (
    <div className="flex flex-col h-full" data-testid="channel-panel">
      {/* Channel List */}
      <div className="px-3 py-2">
        <div className="flex items-center justify-between px-2 pb-1">
          <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide">
            频道
          </h3>
          {onAddChannel && (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 text-[11px] px-1.5"
              onClick={openAddForm}
              data-testid="channel-add-btn"
            >
              <Plus size={12} className="mr-0.5" />
              添加
            </Button>
          )}
        </div>

        {showForm && (
          <Card className="mb-2 shadow-none" data-testid="channel-form">
            <CardContent className="py-3 px-3 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">
                  {formMode === "add" ? "添加频道" : "编辑频道"}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 w-6 p-0"
                  onClick={resetForm}
                  data-testid="channel-form-close"
                >
                  <X size={12} />
                </Button>
              </div>

              <div className="space-y-1">
                <label htmlFor="channel-form-name" className="text-xs text-muted-foreground">名称 *</label>
                <input
                  type="text"
                  className="w-full h-8 px-2 text-sm rounded-md border bg-background outline-none focus:ring-1 focus:ring-ring"
                  value={formName}
                  onChange={(e) => setFormName(e.target.value)}
                  data-testid="channel-form-name"
                  placeholder="频道名称"
                />
                {errors.name && (
                  <p className="text-[11px] text-destructive" data-testid="channel-form-name-error">
                    {errors.name}
                  </p>
                )}
              </div>

              <div className="space-y-1">
                <label htmlFor="channel-form-provider" className="text-xs text-muted-foreground">类型 *</label>
                <select
                  className="w-full h-8 px-2 text-sm rounded-md border bg-background outline-none focus:ring-1 focus:ring-ring"
                  value={formProvider}
                  onChange={(e) => setFormProvider(e.target.value as ChannelConnectionDTO["provider"])}
                  data-testid="channel-form-provider"
                >
                  <option value="feishu">飞书</option>
                  <option value="slack">Slack</option>
                  <option value="wecom">企业微信</option>
                </select>
              </div>

              <div className="space-y-1">
                <label htmlFor="channel-form-workspace-id" className="text-xs text-muted-foreground">Workspace ID *</label>
                <input
                  type="text"
                  className="w-full h-8 px-2 text-sm rounded-md border bg-background outline-none focus:ring-1 focus:ring-ring"
                  value={formWorkspaceId}
                  onChange={(e) => setFormWorkspaceId(e.target.value)}
                  data-testid="channel-form-workspace-id"
                  placeholder="workspace_001"
                />
                {errors.workspaceId && (
                  <p className="text-[11px] text-destructive" data-testid="channel-form-workspace-id-error">
                    {errors.workspaceId}
                  </p>
                )}
              </div>

              <div className="space-y-1">
                <label htmlFor="channel-form-webhook-url" className="text-xs text-muted-foreground">Webhook URL</label>
                <input
                  type="text"
                  className="w-full h-8 px-2 text-sm rounded-md border bg-background outline-none focus:ring-1 focus:ring-ring"
                  value={formWebhookUrl}
                  onChange={(e) => setFormWebhookUrl(e.target.value)}
                  data-testid="channel-form-webhook-url"
                  placeholder="https://hooks.example.com/..."
                />
                {errors.webhookUrl && (
                  <p className="text-[11px] text-destructive" data-testid="channel-form-webhook-url-error">
                    {errors.webhookUrl}
                  </p>
                )}
              </div>

              <Button
                size="sm"
                className="w-full h-8 text-xs"
                onClick={handleSubmit}
                data-testid="channel-form-submit"
              >
                {formMode === "add" ? "添加" : "保存"}
              </Button>
            </CardContent>
          </Card>
        )}

        <div className="flex flex-col gap-0.5">
          {channels.map((channel) => (
            <ChannelItem
              key={channel.channelId}
              channel={channel}
              onToggle={onToggleChannel}
              onEdit={openEditForm}
            />
          ))}
        </div>
      </div>

      {/* Event Log */}
      <div className="flex-1 flex flex-col min-h-0 border-t">
        <h3 className="text-[11px] font-bold text-muted-foreground uppercase tracking-wide px-5 pt-2 pb-1">
          事件日志
        </h3>
        <div
          className="flex-1 overflow-y-auto px-3 pb-2"
          aria-live="polite"
          aria-atomic="false"
        >
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
