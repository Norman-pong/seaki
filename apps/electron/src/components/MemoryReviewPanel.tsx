import { useState, useEffect } from "react";
import {
  Brain,
  Eye,
  RotateCcw,
  ThumbsUp,
  CheckCircle,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import type { ReviewCardDTO } from "@/models/memoryModel";

interface MemoryReviewPanelProps {
  readonly dueCards: readonly ReviewCardDTO[];
  readonly onGrade: (cardId: string, grade: "again" | "hard" | "good" | "easy") => void;
  readonly onViewAll: () => void;
}

const DIFFICULTY_LABEL: Record<ReviewCardDTO["difficulty"], string> = {
  easy: "简单",
  medium: "中等",
  hard: "困难",
  critical: "危急",
};

const DIFFICULTY_VARIANT: Record<ReviewCardDTO["difficulty"], "default" | "secondary" | "destructive" | "outline"> = {
  easy: "secondary",
  medium: "outline",
  hard: "default",
  critical: "destructive",
};

export function MemoryReviewPanel({
  dueCards,
  onGrade,
  onViewAll,
}: MemoryReviewPanelProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);

  useEffect(() => {
    setCurrentIndex(0);
    setRevealed(false);
  }, [dueCards]);

  if (dueCards.length === 0 || currentIndex >= dueCards.length) {
    return (
      <div
        className="flex flex-col items-center justify-center h-full gap-3 text-sm text-muted-foreground"
        data-testid="memory-empty-state"
      >
        <Sparkles size={32} className="text-muted-foreground/50" />
        <span>暂无到期卡片 🎉</span>
      </div>
    );
  }

  const currentCard = dueCards[currentIndex]!;

  function handleReveal() {
    setRevealed(true);
  }

  function handleGrade(grade: "again" | "hard" | "good" | "easy") {
    onGrade(currentCard.cardId, grade);
    setRevealed(false);
    setCurrentIndex((prev) => prev + 1);
  }

  return (
    <div className="flex flex-col h-full p-3 gap-3" data-testid="memory-review-panel">
      <div className="sr-only" aria-live="polite" aria-atomic="true">
        第 {currentIndex + 1} 张卡片，共 {dueCards.length} 张
        {revealed ? "，答案已显示" : ""}
      </div>

      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Brain size={14} className="text-muted-foreground" />
          <span className="text-sm font-medium">
            到期卡片: {dueCards.length - currentIndex} 张
          </span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs"
          onClick={onViewAll}
          data-testid="memory-view-all-btn"
        >
          全部记忆
        </Button>
      </div>

      <Card className="flex-1 flex flex-col shadow-none" data-testid="memory-current-card">
        <CardHeader className="py-3 flex flex-row items-center justify-between gap-2">
          <Badge variant={DIFFICULTY_VARIANT[currentCard.difficulty]} className="text-[10px] h-5">
            {DIFFICULTY_LABEL[currentCard.difficulty]}
          </Badge>
          <span className="text-[11px] text-muted-foreground font-mono">
            {currentCard.cardId}
          </span>
        </CardHeader>
        <CardContent className="flex-1 flex flex-col gap-3 pt-0">
          <div className="flex-1 flex flex-col justify-center">
            <p className="text-sm font-medium leading-relaxed">
              {currentCard.question}
            </p>
            {revealed && (
              <div
                className="mt-4 p-3 rounded-lg bg-muted/60"
                data-testid="memory-answer"
                aria-live="polite"
                aria-atomic="true"
              >
                <p className="text-sm text-muted-foreground leading-relaxed">
                  {currentCard.answer}
                </p>
              </div>
            )}
          </div>

          {!revealed ? (
            <Button
              variant="outline"
              className="w-full h-9 text-sm"
              onClick={handleReveal}
              data-testid="memory-reveal-btn"
            >
              <Eye size={14} className="mr-1.5" />
              显示答案
            </Button>
          ) : (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2">
                <Button
                  variant="destructive"
                  size="sm"
                  className="flex-1 h-8 text-xs"
                  onClick={() => handleGrade("again")}
                  data-testid="memory-grade-again"
                >
                  <RotateCcw size={12} className="mr-1" />
                  Again
                </Button>
                <Button
                  variant="default"
                  size="sm"
                  className="flex-1 h-8 text-xs bg-orange-500 hover:bg-orange-600"
                  onClick={() => handleGrade("hard")}
                  data-testid="memory-grade-hard"
                >
                  <ThumbsUp size={12} className="mr-1" />
                  Hard
                </Button>
                <Button
                  variant="default"
                  size="sm"
                  className="flex-1 h-8 text-xs"
                  onClick={() => handleGrade("good")}
                  data-testid="memory-grade-good"
                >
                  <CheckCircle size={12} className="mr-1" />
                  Good
                </Button>
                <Button
                  variant="default"
                  size="sm"
                  className="flex-1 h-8 text-xs bg-green-600 hover:bg-green-700"
                  onClick={() => handleGrade("easy")}
                  data-testid="memory-grade-easy"
                >
                  <Sparkles size={12} className="mr-1" />
                  Easy
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
        <span className="bg-muted px-2 py-0.5 rounded">
          稳定度: {currentCard.stabilityDays} 天
        </span>
        <span className="bg-muted px-2 py-0.5 rounded">
          复习次数: {currentCard.reviewCount}
        </span>
        {currentCard.source && (
          <span className="bg-muted px-2 py-0.5 rounded truncate">
            来源: {currentCard.source}
          </span>
        )}
      </div>
    </div>
  );
}
