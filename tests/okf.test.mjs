import assert from "node:assert/strict";
import test from "node:test";
import {
  extractOkfLinks,
  inspectOkfDocument,
  resolveOkfLink,
  withoutFrontmatter,
} from "../src/okf.ts";

test("inspects OKF concept metadata and preserves extra fields", () => {
  const result = inspectOkfDocument(`---
type: "Knowledge Map"
title: Construct map
description: >-
  A connected view
tags: [construct, "coding agents"]
status: active
---
# Construct
`, "maps/construct.md");

  assert.equal(result.kind, "concept");
  assert.equal(result.isConformant, true);
  assert.equal(result.metadata.type, "Knowledge Map");
  assert.equal(result.metadata.title, "Construct map");
  assert.deepEqual(result.metadata.tags, ["construct", "coding agents"]);
  assert.equal(result.metadata.extra.status, "active");
});

test("reports missing and malformed concept frontmatter", () => {
  const missing = inspectOkfDocument("# No metadata", "concepts/missing.md");
  assert.equal(missing.isConformant, false);
  assert.match(missing.issues[0].message, /frontmatter/i);

  const malformed = inspectOkfDocument("---\ntype: Concept\n# never closed", "concepts/malformed.md");
  assert.equal(malformed.isConformant, false);
  assert.match(malformed.issues[0].message, /not closed/i);
});

test("accepts a root index that declares only the OKF version", () => {
  const result = inspectOkfDocument("---\nokf_version: 0.1\n---\n# Bundle", "index.md", true);
  assert.equal(result.kind, "index");
  assert.equal(result.isConformant, true);
  assert.equal(result.metadata.okfVersion, "0.1");
  assert.deepEqual(result.issues, []);
});

test("warns when an OKF log does not use ISO date headings", () => {
  const result = inspectOkfDocument("# Log\n\n## Today\n", "log.md");
  assert.equal(result.kind, "log");
  assert.equal(result.isConformant, true);
  assert.match(result.issues[0].message, /ISO date/i);
});

test("resolves bundle-root and relative OKF links", () => {
  assert.equal(
    resolveOkfLink("/bundle/maps/source.md", "/bundle", "/people/luis.md"),
    "/bundle/people/luis.md",
  );
  assert.equal(
    resolveOkfLink("/bundle/maps/source.md", "/bundle", "../people/luis.md#work"),
    "/bundle/people/luis.md",
  );
});

test("extracts unique local Markdown links and ignores remote or image links", () => {
  const links = extractOkfLinks(
    `---
type: Map
resource: https://example.com/ignored.md
---
[Luis](/people/luis.md)
[Luis again](/people/luis.md#work)
[Relative](../projects/construct.md)
![Image](./diagram.md)
[External](https://example.com/page.md)
[Heading](#local)
`,
    "/bundle/maps/source.md",
    "/bundle",
  );

  assert.deepEqual(links, [
    "/bundle/people/luis.md",
    "/bundle/projects/construct.md",
  ]);
});

test("removes frontmatter without removing the Markdown body", () => {
  assert.equal(withoutFrontmatter("---\r\ntype: Concept\r\n---\r\n# Body\r\n"), "# Body\r\n");
});
