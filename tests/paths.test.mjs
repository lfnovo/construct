import assert from "node:assert/strict";
import test from "node:test";
import {
  pathBelongsToLocation,
  pathIdentity,
  parentPath,
  pathsEqual,
  mostSpecificContainingLocation,
  relativePathWithinLocation,
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

test("derives a Location-relative path with portable separators", () => {
  assert.equal(
    relativePathWithinLocation(
      String.raw`C:\Users\Lucas\knowledge\notes\context.md`,
      String.raw`C:\Users\Lucas\knowledge`,
    ),
    "notes/context.md",
  );
  assert.equal(
    relativePathWithinLocation(
      String.raw`\\?\C:\Users\Lucas\Knowledge\Notes\Context.md`,
      String.raw`C:\Users\Lucas\Knowledge`,
    ),
    "Notes/Context.md",
  );
});

test("finds the parent directory across Unix and Windows paths", () => {
  assert.equal(parentPath("/Users/luis/knowledge/index.md"), "/Users/luis/knowledge");
  assert.equal(parentPath("/index.md"), "/");
  assert.equal(parentPath(String.raw`C:\Users\Lucas\knowledge\index.md`), "C:/Users/Lucas/knowledge");
  assert.equal(
    parentPath(String.raw`\\?\C:\Users\Lucas\knowledge\index.md`),
    String.raw`\\?\C:\Users\Lucas\knowledge`,
  );
});

test("prefers the most specific Location containing a file", () => {
  const locations = [
    { id: "home", path: "/Users/luis" },
    { id: "knowledge", path: "/Users/luis/knowledge" },
  ];
  assert.equal(
    mostSpecificContainingLocation(locations, "/Users/luis/knowledge/index.md")?.id,
    "knowledge",
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
