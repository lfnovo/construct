export function moveQuickOpenSelection(
  current: number,
  resultCount: number,
  direction: "next" | "previous",
) {
  if (resultCount <= 0) return 0;
  const normalized = Math.max(0, Math.min(current, resultCount - 1));
  return direction === "next"
    ? (normalized + 1) % resultCount
    : (normalized - 1 + resultCount) % resultCount;
}
