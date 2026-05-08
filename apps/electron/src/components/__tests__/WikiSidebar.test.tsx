import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { WikiSidebar } from "../WikiSidebar";
import { createWikiTree } from "@/models/wikiTreeModel";

const mockTree = createWikiTree();

describe("WikiSidebar", () => {
  it("renders_tabs", () => {
    render(
      <WikiSidebar
        tree={mockTree}
        selectedPageId="wiki_m0_ingest"
        onSelectPage={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("概览")).toBeInTheDocument();
    expect(screen.getByText("页面")).toBeInTheDocument();
    expect(screen.getByText("记忆")).toBeInTheDocument();
    expect(screen.getByText("频道")).toBeInTheDocument();
  });

  it("selects_page_on_click", () => {
    const onSelectPage = vi.fn<() => void>();
    render(
      <WikiSidebar
        tree={mockTree}
        selectedPageId="wiki_home"
        onSelectPage={onSelectPage}
        defaultActiveTab="pages"
      />,
    );

    fireEvent.click(screen.getByTestId("tree-select-wiki_architecture"));
    expect(onSelectPage).toHaveBeenCalledWith("wiki_architecture");
  });

  it("toggles_folder_expand_without_selecting", () => {
    const onSelectPage = vi.fn<() => void>();
    render(
      <WikiSidebar
        tree={mockTree}
        selectedPageId="wiki_home"
        onSelectPage={onSelectPage}
        defaultActiveTab="pages"
      />,
    );

    // Click chevron toggle on folder with children
    const toggle = screen.getByTestId("tree-toggle-wiki_projects");
    fireEvent.click(toggle);

    // onSelectPage should NOT be called when clicking toggle
    expect(onSelectPage).not.toHaveBeenCalled();
  });

  it("shows_selected_state", () => {
    render(
      <WikiSidebar
        tree={mockTree}
        selectedPageId="wiki_architecture"
        onSelectPage={vi.fn<() => void>()}
        defaultActiveTab="pages"
      />,
    );

    const selectedBtn = screen.getByTestId("tree-select-wiki_architecture");
    expect(selectedBtn).toHaveAttribute("aria-current", "true");
  });

  it("renders_memory_tab_with_cards", () => {
    render(
      <WikiSidebar
        tree={mockTree}
        selectedPageId="wiki_m0_ingest"
        onSelectPage={vi.fn<() => void>()}
        memoryCards={[
          {
            cardId: "card_001",
            question: "Q1",
            answer: "A1",
            stabilityDays: 5,
            nextReviewAt: "2026-05-09T09:00:00+08:00",
            reviewCount: 1,
            difficulty: "easy",
          },
        ]}
        onGradeCard={vi.fn<() => void>()}
        defaultActiveTab="memory"
      />,
    );

    expect(screen.getByText(/到期卡片/)).toBeInTheDocument();
  });

  it("renders_channel_tab", () => {
    render(
      <WikiSidebar
        tree={mockTree}
        selectedPageId="wiki_m0_ingest"
        onSelectPage={vi.fn<() => void>()}
        channels={[
          {
            channelId: "ch_01",
            provider: "feishu",
            name: "测试频道",
            status: "connected",
            workspaceId: "ws_01",
          },
        ]}
        channelEvents={[]}
        onToggleChannel={vi.fn<() => void>()}
        defaultActiveTab="channel"
      />,
    );

    expect(screen.getByText("测试频道")).toBeInTheDocument();
  });
});
