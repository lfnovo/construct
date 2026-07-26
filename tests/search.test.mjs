import assert from "node:assert/strict";
import test from "node:test";
import {
  activeFilterCount,
  emptyKnowledgeFilters,
  rememberSearch,
  serializeSearchReferences,
  toggleSearchFilter,
} from "../src/search.ts";

test("toggles multi-select search filters without replacing prior values", () => {
  assert.deepEqual(toggleSearchFilter(["Person"], "Project"), ["Person", "Project"]);
  assert.deepEqual(toggleSearchFilter(["Person", "Project"], "Person"), ["Project"]);
});

test("counts visible and advanced filters", () => {
  assert.equal(activeFilterCount({
    ...emptyKnowledgeFilters(),
    types: ["Person", "Project"],
    tags: ["work"],
    pathPrefix: "people/",
    findings: "with",
  }), 5);
});

test("recent searches are deduplicated and bounded", () => {
  const filters = emptyKnowledgeFilters();
  let recent = [];
  for (let index = 0; index < 22; index += 1) {
    recent = rememberSearch(recent, {
      query: `query ${index}`,
      locationIds: ["one"],
      filters,
    }, index);
  }
  assert.equal(recent.length, 20);
  recent = rememberSearch(recent, {
    query: "query 21",
    locationIds: ["one"],
    filters,
  }, 100);
  assert.equal(recent.length, 20);
  assert.equal(recent[0].searchedAt, 100);
  assert.equal(recent.filter((item) => item.query === "query 21").length, 1);
});

test("serializes relative references without local filesystem paths", () => {
  const output = serializeSearchReferences([{
    locationId: "loc-1",
    relativePath: "people/luis.md",
    title: "Luis",
    matchReason: "Title match",
  }], [{
    id: "loc-1",
    path: "/Users/private/knowledge",
    name: "knowledge",
    available: true,
  }]);
  assert.match(output, /knowledge:people\/luis\.md/);
  assert.doesNotMatch(output, /Users\/private/);
});
