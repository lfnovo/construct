import assert from "node:assert/strict";
import test from "node:test";
import {
  findPositionAnchorIndex,
  normalizePositionText,
  scrollRatio,
  scrollTopFromRatio,
} from "../src/documentPosition.ts";

test("normalizes rendered and source block prefixes for cross-mode matching", () => {
  assert.equal(normalizePositionText("##  A heading\n"), "A heading");
  assert.equal(normalizePositionText("- **Important** item"), "Important item");
  assert.equal(normalizePositionText("> Quoted text"), "Quoted text");
});

test("finds an exact semantic block before using a containing match", () => {
  const anchor = { quote: "The same passage", progress: 0.4 };
  assert.equal(findPositionAnchorIndex(anchor, [
    "Unrelated",
    "The same passage",
    "The same passage with more text",
  ]), 1);
});

test("refuses an ambiguous containing match", () => {
  const anchor = { quote: "same passage", progress: 0 };
  assert.equal(findPositionAnchorIndex(anchor, [
    "First same passage",
    "Second same passage",
  ]), -1);
});

test("bounds proportional scroll restoration", () => {
  assert.equal(scrollRatio(250, 1_200, 200), 0.25);
  assert.equal(scrollTopFromRatio(0.25, 1_200, 200), 250);
  assert.equal(scrollTopFromRatio(2, 1_200, 200), 1_000);
  assert.equal(scrollTopFromRatio(-1, 1_200, 200), 0);
});
