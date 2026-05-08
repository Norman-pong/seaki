import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { MemoryReviewPanel } from "../MemoryReviewPanel";
import type { ReviewCardDTO } from "@/models/memoryModel";

const mockCards: readonly ReviewCardDTO[] = [
  {
    cardId: "card_001",
    question: "Seaki 的 source ingest 范围限制是什么？",
    answer: "本机导入范围限制在当前 workspace 选择文件。",
    source: "M0 本机导入 DecisionRecord",
    stabilityDays: 12,
    nextReviewAt: "2026-05-09T09:00:00+08:00",
    reviewCount: 3,
    difficulty: "easy",
  },
  {
    cardId: "card_002",
    question: "Pipeline 执行前需要哪些权限检查？",
    answer: "需要检查 actor 的 requiredCapabilities。",
    source: "架构决策审查",
    stabilityDays: 5,
    nextReviewAt: "2026-05-08T14:00:00+08:00",
    reviewCount: 1,
    difficulty: "hard",
  },
];

describe("MemoryReviewPanel", () => {
  it("renders_due_cards_count", () => {
    render(
      <MemoryReviewPanel
        dueCards={mockCards}
        onGrade={vi.fn<() => void>()}
        onViewAll={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText("到期卡片: 2 张")).toBeInTheDocument();
  });

  it("shows_question_and_reveal_answer", () => {
    render(
      <MemoryReviewPanel
        dueCards={mockCards}
        onGrade={vi.fn<() => void>()}
        onViewAll={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByText(mockCards[0]!.question)).toBeInTheDocument();
    expect(screen.queryByTestId("memory-answer")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("memory-reveal-btn"));

    expect(screen.getByTestId("memory-answer")).toBeInTheDocument();
    expect(screen.getByText(mockCards[0]!.answer)).toBeInTheDocument();
  });

  it("grades_card_on_button_click", () => {
    const onGrade = vi.fn<() => void>();
    render(
      <MemoryReviewPanel
        dueCards={mockCards}
        onGrade={onGrade}
        onViewAll={vi.fn<() => void>()}
      />,
    );

    fireEvent.click(screen.getByTestId("memory-reveal-btn"));
    fireEvent.click(screen.getByTestId("memory-grade-good"));

    expect(onGrade).toHaveBeenCalledWith("card_001", "good");
  });

  it("shows_empty_state_when_no_cards", () => {
    render(
      <MemoryReviewPanel
        dueCards={[]}
        onGrade={vi.fn<() => void>()}
        onViewAll={vi.fn<() => void>()}
      />,
    );

    expect(screen.getByTestId("memory-empty-state")).toBeInTheDocument();
    expect(screen.getByText("暂无到期卡片 🎉")).toBeInTheDocument();
  });
});
