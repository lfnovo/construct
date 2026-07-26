import assert from "node:assert/strict";
import test from "node:test";
import {
  formatOkfValue,
  resolveOkfLink,
  withoutFrontmatter,
} from "../src/okf.ts";

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

test("formats open-ended typed OKF values for inspection", () => {
  assert.equal(formatOkfValue({
    kind: "mapping",
    entries: [
      { key: { kind: "string", value: "reviewed" }, value: { kind: "boolean", value: true } },
      { key: { kind: "string", value: "score" }, value: { kind: "float", value: "0.98" } },
    ],
  }), '{"reviewed":true,"score":0.98}');
});

test("removes frontmatter without removing the Markdown body", () => {
  assert.equal(withoutFrontmatter("---\r\ntype: Concept\r\n---\r\n# Body\r\n"), "# Body\r\n");
});
