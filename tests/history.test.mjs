import assert from "node:assert/strict";
import test from "node:test";
import { deduplicateHistory } from "../src/history.ts";

function event({ id, path, observedAt, kind = "modified", previousPath, locationId = "location-a" }) {
  return {
    id,
    locationId,
    path,
    previousPath,
    relativePath: path,
    kind,
    observedAt,
    source: "external",
    available: kind !== "removed",
  };
}

test("keeps only the latest event for a repeatedly edited file", () => {
  const result = deduplicateHistory([
    event({ id: "first", path: "/repo/notes.md", observedAt: 100 }),
    event({ id: "latest", path: "/repo/notes.md", observedAt: 300 }),
    event({ id: "middle", path: "/repo/notes.md", observedAt: 200 }),
  ]);

  assert.deepEqual(result.map(({ id }) => id), ["latest"]);
});

test("preserves file identity across a rename and later edits", () => {
  const result = deduplicateHistory([
    event({ id: "created", path: "/repo/old.md", observedAt: 100, kind: "created" }),
    event({ id: "renamed", path: "/repo/new.md", previousPath: "/repo/old.md", observedAt: 200, kind: "renamed" }),
    event({ id: "edited", path: "/repo/new.md", observedAt: 300 }),
  ]);

  assert.deepEqual(result.map(({ id }) => id), ["edited"]);
});

test("does not merge equal paths from different locations", () => {
  const result = deduplicateHistory([
    event({ id: "one", path: "/notes.md", observedAt: 100, locationId: "location-a" }),
    event({ id: "two", path: "/notes.md", observedAt: 200, locationId: "location-b" }),
  ]);

  assert.deepEqual(result.map(({ id }) => id), ["two", "one"]);
});
