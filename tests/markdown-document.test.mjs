import assert from "node:assert/strict";
import test from "node:test";
import {
  joinMarkdownDocument,
  serializeVisualMarkdown,
  splitMarkdownDocument,
} from "../src/markdownDocument.ts";

test("leaves Markdown without frontmatter untouched", () => {
  const source = "# Title\n\nBody\n";
  const parts = splitMarkdownDocument(source);

  assert.equal(parts.hasFrontmatter, false);
  assert.equal(parts.frontmatter, "");
  assert.equal(parts.body, source);
  assert.equal(joinMarkdownDocument(parts.frontmatter, parts.body), source);
});

test("preserves YAML frontmatter byte-for-byte with LF endings", () => {
  const source = "---\ntype: \"Knowledge Map\"\ntags: [one, two]\n---\n# Title\n";
  const parts = splitMarkdownDocument(source);

  assert.equal(parts.hasFrontmatter, true);
  assert.equal(parts.frontmatter, "---\ntype: \"Knowledge Map\"\ntags: [one, two]\n---\n");
  assert.equal(parts.body, "# Title\n");
  assert.equal(joinMarkdownDocument(parts.frontmatter, parts.body), source);
});

test("preserves YAML frontmatter byte-for-byte with CRLF endings", () => {
  const source = "---\r\ndescription: |-\r\n  Two lines\r\n  of text\r\n---\r\n\r\nBody\r\n";
  const parts = splitMarkdownDocument(source);

  assert.equal(parts.frontmatter, "---\r\ndescription: |-\r\n  Two lines\r\n  of text\r\n---\r\n");
  assert.equal(parts.body, "\r\nBody\r\n");
  assert.equal(joinMarkdownDocument(parts.frontmatter, parts.body), source);
});

test("does not mistake later horizontal rules for the frontmatter closing delimiter", () => {
  const source = "---\ntitle: Test\n---\nFirst\n\n---\n\nSecond\n";
  const parts = splitMarkdownDocument(source);

  assert.equal(parts.body, "First\n\n---\n\nSecond\n");
});

test("reports an unclosed frontmatter block without hiding its source", () => {
  const source = "---\ntitle: Test\n# still YAML";
  const parts = splitMarkdownDocument(source);

  assert.equal(parts.hasFrontmatter, true);
  assert.match(parts.error || "", /not closed/i);
  assert.equal(parts.body, source);
  assert.equal(joinMarkdownDocument(parts.frontmatter, parts.body), source);
});

test("restores exact source bytes when visual undo returns to the normalized baseline", () => {
  const original = "---\r\ntype: Note\r\n---\r\n\r\n# Title\r\n\r\nBody without a final newline";
  const frontmatter = "---\r\ntype: Note\r\n---\r\n";
  const normalizedBody = "\n# Title\n\nBody without a final newline\n";

  assert.equal(
    serializeVisualMarkdown(frontmatter, normalizedBody, normalizedBody, original),
    original,
  );
  assert.equal(
    serializeVisualMarkdown(frontmatter, `${normalizedBody}\nChanged`, normalizedBody, original),
    `${frontmatter}${normalizedBody}\nChanged`,
  );
});
