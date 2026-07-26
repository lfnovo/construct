import assert from "node:assert/strict";
import test from "node:test";
import { buildTypeColorMap, toggleFilterValue } from "../src/explore.ts";

test("adds a type without replacing the existing selection", () => {
  assert.deepEqual(toggleFilterValue(["Person"], "Project"), ["Person", "Project"]);
});

test("removes only the clicked type from the selection", () => {
  assert.deepEqual(toggleFilterValue(["Person", "Project", "Playbook"], "Project"), ["Person", "Playbook"]);
});

test("assigns a distinct color to each type in a bundle", () => {
  const colors = buildTypeColorMap(["Person", "Project", "Organization"]);
  assert.notEqual(colors.Person, colors.Project);
  assert.notEqual(colors.Person, colors.Organization);
  assert.notEqual(colors.Project, colors.Organization);
});

test("keeps type colors stable regardless of discovery order", () => {
  assert.deepEqual(
    buildTypeColorMap(["Project", "Person", "Organization"]),
    buildTypeColorMap(["Organization", "Project", "Person"]),
  );
});
