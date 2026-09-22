import assert from "node:assert/strict";
import test from "node:test";
import { JSDOM } from "jsdom";

const dom = new JSDOM("<!doctype html><html><body></body></html>", { url: "http://localhost/", pretendToBeVisual: true });
for (const name of ["window", "document", "navigator", "Node", "NodeFilter", "Element", "HTMLElement", "Event"]) {
  Object.defineProperty(globalThis, name, { configurable: true, value: dom.window[name] });
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const React = await import("react");
const { act, createElement: h } = React;
const { createRoot } = await import("react-dom/client");
const { MarkdownPreview } = await import("../src/MarkdownPreview.tsx");
const mermaid = (await import("mermaid")).default;
const noop = () => {};

async function mount(content) {
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  async function render(next) {
    await act(async () => {
      root.render(h(MarkdownPreview, { content: next, sourcePath: "/synthetic.md", onOpenInternal: noop }));
      await Promise.resolve();
      await Promise.resolve();
    });
  }
  await render(content);
  return { container, render, async close() { await act(() => root.unmount()); container.remove(); } };
}

function withMermaidMock(render) {
  const previousInitialize = mermaid.initialize;
  const previousRender = mermaid.render;
  const initialized = [];
  mermaid.initialize = (options) => { initialized.push(options); };
  mermaid.render = render;
  return { initialized, restore() { mermaid.initialize = previousInitialize; mermaid.render = previousRender; } };
}

test("renders independent Mermaid blocks outside code fences and keeps ordinary code", async () => {
  const calls = [];
  const mock = withMermaidMock(async (id, source) => {
    calls.push({ id, source });
    return { svg: `<svg data-render-id="${id}"><text>${source}</text></svg>` };
  });
  const view = await mount("```mermaid\ngraph TD; A-->B\n```\n\n```mermaid\nsequenceDiagram\n  A->>B: hello\n```\n\n```js\nconst answer = 42;\n```\n\n```mermaid-extra\nordinary code\n```");
  try {
    await act(async () => { await Promise.resolve(); });
    assert.equal(view.container.querySelectorAll(".mermaid svg").length, 2);
    assert.equal(new Set(calls.map((call) => call.id)).size, 2);
    assert.equal(view.container.querySelectorAll("pre .mermaid").length, 0);
    assert.equal(view.container.querySelector("pre code.language-js").textContent, "const answer = 42;\n");
    assert.equal(view.container.querySelector("pre code.language-mermaid-extra").textContent, "ordinary code\n");
    assert.equal(mock.initialized.at(-1).securityLevel, "strict");
  } finally { await view.close(); mock.restore(); }
});

test("does not let an older Mermaid render overwrite newer source", async () => {
  const pending = [];
  const mock = withMermaidMock((id, source) => new Promise((resolve) => pending.push({ id, source, resolve })));
  const view = await mount("```mermaid\ngraph TD; Old-->Diagram\n```");
  try {
    await act(async () => { await Promise.resolve(); });
    await view.render("```mermaid\ngraph TD; New-->Diagram\n```");
    assert.equal(pending.length, 2);
    await act(async () => { pending[0].resolve({ svg: "<svg><text>Old diagram</text></svg>" }); await Promise.resolve(); });
    assert.equal(view.container.textContent.includes("Old diagram"), false);
    await act(async () => { pending[1].resolve({ svg: "<svg><text>New diagram</text></svg>" }); await Promise.resolve(); });
    assert.equal(view.container.textContent.includes("New diagram"), true);
  } finally { await view.close(); mock.restore(); }
});

test("isolates invalid Mermaid source and keeps surrounding Markdown visible", async () => {
  const mock = withMermaidMock(async () => { throw new Error("invalid diagram"); });
  const view = await mount("Before\n\n```mermaid\nnot valid\n```\n\nAfter");
  try {
    await act(async () => { await Promise.resolve(); });
    assert.equal(view.container.querySelector(".mermaid-error").textContent, "This Mermaid diagram could not be rendered.\n\nnot valid");
    assert.match(view.container.textContent, /Before/);
    assert.match(view.container.textContent, /After/);
  } finally { await view.close(); mock.restore(); }
});
