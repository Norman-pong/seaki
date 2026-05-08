import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { ChannelPanel } from "../ChannelPanel";
import type { ChannelConnectionDTO, ChannelEventDTO } from "@/models/memoryModel";

const mockChannels: readonly ChannelConnectionDTO[] = [
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

const mockEvents: readonly ChannelEventDTO[] = [
  {
    eventId: "evt_001",
    channelId: "ch_feishu_01",
    eventType: "message.received",
    summary: "收到来自 @alice 的消息",
    timestamp: "2026-05-08T12:30:00+08:00",
    status: "success",
  },
  {
    eventId: "evt_004",
    channelId: "ch_slack_01",
    eventType: "error",
    summary: "连接超时",
    timestamp: "2026-05-07T18:00:00+08:00",
    status: "failed",
  },
];

describe("ChannelPanel", () => {
  it("renders_channel_list", () => {
    render(
      <ChannelPanel
        channels={mockChannels}
        events={mockEvents}
        onToggleChannel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("channel-item-ch_feishu_01")).toBeInTheDocument();
    expect(screen.getByTestId("channel-item-ch_slack_01")).toBeInTheDocument();
    expect(screen.getByText("Seaki 研发群")).toBeInTheDocument();
    expect(screen.getByText("seaki-dev")).toBeInTheDocument();
  });

  it("shows_connection_status", () => {
    render(
      <ChannelPanel
        channels={mockChannels}
        events={mockEvents}
        onToggleChannel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("已连接")).toBeInTheDocument();
    expect(screen.getByText("未连接")).toBeInTheDocument();
  });

  it("renders_event_log", () => {
    render(
      <ChannelPanel
        channels={mockChannels}
        events={mockEvents}
        onToggleChannel={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("channel-event-evt_001")).toBeInTheDocument();
    expect(screen.getByTestId("channel-event-evt_004")).toBeInTheDocument();
    expect(screen.getByText("收到来自 @alice 的消息")).toBeInTheDocument();
  });

  it("toggles_channel_on_click", () => {
    const onToggleChannel = vi.fn<() => void>();
    render(
      <ChannelPanel
        channels={mockChannels}
        events={mockEvents}
        onToggleChannel={onToggleChannel}
      />,
    );

    fireEvent.click(screen.getByTestId("channel-toggle-ch_feishu_01"));
    expect(onToggleChannel).toHaveBeenCalledWith("ch_feishu_01");

    fireEvent.click(screen.getByTestId("channel-toggle-ch_slack_01"));
    expect(onToggleChannel).toHaveBeenCalledWith("ch_slack_01");
  });
});
