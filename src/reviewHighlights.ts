import type { Element, ElementContent, Root, Text } from "hast";
import type { ReviewComment } from "./review";
import { resolveReviewAnchors } from "./reviewAnchors.ts";

type TextRun = { node: Text; start: number; end: number };
type NormalizedRun = { start: number; end: number; rawStart: number; rawEnd: number; whitespace: boolean };
type Highlight = { id: string; number: number; start: number; end: number };
const TABLE_CONTAINERS = new Set(["table", "tbody", "thead", "tfoot", "tr"]);

/** Trusted, render-time decorations, applied after sanitization and highlighting. */
export function rehypeReviewHighlights({ comments }: { comments: readonly ReviewComment[] }) {
  return (tree: Root) => {
    if (!comments.length) return;
    const textRuns: TextRun[] = [];
    const parts: string[] = [];
    let length = 0;
    const collect = (parent: Root | Element) => {
      for (const child of parent.children) {
        if (child.type === "text") {
          // hast-util-to-jsx-runtime removes structural whitespace from tables.
          if (parent.type === "element" && TABLE_CONTAINERS.has(parent.tagName) && !child.value.trim()) continue;
          textRuns.push({ node: child, start: length, end: length + child.value.length });
          parts.push(child.value);
          length += child.value.length;
        } else if (child.type === "element") {
          // Mermaid owns its generated SVG; its source is not rendered prose.
          const classes = child.properties.className;
          if (child.tagName !== "code" || !Array.isArray(classes) || !classes.includes("language-mermaid")) {
            collect(child);
          }
        }
      }
    };
    collect(tree);

    const raw = parts.join("");
    const normalizedRuns: NormalizedRun[] = [];
    const normalizedParts: string[] = [];
    let normalizedLength = 0;
    for (const match of raw.matchAll(/\s+|\S+/g)) {
      const whitespace = /^\s/.test(match[0]);
      if (whitespace && (!normalizedLength || match.index + match[0].length === raw.length)) continue;
      const value = whitespace ? " " : match[0];
      normalizedRuns.push({ start: normalizedLength, end: normalizedLength + value.length,
        rawStart: match.index, rawEnd: match.index + match[0].length, whitespace });
      normalizedParts.push(value);
      normalizedLength += value.length;
    }

    const rawOffset = (offset: number, end: boolean) => {
      let low = 0;
      let high = normalizedRuns.length;
      // An end offset belongs to the character immediately before it.
      const character = end ? offset - 1 : offset;
      while (low < high) {
        const mid = (low + high) >>> 1;
        if (normalizedRuns[mid].end <= character) low = mid + 1;
        else high = mid;
      }
      const run = normalizedRuns[low];
      return run.whitespace ? (end ? run.rawEnd : run.rawStart) : run.rawStart + offset - run.start;
    };
    const highlights: Highlight[] = resolveReviewAnchors(normalizedParts.join(""), comments)
      .map((range) => ({ ...range, start: rawOffset(range.start, false), end: rawOffset(range.end, true) }));
    if (!highlights.length) return;

    const events = highlights.flatMap((highlight) => [
      { offset: highlight.start, highlight, add: true },
      { offset: highlight.end, highlight, add: false },
    ]).sort((a, b) => a.offset - b.offset);
    const active = new Set<Highlight>();
    const replacements = new Map<Text, ElementContent[]>();
    let eventIndex = 0;
    const applyEvents = (offset: number) => {
      while (eventIndex < events.length && events[eventIndex].offset <= offset) {
        const event = events[eventIndex++];
        if (event.add) active.add(event.highlight);
        else active.delete(event.highlight);
      }
    };
    for (const run of textRuns) {
      const children: ElementContent[] = [];
      let offset = run.start;
      while (offset < run.end) {
        applyEvents(offset);
        const end = Math.min(run.end, events[eventIndex]?.offset ?? run.end);
        let child: ElementContent = { type: "text", value: run.node.value.slice(offset - run.start, end - run.start) };
        // Nested marks retain every overlapping comment's navigation target.
        for (const highlight of [...active].sort((a, b) => b.number - a.number)) {
          child = { type: "element", tagName: "mark", properties: {
            dataReviewId: highlight.id, tabIndex: 0, role: "button",
            ariaLabel: `Open review comment ${highlight.number}`,
          }, children: [child] };
        }
        children.push(child);
        offset = end;
      }
      replacements.set(run.node, children);
    }
    const decorate = (parent: Root | Element) => {
      for (let index = 0; index < parent.children.length; index += 1) {
        const child = parent.children[index];
        if (child.type === "text") {
          const replacement = replacements.get(child);
          if (replacement) {
            parent.children.splice(index, 1, ...replacement);
            index += replacement.length - 1;
          }
        } else if (child.type === "element") decorate(child);
      }
    };
    decorate(tree);
  };
}
