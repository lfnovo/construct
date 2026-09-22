import { createReviewAnchor, normalizeReviewText, type ReviewAnchor } from "./reviewAnchors";

// Read rendered text only. Review decorations must never split, move, or normalize
// React-owned DOM nodes: doing so invalidates React's reconciliation references.
export function captureReviewAnchor(
  root: HTMLElement,
  range: Range,
  quote: string,
): ReviewAnchor | null {
  const before = document.createRange();
  before.selectNodeContents(root);
  before.setEnd(range.startContainer, range.startOffset);
  const text: string[] = [];
  const prefix: string[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    // Match the prose projection used by the render-time highlighter. Generated
    // diagram labels and image-error UI are not part of the Markdown text tree.
    if (!node.parentElement?.closest('[data-review-generated="true"]')) {
      const value = node.nodeValue || "";
      text.push(value);
      if (before.comparePoint(node, 0) <= 0) {
        prefix.push(node === range.startContainer ? value.slice(0, range.startOffset) : value);
      }
    }
    node = walker.nextNode();
  }
  return createReviewAnchor(text.join(""), quote, normalizeReviewText(prefix.join("")).length);
}
