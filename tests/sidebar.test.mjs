import assert from "node:assert/strict";
import test from "node:test";
import {
  defaultSidebarPanelSizes,
  resizeSidebarPanelPair,
  sanitizeSidebarPanelSizes,
} from "../src/sidebar.ts";

test("sidebar panel sizes restore safe normalized defaults", () => {
  assert.deepEqual(sanitizeSidebarPanelSizes(undefined), defaultSidebarPanelSizes);

  const restored = sanitizeSidebarPanelSizes({
    locations: 2,
    files: Number.NaN,
    history: 1,
  });
  assert.equal(restored.locations + restored.files + restored.history, 1);
  assert.ok(restored.files > 0);
});

test("resizing adjacent expanded panels preserves their combined weight", () => {
  const resized = resizeSidebarPanelPair(
    defaultSidebarPanelSizes,
    "locations",
    "files",
    200,
    300,
    50,
  );

  assert.equal(resized.locations, 0.36);
  assert.equal(resized.files, 0.36);
  assert.equal(resized.history, defaultSidebarPanelSizes.history);
});

test("sidebar panel resize enforces a usable minimum height", () => {
  const resized = resizeSidebarPanelPair(
    defaultSidebarPanelSizes,
    "files",
    "history",
    300,
    200,
    500,
  );

  assert.equal(resized.files, 0.63);
  assert.equal(resized.history, 0.12);
});
