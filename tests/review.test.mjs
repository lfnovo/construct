import assert from "node:assert/strict";
import test from "node:test";
import {
  buildReviewPrompt,
  setReviewComments,
  splitReviewDocument,
} from "../src/review.ts";
import { extractOkfLinks } from "../src/okf.ts";

const comment = {
  id: "review-1",
  quote: "A selected sentence.",
  comment: "Make this claim more concrete.",
  createdAt: "2026-07-26T12:00:00.000Z",
};

test("stores reviews after frontmatter and restores the exact document when cleared", () => {
  const original = "---\r\ntype: Note\r\n---\r\n\r\n# Title\r\n\r\nBody";
  const added = setReviewComments(original, [comment]);

  assert.equal(added.error, null);
  assert.match(added.content, /^---\r\ntype: Note\r\n---\r\n<!-- construct-review:v1\r\n/);
  assert.deepEqual(splitReviewDocument(added.content).comments, [comment]);
  assert.equal(setReviewComments(added.content, []).content, original);
});

test("stores reviews at the beginning when a document has no frontmatter", () => {
  const original = "# Title\n\nBody\n";
  const added = setReviewComments(original, [comment]).content;

  assert.ok(added.startsWith("<!-- construct-review:v1\n"));
  assert.equal(splitReviewDocument(added).body, original);
  assert.equal(setReviewComments(added, []).content, original);
});

test("escapes HTML comment terminators without changing review text", () => {
  const dangerous = { ...comment, comment: "Do not emit --> in the wrapper." };
  const added = setReviewComments("# Title", [dangerous]).content;

  assert.equal(added.includes("--> in the wrapper"), false);
  assert.deepEqual(splitReviewDocument(added).comments, [dangerous]);
});

test("keeps malformed review data accessible and refuses to rewrite it", () => {
  const malformed = "<!-- construct-review:v1\nnot-json\n-->\n# Title";
  const parsed = splitReviewDocument(malformed);
  const update = setReviewComments(malformed, [comment]);

  assert.match(parsed.error, /invalid/);
  assert.equal(parsed.body, "# Title");
  assert.equal(update.content, malformed);
  assert.match(update.error, /invalid/);
});

test("builds a standalone agent prompt while escaping XML-like content", () => {
  const prompt = buildReviewPrompt("docs/spec.md", [
    { ...comment, id: `review-"1"`, quote: "x < y & y > z" },
  ]);

  assert.match(prompt, /File: docs\/spec\.md/);
  assert.match(prompt, /<comment id="review-&quot;1&quot;">/);
  assert.match(prompt, /x &lt; y &amp; y &gt; z/);
  assert.match(prompt, /construct-review:v1/);
});

test("does not turn Markdown links inside review feedback into OKF graph edges", () => {
  const reviewed = setReviewComments("# Title", [{
    ...comment,
    comment: "Compare this with [another concept](/references/other.md).",
  }]).content;

  assert.deepEqual(extractOkfLinks(reviewed, "/bundle/current.md", "/bundle"), []);
});
