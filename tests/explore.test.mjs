import assert from "node:assert/strict";
import test from "node:test";
import { buildTypeColorMap, sortFacetsByCount, TAG_PREVIEW_LIMIT, toggleFilterValue, visibleTagFacets } from "../src/explore.ts";

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

test("sorts facets by frequency and uses the name to break ties", () => {
  assert.deepEqual(
    sortFacetsByCount([["Project", 7], ["Template", 4], ["Person", 9], ["Organization", 4]]),
    [["Person", 9], ["Project", 7], ["Organization", 4], ["Template", 4]],
  );
});

test("shows only the twenty most frequent tags before expansion", () => {
  const tags = Array.from({ length: 25 }, (_, index) => [`tag-${index + 1}`, 25 - index]);

  assert.deepEqual(visibleTagFacets(tags, undefined, false), tags.slice(0, TAG_PREVIEW_LIMIT));
  assert.deepEqual(visibleTagFacets(tags, undefined, true), tags);
});

test("keeps an active tag visible when it falls outside the preview", () => {
  const tags = Array.from({ length: 25 }, (_, index) => [`tag-${index + 1}`, 25 - index]);

  assert.deepEqual(
    visibleTagFacets(tags, "tag-24", false),
    [...tags.slice(0, TAG_PREVIEW_LIMIT), tags[23]],
  );
});
