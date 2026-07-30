import type { TabMode } from "./types";

export type DocumentPositionAnchor = {
  quote: string;
  progress: number;
};

export type DocumentViewState = {
  scrollTop: number;
  ratio: number;
  anchor: DocumentPositionAnchor | null;
};

export type DocumentModeTransfer = {
  targetMode: TabMode;
  anchor: DocumentPositionAnchor | null;
  ratio: number;
};

export function normalizePositionText(value: string) {
  return value
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^#{1,6}\s+/, "")
    .replace(/^>\s+/, "")
    .replace(/^(?:[-*+]|\d+[.)])\s+/, "")
    .replace(/[*_~`]/g, "")
    .trim();
}

export function scrollRatio(scrollTop: number, scrollHeight: number, clientHeight: number) {
  const maximum = Math.max(0, scrollHeight - clientHeight);
  return maximum ? Math.max(0, Math.min(1, scrollTop / maximum)) : 0;
}

export function scrollTopFromRatio(ratio: number, scrollHeight: number, clientHeight: number) {
  const maximum = Math.max(0, scrollHeight - clientHeight);
  return Math.max(0, Math.min(maximum, maximum * Math.max(0, Math.min(1, ratio))));
}

export function findPositionAnchorIndex(anchor: DocumentPositionAnchor, candidates: string[]) {
  if (!anchor.quote) return -1;
  const quote = normalizePositionText(anchor.quote);
  const normalized = candidates.map(normalizePositionText);
  const exact = normalized.findIndex((candidate) => candidate === quote);
  if (exact >= 0) return exact;

  const containing = normalized
    .map((candidate, index) => ({ candidate, index }))
    .filter(({ candidate }) => candidate.includes(quote) || quote.includes(candidate));
  return containing.length === 1 ? containing[0].index : -1;
}
