import assert from "node:assert/strict";
import test from "node:test";
import { toggleFilterValue } from "../src/explore.ts";

test("adds a type without replacing the existing selection", () => {
  assert.deepEqual(toggleFilterValue(["Person"], "Project"), ["Person", "Project"]);
});

test("removes only the clicked type from the selection", () => {
  assert.deepEqual(toggleFilterValue(["Person", "Project", "Playbook"], "Project"), ["Person", "Playbook"]);
});
