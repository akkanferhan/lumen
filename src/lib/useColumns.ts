import { useLayoutEffect, useState, type RefObject } from "react";

/** Number of columns a CSS grid actually rendered, kept in sync on resize. */
export function useColumns(ref: RefObject<HTMLElement | null>, fallback = 4) {
  const [columns, setColumns] = useState(fallback);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setColumns(getComputedStyle(el).gridTemplateColumns.split(" ").length || 1);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return columns;
}
