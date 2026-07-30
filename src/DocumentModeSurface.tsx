import { useEffect, useRef, type ReactNode } from "react";
import {
  findPositionAnchorIndex,
  normalizePositionText,
  scrollRatio,
  scrollTopFromRatio,
  type DocumentModeTransfer,
  type DocumentPositionAnchor,
  type DocumentViewState,
} from "./documentPosition";
import type { TabMode } from "./types";

type RestoreState = {
  saved: DocumentViewState | null;
  transfer: DocumentModeTransfer | null;
};

type Props = {
  tabId: string;
  mode: TabMode;
  children: ReactNode;
  consumeRestoreState: () => RestoreState;
  onViewState: (state: DocumentViewState) => void;
};

const scrollerSelectors: Record<TabMode, string> = {
  preview: ".markdown-preview",
  review: ".review-document .markdown-preview",
  edit: ".visual-editor",
  source: ".cm-scroller",
  diff: ".diff-view",
};

const blockSelector = [
  ".cm-line",
  ".ProseMirror > *",
  ".markdown-preview > *",
  ".diff-view pre",
].join(", ");

function visibleBlocks(scroller: HTMLElement) {
  return Array.from(scroller.querySelectorAll<HTMLElement>(blockSelector))
    .filter((element) => normalizePositionText(element.textContent || ""));
}

function captureViewState(scroller: HTMLElement): DocumentViewState {
  const scrollerRect = scroller.getBoundingClientRect();
  const viewportTop = scrollerRect.top + 8;
  const blocks = visibleBlocks(scroller);
  const block = blocks.find((candidate) => {
    const rect = candidate.getBoundingClientRect();
    return rect.bottom > viewportTop && rect.top < scrollerRect.bottom;
  }) || null;
  let anchor: DocumentPositionAnchor | null = null;
  if (block) {
    const rect = block.getBoundingClientRect();
    const text = normalizePositionText(block.textContent || "").slice(0, 240);
    anchor = {
      quote: text,
      progress: rect.height
        ? Math.max(0, Math.min(1, (viewportTop - rect.top) / rect.height))
        : 0,
    };
  }
  return {
    scrollTop: scroller.scrollTop,
    ratio: scrollRatio(scroller.scrollTop, scroller.scrollHeight, scroller.clientHeight),
    anchor,
  };
}

function restoreFromAnchor(scroller: HTMLElement, anchor: DocumentPositionAnchor) {
  const blocks = visibleBlocks(scroller);
  const index = findPositionAnchorIndex(
    anchor,
    blocks.map((block) => block.textContent || ""),
  );
  if (index < 0) return false;
  const block = blocks[index];
  const scrollerRect = scroller.getBoundingClientRect();
  const blockRect = block.getBoundingClientRect();
  const top = scroller.scrollTop
    + blockRect.top
    - scrollerRect.top
    + (blockRect.height * anchor.progress)
    - 8;
  scroller.scrollTop = Math.max(0, top);
  return true;
}

function restoreViewState(scroller: HTMLElement, state: RestoreState) {
  if (state.transfer) {
    if (state.transfer.anchor && restoreFromAnchor(scroller, state.transfer.anchor)) return;
    scroller.scrollTop = scrollTopFromRatio(
      state.transfer.ratio,
      scroller.scrollHeight,
      scroller.clientHeight,
    );
    return;
  }
  if (state.saved) scroller.scrollTop = state.saved.scrollTop;
}

export function DocumentModeSurface({
  tabId,
  mode,
  children,
  consumeRestoreState,
  onViewState,
}: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const restoreRef = useRef(consumeRestoreState);
  const onViewStateRef = useRef(onViewState);

  useEffect(() => {
    restoreRef.current = consumeRestoreState;
    onViewStateRef.current = onViewState;
  }, [consumeRestoreState, onViewState]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let scroller: HTMLElement | null = null;
    let animationFrame = 0;
    let observer: MutationObserver | null = null;

    const capture = () => {
      if (scroller) onViewStateRef.current(captureViewState(scroller));
    };
    const attach = () => {
      if (scroller) return true;
      const candidate = host.querySelector<HTMLElement>(scrollerSelectors[mode]);
      if (!candidate) return false;
      scroller = candidate;
      scroller.addEventListener("scroll", capture, { passive: true });
      const restoreState = restoreRef.current();
      animationFrame = window.requestAnimationFrame(() => {
        if (!scroller) return;
        restoreViewState(scroller, restoreState);
        capture();
      });
      return true;
    };

    if (!attach()) {
      observer = new MutationObserver(() => {
        if (attach()) observer?.disconnect();
      });
      observer.observe(host, { childList: true, subtree: true });
    }

    return () => {
      observer?.disconnect();
      window.cancelAnimationFrame(animationFrame);
      if (scroller) {
        capture();
        scroller.removeEventListener("scroll", capture);
      }
    };
  }, [mode, tabId]);

  return <div className="document-mode-surface" ref={hostRef}>{children}</div>;
}
