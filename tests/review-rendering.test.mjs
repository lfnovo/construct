import assert from "node:assert/strict";
import test from "node:test";
import { JSDOM } from "jsdom";

const dom = new JSDOM("<!doctype html><html><body></body></html>", { url: "http://localhost/", pretendToBeVisual: true });
for (const name of ["window", "document", "navigator", "Node", "NodeFilter", "Element", "HTMLElement", "HTMLTextAreaElement", "Event", "MouseEvent", "KeyboardEvent"]) {
  Object.defineProperty(globalThis, name, { configurable: true, value: dom.window[name] });
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
HTMLElement.prototype.scrollIntoView = function () { this.dataset.scrolled = "true"; };
window.confirm = () => true;

const React = await import("react");
const { act, createElement: h, useState } = React;
const { createRoot } = await import("react-dom/client");
const { ReviewEditor } = await import("../src/ReviewEditor.tsx");
const { ReviewDraftProvider } = await import("../src/ReviewDraft.tsx");
const { DocumentErrorBoundary } = await import("../src/DocumentErrorBoundary.tsx");
const { DocumentModeSurface } = await import("../src/DocumentModeSurface.tsx");
const { MarkdownPreview } = await import("../src/MarkdownPreview.tsx");
const { setReviewComments, splitReviewDocument } = await import("../src/review.ts");
const { api } = await import("../src/api.ts");
const { captureReviewAnchor } = await import("../src/reviewDom.ts");
const noop = () => {};
const note = (id, quote) => ({ id, quote, comment: `Note ${id}`, createdAt: "2026-09-02T12:00:00Z" });

async function click(element) {
  assert.ok(element, "expected an element to click");
  await act(() => element.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true })));
}

async function mountReview(body, comments = [], overrides = {}, wrap = (child) => child) {
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  const state = { content: setReviewComments(body, comments).content, changes: 0 };
  function Harness() {
    const [content, setContent] = useState(state.content);
    state.replace = setContent;
    state.content = content;
    return wrap(h(ReviewEditor, { content, sourcePath: "/synthetic.md", relativePath: "synthetic.md", readOnly: false,
      onChange: (next) => { state.changes += 1; setContent(next); }, onOpenInternal: (path) => noop(path), onRequestSource: noop, onNotify: noop, ...overrides }));
  }
  await act(() => root.render(h(React.StrictMode, null, h(ReviewDraftProvider, null, h(Harness)))));
  return { container, state, async close() { await act(() => root.unmount()); container.remove(); } };
}

async function selectParagraph(container) {
  const paragraph = container.querySelector(".markdown-preview p");
  const range = document.createRange();
  range.selectNodeContents(paragraph);
  window.getSelection().removeAllRanges();
  window.getSelection().addRange(range);
  await act(() => paragraph.dispatchEvent(new MouseEvent("mouseup", { bubbles: true })));
  return container.querySelector("textarea");
}

async function type(textarea, value) {
  await act(() => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set.call(textarea, value);
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

test("typing a review does not replace Markdown nodes or change the document buffer", async () => {
  const body = Array.from({ length: 200 }, (_, index) => `Paragraph ${index}: **bold**, [link](https://example.org) and \`code\`.\n\n`).join("");
  const view = await mountReview(body);
  try {
    const textarea = await selectParagraph(view.container);
    const preview = view.container.querySelector(".markdown-preview");
    const nodes = [...preview.querySelectorAll("p, a, code, strong")];
    const mutations = [];
    const observer = new dom.window.MutationObserver((records) => mutations.push(...records));
    observer.observe(preview, { childList: true, subtree: true, characterData: true });
    for (const value of ["n", "ne", "new", "new note"]) await type(textarea, value);
    observer.disconnect();
    assert.equal(textarea.value, "new note");
    assert.equal(view.state.content, body);
    assert.equal(view.state.changes, 0);
    assert.deepEqual([...preview.querySelectorAll("p, a, code, strong")], nodes);
    assert.equal(mutations.length, 0, "comment input must not rerender the Markdown subtree");
  } finally { await view.close(); }
});

test("adding, removing and clearing comments on links/code preserves the view, scroll and source bytes", async () => {
  const body = "---\r\ntitle: Synthetic\r\n---\r\nAlpha [link](https://example.org) and `code` omega.\r\n";
  const quote = "Alpha link and code omega.";
  const view = await mountReview(body, [note("first", quote)]);
  try {
    const preview = view.container.querySelector(".markdown-preview");
    const code = preview.querySelector("code");
    const link = preview.querySelector("a");
    preview.scrollTop = 120;
    const textarea = await selectParagraph(view.container);
    await type(textarea, "A second comment");
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Add comment"));
    assert.equal(splitReviewDocument(view.state.content).comments.length, 2);
    assert.equal(view.container.querySelectorAll(".review-comment").length, 2);
    assert.equal(view.container.querySelector(".markdown-preview"), preview);
    assert.equal(preview.querySelector("code"), code);
    assert.equal(preview.querySelector("a"), link);
    assert.equal(preview.scrollTop, 120);
    assert.equal(preview.textContent, quote);
    await click(view.container.querySelector(".review-comment button"));
    assert.equal(splitReviewDocument(view.state.content).comments.length, 1);
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Clear all comments"));
    assert.equal(view.state.content, body);
    assert.equal(preview.querySelectorAll("mark").length, 0);
    assert.equal(preview.scrollTop, 120);
  } finally { await view.close(); }
});

test("a refreshed body with existing comments never removes React-owned nodes imperatively", async () => {
  const view = await mountReview("Alpha **bold** omega.\n", [note("one", "Alpha bold omega.")]);
  try {
    await act(() => view.state.replace(setReviewComments("Alpha **bold** omega.\n", [note("one", "Alpha bold omega."), note("two", "bold")]).content));
    await act(() => view.state.replace(setReviewComments("**bold** omega.\n", [note("one", "Alpha bold omega."), note("two", "bold")]).content));
    assert.equal(view.container.querySelector(".markdown-preview").textContent, "bold omega.");
    assert.equal(view.container.querySelectorAll(".review-comment.detached").length, 1);
    assert.equal(view.container.querySelector("mark").textContent, "bold");
  } finally { await view.close(); }
});

test("overlapping highlights navigate by click and keyboard without following annotated links", async () => {
  const previousOpen = api.openExternalUrl;
  let opened = 0;
  api.openExternalUrl = async () => { opened += 1; };
  const view = await mountReview("Alpha [link](https://example.org) omega.\n", [note("one", "Alpha link omega."), note("two", "link")]);
  try {
    const nested = view.container.querySelector('mark[data-review-id="two"]');
    await click(nested);
    assert.equal(opened, 0);
    assert.ok(view.container.querySelectorAll(".review-comment")[1].classList.contains("active"));
    const outer = view.container.querySelector('mark[data-review-id="one"]');
    await act(() => outer.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));
    assert.ok(view.container.querySelector(".review-comment").classList.contains("active"));
    await click(view.container.querySelectorAll(".review-comment")[1]);
    assert.equal(nested.dataset.scrolled, "true");
    assert.equal(document.activeElement, nested);
  } finally { api.openExternalUrl = previousOpen; await view.close(); }
});

test("render failures retain the parent buffer and offer Source, retry, and content-free diagnostics", async () => {
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  const previousReport = api.reportDocumentRenderFailure;
  const previousError = console.error;
  const reports = [];
  api.reportDocumentRenderFailure = async (...args) => { reports.push(args); };
  console.error = noop; // React prints the intentionally thrown test exception.
  let shouldThrow = true;
  const buffer = "Unsaved synthetic Markdown";
  function FailingView() { if (shouldThrow) throw new Error("Private synthetic text must not be logged"); return h("p", null, "Recovered"); }
  function Harness() {
    const [source, setSource] = useState(false);
    return source ? h("textarea", { readOnly: true, value: buffer }) : h(DocumentErrorBoundary, { mode: "review", onRequestSource: () => setSource(true) }, h(FailingView));
  }
  try {
    await act(() => root.render(h(Harness)));
    assert.ok(container.querySelector('[role="alert"]'));
    assert.deepEqual(reports, [["review"]]);
    shouldThrow = false;
    await click([...container.querySelectorAll("button")].find((button) => button.textContent === "Retry view"));
    assert.equal(container.textContent, "Recovered");
    shouldThrow = true;
    await act(() => root.render(h(Harness)));
    await click([...container.querySelectorAll("button")].find((button) => button.textContent === "Open Source"));
    assert.equal(container.querySelector("textarea").value, buffer);
  } finally {
    await act(() => root.unmount()); container.remove();
    api.reportDocumentRenderFailure = previousReport; console.error = previousError;
  }
});

test("review markers are trusted decorations, not executable or document-supplied attributes", async () => {
  const container = document.createElement("div");
  const root = createRoot(container);
  try {
    await act(() => root.render(h(MarkdownPreview, { content: '<mark data-review-id="spoof" onclick="alert(1)">safe</mark><script>alert(1)</script>', sourcePath: "/synthetic.md", onOpenInternal: noop, reviewComments: [note('id" <safe>', "safe")] })));
    assert.equal(container.querySelector("script"), null);
    assert.equal(container.querySelector("[onclick]"), null);
    assert.equal(container.querySelector('[data-review-id="spoof"]'), null);
    assert.equal(container.querySelector("mark[data-review-id]").dataset.reviewId, 'id" <safe>');
  } finally { await act(() => root.unmount()); }
});

test("selection offsets use Markdown prose rather than generated diagram or image-error labels", () => {
  const container = document.createElement("article");
  container.innerHTML = '<p>same </p><div class="mermaid" data-review-generated="true">generated label</div><span class="missing-image" data-review-generated="true">Image unavailable</span><p>same end</p>';
  const selected = container.querySelectorAll("p")[1].firstChild;
  const range = document.createRange();
  range.setStart(selected, 0);
  range.setEnd(selected, 4);
  const anchor = captureReviewAnchor(container, range, "same");
  assert.equal(anchor.start, 5);
  assert.equal(anchor.prefix, "same ");
  assert.equal(anchor.suffix, " end");
});

test("source render failure offers retry rather than a no-op Open Source action", async () => {
  const container = document.createElement("div");
  const root = createRoot(container);
  const previousReport = api.reportDocumentRenderFailure;
  const previousError = console.error;
  api.reportDocumentRenderFailure = async () => {};
  console.error = noop;
  function BrokenSource() { throw new Error("Synthetic Source failure"); }
  try {
    await act(() => root.render(h(DocumentErrorBoundary, { mode: "source", onRequestSource: noop }, h(BrokenSource))));
    assert.deepEqual([...container.querySelectorAll("button")].map((button) => button.textContent), ["Retry view"]);
    assert.match(container.textContent, /Use Save to save it/);
    assert.doesNotMatch(container.textContent, /Open Source/);
  } finally {
    await act(() => root.unmount());
    api.reportDocumentRenderFailure = previousReport; console.error = previousError;
  }
});

test("an outer panel failure preserves the final keystroke and selection through retry", async () => {
  const previousReport = api.reportDocumentRenderFailure;
  const previousError = console.error;
  api.reportDocumentRenderFailure = async () => {};
  console.error = noop;
  function FailingPanel({ children }) {
    const [failed, setFailed] = useState(false);
    if (failed) throw new Error("Synthetic panel failure after input");
    return h("div", { onInput: (event) => {
      if (event.target.value.endsWith("!")) setFailed(true);
    } }, children);
  }
  const body = "Alpha **bold** omega.\n";
  const view = await mountReview(body, [], {}, (child) => h(DocumentErrorBoundary,
    { mode: "review", onRequestSource: noop }, h(FailingPanel, null, child)));
  try {
    await type(await selectParagraph(view.container), "Preserve this final character!");
    assert.ok(view.container.querySelector('[role="alert"]'));
    assert.equal(view.container.querySelector("textarea"), null, "the entire panel was unmounted");
    assert.equal(view.state.content, body, "composition must not update the document buffer");
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Retry view"));
    assert.equal(view.container.querySelector("textarea").value, "Preserve this final character!");
    assert.equal(view.container.querySelector(".review-composer blockquote").textContent, "Alpha bold omega.");
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Add comment"));
    assert.equal(splitReviewDocument(view.state.content).comments[0].comment, "Preserve this final character!");
  } finally {
    await view.close(); api.reportDocumentRenderFailure = previousReport; console.error = previousError;
  }
});

test("raw HTML cannot impersonate renderer-owned anchor exclusions", async () => {
  for (const className of ["mermaid", "mermaid-error", "missing-image"]) {
    const body = `<div class="${className}" data-review-generated="true">same</div>\n\nsame`;
    const view = await mountReview(body);
    try {
      const preview = view.container.querySelector(".markdown-preview");
      assert.equal(preview.querySelectorAll("[data-review-generated]").length, 0, "the sanitizer rejects forged markers");
      // Even if presentation classes are added later, they cannot exclude prose.
      preview.querySelector("div").className = className;
      await type(await selectParagraph(view.container), "Only the second same");
      await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Add comment"));
      assert.equal(preview.querySelector("div").querySelectorAll("mark").length, 0);
      assert.equal(preview.querySelector("p mark").textContent, "same");
      assert.equal(view.container.querySelectorAll(".review-comment.detached").length, 0);
    } finally { await view.close(); }
  }
});

test("the current tab draft survives a Source round trip and Cancel clears recovery state", async () => {
  let source = false;
  const body = "Synthetic passage.\n";
  const view = await mountReview(body, [], {}, (child) => source ? h("p", null, "Synthetic Source mode") : child);
  try {
    await type(await selectParagraph(view.container), "Temporary note");
    source = true;
    await act(() => view.state.replace(`${body}\n`));
    assert.equal(view.container.querySelectorAll(".review-workspace").length, 0);
    source = false;
    await act(() => view.state.replace(body));
    assert.equal(view.container.querySelector("textarea").value, "Temporary note");
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Cancel"));
    source = true;
    await act(() => view.state.replace(`${body}\n`));
    source = false;
    await act(() => view.state.replace(body));
    assert.equal(view.container.querySelectorAll(".review-composer").length, 0);
    assert.equal(view.state.content, body);
  } finally { await view.close(); }
});

test("a comment on a repeated raw HTML table cell highlights the selected cell", async () => {
  const body = "<table><tbody>\n<tr>\n<td>same</td>\n<td>same</td>\n</tr>\n</tbody></table>";
  const view = await mountReview(body);
  try {
    const cell = view.container.querySelectorAll("td")[1];
    const range = document.createRange();
    range.selectNodeContents(cell);
    window.getSelection().removeAllRanges();
    window.getSelection().addRange(range);
    await act(() => cell.dispatchEvent(new MouseEvent("mouseup", { bubbles: true })));
    await type(view.container.querySelector("textarea"), "Second cell");
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Add comment"));
    assert.equal(view.container.querySelectorAll("td")[0].querySelector("mark"), null);
    assert.equal(view.container.querySelectorAll("td")[1].querySelector("mark").textContent, "same");
    assert.equal(view.container.querySelectorAll(".review-comment.detached").length, 0);
  } finally { await view.close(); }
});

test("retry keeps the live scroller, pending comment, resolved state and active highlight", async () => {
  const mermaid = (await import("mermaid")).default;
  const previousInitialize = mermaid.initialize;
  const previousRender = mermaid.render;
  const previousReport = api.reportDocumentRenderFailure;
  const previousError = console.error;
  const captures = [];
  api.reportDocumentRenderFailure = async () => {};
  console.error = noop;
  const comments = [note("one", "Alpha bold omega.")];
  const body = "Alpha **bold** omega.\n";
  const view = await mountReview(body, comments, {}, (child) => h(DocumentModeSurface, {
    tabId: "synthetic", mode: "review", consumeRestoreState: () => ({ saved: null, transfer: null }),
    onViewState: (state) => captures.push(state),
  }, child));
  try {
    await act(() => new Promise((resolve) => window.requestAnimationFrame(resolve)));
    const preview = view.container.querySelector(".markdown-preview");
    await click(view.container.querySelector(".review-comment"));
    await type(await selectParagraph(view.container), "Pending note survives");
    mermaid.initialize = () => { throw new Error("Synthetic renderer failure"); };
    await act(() => view.state.replace(setReviewComments(`${body}\n\`\`\`mermaid\ngraph TD; A-->B\n\`\`\`\n`, comments).content));
    assert.ok(view.container.querySelector('[role="alert"]'));
    assert.equal(view.container.querySelector("textarea").value, "Pending note survives");
    mermaid.initialize = noop;
    mermaid.render = async () => ({ svg: "<svg><text>Synthetic diagram</text></svg>" });
    await click([...view.container.querySelectorAll("button")].find((button) => button.textContent === "Retry view"));
    assert.equal(view.container.querySelector('[role="alert"]'), null);
    assert.equal(view.container.querySelector(".markdown-preview"), preview);
    assert.ok(preview.querySelector('mark[data-review-id="one"].active'));
    assert.equal(view.container.querySelectorAll(".review-comment.detached").length, 0);
    assert.equal(view.container.querySelector("textarea").value, "Pending note survives");
    const count = captures.length;
    preview.scrollTop = 90;
    await act(() => preview.dispatchEvent(new Event("scroll")));
    assert.equal(captures.length, count + 1);
    assert.equal(captures.at(-1).scrollTop, 90);
  } finally {
    await view.close();
    mermaid.initialize = previousInitialize; mermaid.render = previousRender;
    api.reportDocumentRenderFailure = previousReport; console.error = previousError;
  }
});
