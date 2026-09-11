import assert from "node:assert/strict";
import test from "node:test";
import {
  normalizeLocationName,
  normalizeLocationRecord,
  renameLocation,
} from "../src/locations.ts";

const location = {
  id: "location-1",
  path: "/projects/construct/docs",
  name: "docs",
  available: true,
};

test("renames only the persisted Location display label", () => {
  const renamed = renameLocation(location, "  Construct documentation  ");

  assert.deepEqual(renamed, {
    ...location,
    name: "Construct documentation",
  });
  assert.equal(renamed.id, location.id);
  assert.equal(renamed.path, location.path);
});

test("rejects blank Location names without changing the record", () => {
  assert.equal(renameLocation(location, " \t "), null);
});

test("restores custom names and falls back safely for legacy malformed names", () => {
  assert.equal(normalizeLocationName("  Shared docs ", location.path), "Shared docs");
  assert.equal(normalizeLocationName("", location.path), "docs");
  assert.equal(normalizeLocationName(undefined, location.path), "docs");
  assert.deepEqual(normalizeLocationRecord({ ...location, name: "   " }), location);
});
