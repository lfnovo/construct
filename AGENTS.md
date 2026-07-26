# Agent guide

Construct is a local-first Tauri desktop application for reading, editing, and exploring Markdown knowledge created by coding agents.

## Ground rules

- Keep the application UI and user-facing errors in English.
- Keep Git integration read-only.
- Preserve explicit saves; do not introduce autosave without a product decision.
- Treat OKF metadata as open-ended. Do not impose a closed taxonomy or rewrite bundles automatically.
- Update `docs/product-spec.md` when behavior or product decisions change.
- Preserve user state and provide migrations when changing the Tauri identifier or persisted schema.

## Architecture

- `src/App.tsx`: workspace orchestration and desktop interaction state.
- `src/MarkdownPreview.tsx`: sanitized Markdown, Mermaid, and link handling.
- `src/CodeEditor.tsx`: CodeMirror source editor.
- `src/okf.ts`: pure OKF parsing and link helpers.
- `src/history.ts`: history identity and deduplication.
- `src/KnowledgeGraph.tsx`: local OKF graph visualization.
- `src-tauri/src/lib.rs`: filesystem, watcher, persistence, Git, and native shell commands.

Read `docs/architecture.md` before changing module boundaries or persistence.

## Required validation

```bash
npm ci
npm run validate
npm run build
```

For a focused TypeScript change, use `npm run check`, `npm run lint:web`, and `npm run test:web`. For Rust, use `npm run format:check`, `npm run lint:rust`, and `npm run test:rust`.

## Change discipline

- Keep changes scoped and add regression tests for pure logic.
- Do not commit `dist/`, `node_modules/`, `src-tauri/target/`, local environment files, or user content.
- Do not modify generated icons directly; edit `src-tauri/app-icon.svg` and regenerate them with `npm exec tauri icon src-tauri/app-icon.svg`.
- Keep the working tree clean after completed work.
