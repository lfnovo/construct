import { splitMarkdownDocument } from "./markdownDocument.ts";
import type { ReviewAnchor } from "./reviewAnchors.ts";

const REVIEW_MARKER = "<!-- construct-review:v1";
const REVIEW_START = `${REVIEW_MARKER}\n`;
const REVIEW_END = "\n-->";

export type ReviewComment = {
  id: string;
  quote: string;
  comment: string;
  createdAt: string;
  anchor?: ReviewAnchor;
};

export type ReviewDocument = {
  frontmatter: string;
  reviewBlock: string;
  body: string;
  comments: ReviewComment[];
  error?: string;
};

function isReviewComment(value: unknown): value is ReviewComment {
  if (!value || typeof value !== "object") return false;
  const item = value as Record<string, unknown>;
  const anchor = item.anchor as Record<string, unknown> | undefined;
  const validAnchor = anchor === undefined || (
    anchor !== null
    && typeof anchor === "object"
    && typeof anchor.start === "number"
    && Number.isInteger(anchor.start)
    && anchor.start >= 0
    && typeof anchor.end === "number"
    && Number.isInteger(anchor.end)
    && anchor.end >= anchor.start
    && typeof anchor.prefix === "string"
    && typeof anchor.suffix === "string"
  );
  return validAnchor
    && typeof item.id === "string"
    && typeof item.quote === "string"
    && typeof item.comment === "string"
    && typeof item.createdAt === "string";
}

function lineEndingFor(content: string) {
  return content.includes("\r\n") ? "\r\n" : "\n";
}

function serializeReviewBlock(comments: ReviewComment[], lineEnding: string) {
  const payload = JSON.stringify({ comments }, null, 2)
    // HTML comments cannot contain `--`. JSON escapes preserve the original text.
    .replaceAll("--", "\\u002d\\u002d")
    .replaceAll("\n", lineEnding);
  return `${REVIEW_MARKER}${lineEnding}${payload}${lineEnding}-->${lineEnding}`;
}

export function splitReviewDocument(content: string): ReviewDocument {
  const document = splitMarkdownDocument(content);
  if (document.error) {
    return {
      frontmatter: "",
      reviewBlock: "",
      body: content,
      comments: [],
      error: document.error,
    };
  }

  if (!document.body.startsWith(REVIEW_MARKER)) {
    return {
      frontmatter: document.frontmatter,
      reviewBlock: "",
      body: document.body,
      comments: [],
    };
  }

  const normalized = document.body.replace(/\r\n/g, "\n");
  if (!normalized.startsWith(REVIEW_START)) {
    return {
      frontmatter: document.frontmatter,
      reviewBlock: "",
      body: document.body,
      comments: [],
      error: "The Construct review block has an invalid header.",
    };
  }

  const closingIndex = normalized.indexOf(REVIEW_END, REVIEW_START.length);
  if (closingIndex < 0) {
    return {
      frontmatter: document.frontmatter,
      reviewBlock: "",
      body: document.body,
      comments: [],
      error: "The Construct review block is not closed.",
    };
  }

  const normalizedBlockEnd = closingIndex + REVIEW_END.length;
  const hasTrailingLineEnding = normalized.slice(normalizedBlockEnd).startsWith("\n");
  const normalizedBlock = normalized.slice(0, normalizedBlockEnd + (hasTrailingLineEnding ? 1 : 0));
  const originalBlockLength = document.body.startsWith(normalizedBlock)
    ? normalizedBlock.length
    : normalizedBlock.replaceAll("\n", "\r\n").length;
  const reviewBlock = document.body.slice(0, originalBlockLength);
  const rawPayload = normalized.slice(REVIEW_START.length, closingIndex);

  try {
    const payload = JSON.parse(rawPayload) as { comments?: unknown };
    if (!Array.isArray(payload.comments) || !payload.comments.every(isReviewComment)) {
      throw new Error("Expected a comments array.");
    }
    return {
      frontmatter: document.frontmatter,
      reviewBlock,
      body: document.body.slice(originalBlockLength),
      comments: payload.comments,
    };
  } catch (cause) {
    return {
      frontmatter: document.frontmatter,
      reviewBlock,
      body: document.body.slice(originalBlockLength),
      comments: [],
      error: `The Construct review block is invalid: ${cause instanceof Error ? cause.message : String(cause)}`,
    };
  }
}

export function setReviewComments(content: string, comments: ReviewComment[]) {
  const document = splitReviewDocument(content);
  if (document.error) return { content, error: document.error };
  const reviewBlock = comments.length ? serializeReviewBlock(comments, lineEndingFor(content)) : "";
  return {
    content: `${document.frontmatter}${reviewBlock}${document.body}`,
    error: null,
  };
}

function escapeXml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function escapeXmlAttribute(value: string) {
  return escapeXml(value)
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

export function buildReviewPrompt(relativePath: string, comments: ReviewComment[]) {
  const entries = comments.map((item) => [
    `<comment id="${escapeXmlAttribute(item.id)}">`,
    `<quote>${escapeXml(item.quote)}</quote>`,
    `<feedback>${escapeXml(item.comment)}</feedback>`,
    "</comment>",
  ].join("\n")).join("\n\n");

  return [
    "Review the following Markdown document and address every open Construct review comment.",
    "",
    `File: ${relativePath}`,
    "",
    "<review-comments>",
    entries,
    "</review-comments>",
    "",
    "The comments are also embedded in the document's construct-review:v1 block.",
    "Update the document itself. Remove comments you addressed from that block and preserve unresolved comments.",
  ].join("\n");
}
