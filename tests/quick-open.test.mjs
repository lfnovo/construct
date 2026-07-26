import assert from "node:assert/strict";
import test from "node:test";
import { moveQuickOpenSelection } from "../src/quickOpen.ts";

test("moves through quick-open results and wraps at both ends", () => {
  assert.equal(moveQuickOpenSelection(0, 4, "next"), 1);
  assert.equal(moveQuickOpenSelection(3, 4, "next"), 0);
  assert.equal(moveQuickOpenSelection(2, 4, "previous"), 1);
  assert.equal(moveQuickOpenSelection(0, 4, "previous"), 3);
});

test("keeps quick-open selection safe for empty or changing results", () => {
  assert.equal(moveQuickOpenSelection(5, 0, "next"), 0);
  assert.equal(moveQuickOpenSelection(5, 2, "next"), 0);
  assert.equal(moveQuickOpenSelection(-2, 2, "previous"), 1);
});
