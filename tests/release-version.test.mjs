import assert from "node:assert/strict";
import test from "node:test";
import {
  projectVersions,
  releaseVersionErrors,
} from "../scripts/check-release-version.mjs";

test("the current project version sources stay synchronized", () => {
  const versions = projectVersions();
  assert.equal(Object.keys(versions).length, 5);
  assert.equal(new Set(Object.values(versions)).size, 1);
  assert.match(versions["package.json"], /^\d+\.\d+\.\d+/);
});

test("a release tag must match every version source", () => {
  assert.deepEqual(releaseVersionErrors("v0.1.0", {
    package: "0.1.0",
    tauri: "0.1.0",
  }), []);
  assert.deepEqual(releaseVersionErrors("v0.1.0", {
    package: "0.1.0",
    tauri: "0.2.0",
  }), ['tauri has version "0.2.0"; expected "0.1.0".']);
  assert.match(releaseVersionErrors("preview", {})[0], /must use v<major>/);
});
