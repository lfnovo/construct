import assert from "node:assert/strict";
import test from "node:test";
import { rehypeReviewHighlights } from "../src/reviewHighlights.ts";

const text = (value) => ({ type: "text", value });
const element = (tagName, children, properties = {}) => ({ type: "element", tagName, properties, children });
const comment = (id, quote, anchor) => ({ id, quote, comment: "Synthetic note", createdAt: "2026-09-02T12:00:00Z", ...(anchor ? { anchor } : {}) });
const allText = (node) => node.type === "text" ? node.value : (node.children || []).map(allText).join("");
const marks = (node) => [
  ...(node.tagName === "mark" ? [node] : []),
  ...(node.children || []).flatMap(marks),
];

test("review decorations span syntax nodes, Unicode and collapsed whitespace without changing text", () => {
  const tree = { type: "root", children: [element("p", [text("  Olá\t"), element("strong", [text("mundo")]), text("\n\n"), element("code", [text("🚀")]), text(" fim  ")])] };
  const original = allText(tree);
  rehypeReviewHighlights({ comments: [comment("one", "Olá mundo 🚀")] })(tree);
  assert.equal(allText(tree), original);
  assert.equal(marks(tree).map(allText).join(""), "Olá\tmundo\n\n🚀");
  assert.ok(marks(tree).every((mark) => mark.properties.dataReviewId === "one"));
  assert.ok(marks(tree).every((mark) => mark.properties.tabIndex === 0));
});

test("overlapping and adjacent comments retain all navigation identities", () => {
  const tree = { type: "root", children: [element("p", [text("alpha beta gamma")])] };
  rehypeReviewHighlights({ comments: [comment("one", "alpha beta"), comment("two", "beta gamma"), comment("three", "gamma")] })(tree);
  assert.equal(allText(tree), "alpha beta gamma");
  for (const [id, quote] of [["one", "alpha beta"], ["two", "beta gamma"], ["three", "gamma"]]) {
    assert.equal(marks(tree).filter((mark) => mark.properties.dataReviewId === id).map(allText).join(""), quote);
  }
});

test("ambiguous legacy comments stay detached; contextual anchors select one occurrence", () => {
  const tree = { type: "root", children: [element("p", [text("first same second same last")])] };
  rehypeReviewHighlights({ comments: [comment("legacy", "same"), comment("anchored", "same", { start: 18, end: 22, prefix: "second ", suffix: " last" })] })(tree);
  assert.deepEqual(marks(tree).map((mark) => mark.properties.dataReviewId), ["anchored"]);
  assert.equal(allText(tree), "first same second same last");
});

test("empty documents and missing quotes do not manufacture highlights", () => {
  for (const value of ["", " \n\t ", "unrelated"]) {
    const tree = { type: "root", children: [text(value)] };
    rehypeReviewHighlights({ comments: [comment("one", "missing"), comment("empty", "")] })(tree);
    assert.equal(marks(tree).length, 0);
    assert.equal(allText(tree), value);
  }
});

test("Mermaid source remains unchanged and cannot acquire React marks inside its code", () => {
  const code = element("code", [text("graph TD; A-->B")], { className: ["language-mermaid"] });
  const tree = { type: "root", children: [element("pre", [code]), element("p", [text("After diagram")])] };
  rehypeReviewHighlights({ comments: [comment("diagram", "graph TD"), comment("prose", "After diagram")] })(tree);
  assert.equal(marks(code).length, 0);
  assert.deepEqual(marks(tree).map((mark) => mark.properties.dataReviewId), ["prose"]);
});

test("many comments share one text collection pass instead of rereading every node per comment", () => {
  let reads = 0;
  const children = Array.from({ length: 80 }, (_, index) => {
    const value = `Paragraph ${index}. `;
    return element("p", [{ type: "text", get value() { reads += 1; return value; } }]);
  });
  const tree = { type: "root", children };
  rehypeReviewHighlights({ comments: Array.from({ length: 80 }, (_, index) => comment(String(index), `Paragraph ${index}.`)) })(tree);
  assert.ok(reads < 80 * 8, `Text nodes were read ${reads} times`);
  assert.equal(new Set(marks(tree).map((mark) => mark.properties.dataReviewId)).size, 80);
});
