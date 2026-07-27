import assert from "node:assert/strict";
import test from "node:test";
import {
  pathBelongsToLocation,
  pathIdentity,
  pathsEqual,
} from "../src/paths.ts";

test("normalizes Windows verbatim paths and separators", () => {
  assert.equal(
    pathIdentity(String.raw`\\?\C:\Users\Lucas\knowledge\index.md`),
    "c:/users/lucas/knowledge/index.md",
  );
  assert.equal(
    pathIdentity(String.raw`\\?\UNC\server\share\knowledge\index.md`),
    "//server/share/knowledge/index.md",
  );
});

test("compares Windows paths case-insensitively", () => {
  assert.equal(
    pathsEqual(
      String.raw`C:\Users\Lucas\Knowledge`,
      "\\\\?\\c:\\users\\lucas\\knowledge\\",
    ),
    true,
  );
});

test("matches files inside a Windows Location across canonical path forms", () => {
  assert.equal(
    pathBelongsToLocation(
      String.raw`\\?\C:\Users\Lucas\knowledge\projects\construct.md`,
      String.raw`C:\Users\Lucas\knowledge`,
    ),
    true,
  );
});

test("does not confuse sibling paths with descendants", () => {
  assert.equal(
    pathBelongsToLocation(
      "/Users/luis/knowledge-old/index.md",
      "/Users/luis/knowledge",
    ),
    false,
  );
  assert.equal(
    pathBelongsToLocation(
      String.raw`C:\Users\Lucas\knowledge-old\index.md`,
      String.raw`C:\Users\Lucas\knowledge`,
    ),
    false,
  );
});
