import assert from "node:assert/strict";
import test from "node:test";
import {
  createReviewAnchor,
  resolveReviewAnchor,
} from "../src/reviewAnchors.ts";

test("creates a locator around the occurrence nearest the selection", () => {
  const text = "First repeated passage. Second repeated passage.";
  const quote = "repeated passage";
  const anchor = createReviewAnchor(text, quote, 34);

  assert.ok(anchor);
  assert.equal(anchor.start, 31);
  assert.equal(text.slice(anchor.start, anchor.end), quote);
  assert.match(anchor.prefix, /Second $/);
});

test("resolves a repeated quote through its surrounding context", () => {
  const original = "Alpha repeated passage. Beta repeated passage. Gamma.";
  const quote = "repeated passage";
  const anchor = createReviewAnchor(original, quote, 30);
  const changed = "Introduction. Alpha repeated passage. Beta repeated passage. Gamma.";
  const resolved = resolveReviewAnchor(changed, quote, anchor);

  assert.ok(resolved);
  assert.equal(changed.slice(resolved.start, resolved.end), quote);
  assert.match(changed.slice(0, resolved.start), /Beta $/);
});

test("keeps a unique legacy quote locatable without a stored anchor", () => {
  const text = "A unique sentence remains available.";
  assert.deepEqual(
    resolveReviewAnchor(text, "unique sentence"),
    { start: 2, end: 17 },
  );
});

test("refuses to guess between repeated legacy quotes", () => {
  assert.equal(
    resolveReviewAnchor("repeated text and repeated text.", "repeated text"),
    null,
  );
});

test("detaches a comment when its quote no longer exists", () => {
  const anchor = createReviewAnchor("The original claim.", "original claim", 4);
  assert.equal(resolveReviewAnchor("The replacement claim.", "original claim", anchor || undefined), null);
});
