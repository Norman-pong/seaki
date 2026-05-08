import { useState, useEffect, useRef, useCallback } from "react";

interface VirtualListState<T> {
  readonly containerRef: React.RefObject<HTMLDivElement | null>;
  readonly visibleItems: readonly T[];
  readonly totalHeight: number;
  readonly offsetTop: number;
  readonly startIndex: number;
}

export function useVirtualList<T>(
  items: readonly T[],
  itemHeight: number,
  overscan = 5,
): VirtualListState<T> {
  const containerRef = useRef<HTMLDivElement>(null);
  const [range, setRange] = useState({ start: 0, end: items.length });

  const updateRange = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const scrollTop = el.scrollTop;
    const clientHeight = el.clientHeight;

    // In test environments (jsdom) clientHeight may be 0; render everything.
    if (clientHeight === 0) {
      setRange((prev) =>
        prev.start === 0 && prev.end === items.length ? prev : { start: 0, end: items.length },
      );
      return;
    }

    const start = Math.max(0, Math.floor(scrollTop / itemHeight) - overscan);
    const end = Math.min(
      items.length,
      Math.ceil((scrollTop + clientHeight) / itemHeight) + overscan,
    );
    setRange((prev) => (prev.start === start && prev.end === end ? prev : { start, end }));
  }, [items.length, itemHeight, overscan]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener("scroll", updateRange, { passive: true });
    updateRange();
    return () => el.removeEventListener("scroll", updateRange);
  }, [updateRange]);

  useEffect(() => {
    updateRange();
  }, [items.length, updateRange]);

  const visibleItems = items.slice(range.start, range.end);
  const totalHeight = items.length * itemHeight;
  const offsetTop = range.start * itemHeight;

  return { containerRef, visibleItems, totalHeight, offsetTop, startIndex: range.start };
}
