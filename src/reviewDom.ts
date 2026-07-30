import {
  createReviewAnchor,
  normalizeReviewText,
  type ResolvedReviewAnchor,
  type ReviewAnchor,
} from "./reviewAnchors";

type TextPosition = {
  node: Text;
  start: number;
  end: number;
};

type RenderedTextIndex = {
  text: string;
  positions: TextPosition[];
};

export function buildRenderedTextIndex(root: HTMLElement): RenderedTextIndex {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const positions: TextPosition[] = [];
  let text = "";
  let node = walker.nextNode() as Text | null;
  while (node) {
    const value = node.nodeValue || "";
    for (let offset = 0; offset < value.length; offset += 1) {
      const character = value[offset];
      if (/\s/.test(character)) {
        if (text && !text.endsWith(" ")) {
          text += " ";
          positions.push({ node, start: offset, end: offset + 1 });
        } else if (text.endsWith(" ") && positions.length) {
          const previous = positions[positions.length - 1];
          if (previous.node === node) previous.end = offset + 1;
        }
      } else {
        text += character;
        positions.push({ node, start: offset, end: offset + 1 });
      }
    }
    node = walker.nextNode() as Text | null;
  }
  if (text.endsWith(" ")) {
    text = text.slice(0, -1);
    positions.pop();
  }
  return { text, positions };
}

export function captureReviewAnchor(
  root: HTMLElement,
  range: Range,
  quote: string,
): ReviewAnchor | null {
  const before = document.createRange();
  before.selectNodeContents(root);
  before.setEnd(range.startContainer, range.startOffset);
  const approximateStart = normalizeReviewText(before.toString()).length;
  return createReviewAnchor(buildRenderedTextIndex(root).text, quote, approximateStart);
}

export function clearReviewHighlights(root: HTMLElement) {
  const marks = Array.from(root.querySelectorAll<HTMLElement>("mark[data-review-id]")).reverse();
  for (const mark of marks) {
    const parent = mark.parentNode;
    mark.replaceWith(...Array.from(mark.childNodes));
    parent?.normalize();
  }
}

export function highlightReviewRange(
  root: HTMLElement,
  resolved: ResolvedReviewAnchor,
  reviewId: string,
  reviewNumber: number,
) {
  const index = buildRenderedTextIndex(root);
  const grouped = new Map<Text, { start: number; end: number }>();
  for (const position of index.positions.slice(resolved.start, resolved.end)) {
    const current = grouped.get(position.node);
    grouped.set(position.node, {
      start: current ? Math.min(current.start, position.start) : position.start,
      end: current ? Math.max(current.end, position.end) : position.end,
    });
  }

  for (const [node, offsets] of Array.from(grouped.entries()).reverse()) {
    if (!node.isConnected || offsets.end <= offsets.start) continue;
    const selected = node.splitText(offsets.start);
    selected.splitText(offsets.end - offsets.start);
    const mark = document.createElement("mark");
    mark.dataset.reviewId = reviewId;
    mark.tabIndex = 0;
    mark.setAttribute("role", "button");
    mark.setAttribute("aria-label", `Open review comment ${reviewNumber}`);
    selected.parentNode?.insertBefore(mark, selected);
    mark.appendChild(selected);
  }
}
