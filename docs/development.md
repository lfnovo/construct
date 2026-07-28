# Development guide

**Status:** Current

This guide covers local development and validation. Read
[CONTRIBUTING.md](../CONTRIBUTING.md) before preparing a change and
[architecture.md](architecture.md) before changing module boundaries,
persistence, filesystem authority, or the local service.

## Toolchain

The primary development environment is macOS.

Required:

- Node.js 22, selected by `.nvmrc`;
- npm;
- `rustup`;
- the Rust toolchain pinned in `rust-toolchain.toml`;
- Xcode Command Line Tools;
- platform prerequisites for Tauri 2.

On Windows, install the Tauri prerequisites and Visual Studio C++ build tools.
The desktop workspace and stateless linter compile there, but local indexing
and MCP are currently Unix-only.

## Set up the repository

```bash
git clone https://github.com/lfnovo/construct.git
cd construct
npm ci
rustup show
```

`rustup` reads `rust-toolchain.toml` and installs the pinned compiler, Clippy,
and rustfmt when necessary.

## Run Construct

Run the Tauri desktop application with the Vite development server:

```bash
npm run dev
```

Run only the web frontend when working on presentation that does not need native
commands:

```bash
npm run dev:web
```

Native workflows such as filesystem access, Git inspection, the local index,
and MCP require the Tauri application.

## Build

Build the frontend:

```bash
npm run build:web
```

Build the release-mode desktop application:

```bash
npm run build
```

On macOS, the app bundle is:

```text
src-tauri/target/release/bundle/macos/Construct.app
```

Build only the Rust executable:

```bash
cargo build --manifest-path src-tauri/Cargo.toml
cargo build --release --manifest-path src-tauri/Cargo.toml
```

The debug executable is `src-tauri/target/debug/construct`. The same executable
dispatches desktop, `service`, `mcp serve`, and `okf lint` modes.

Build the isolated linter used by the Linux release and CI Action:

```bash
cargo build \
  --manifest-path src-tauri/Cargo.toml \
  --no-default-features \
  --features okf-cli
```

This target intentionally cannot start the desktop, service, or MCP. It exists
to keep the stateless linter free from Tauri, SurrealDB, and platform UI
dependencies while reusing the exact parser and report code.

Do not commit `dist/`, `node_modules/`, `src-tauri/target/`, local environment
files, application data, indexes, or user content.

## Validation

Run the complete source validator:

```bash
npm run validate
```

Run the release-mode app build separately:

```bash
npm run build
```

The complete validation expected before a pull request is:

```bash
npm ci
npm run validate
npm run build
```

`validate` checks the documentation index and local links, then runs web and
Rust linting, TypeScript checking, Node and Rust tests, Rust formatting checks,
and the production web build.

For documentation-only work, run the lightweight check directly. It uses only
Node.js and does not install application dependencies or compile Rust:

```bash
npm run check:docs
```

Pull request CI classifies changed paths before starting expensive jobs.
Documentation-only changes run the documentation check; frontend changes run
the web checks on Linux; native changes run Rust checks on macOS and Windows.
The release-mode macOS bundle is reserved for bundle-critical pull request
changes and code merged to `main`.

### Focused web checks

```bash
npm run check
npm run lint:web
npm run test:web
```

### Focused Rust checks

```bash
npm run format:check
npm run lint:rust
npm run test:rust
```

### MCP smoke test

Build the debug binary, then run:

```bash
cargo build --manifest-path src-tauri/Cargo.toml
npm run test:mcp
```

Pass another executable path directly when needed:

```bash
node scripts/mcp-smoke.mjs /absolute/path/to/construct
```

The smoke test creates synthetic temporary Locations and removes them after the
run.

### OKF linter smoke checks

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- \
  okf lint tests/fixtures/okf/v02

cargo run --manifest-path src-tauri/Cargo.toml -- \
  okf lint tests/fixtures/okf/v02 --format json
```

Use an existing fixture path from `tests/fixtures` when adding or changing
parser behavior. The Rust test suite contains the authoritative fixture
coverage.

## Project structure

| Path | Responsibility |
| --- | --- |
| `src/App.tsx` | Workspace orchestration, Locations, panes, tabs, and commands |
| `src/CodeEditor.tsx` | CodeMirror Source editor |
| `src/VisualEditor.tsx` | Rich Markdown editing |
| `src/ReviewEditor.tsx` | Selection-based comments and agent handoff |
| `src/SearchWorkspace.tsx` | Local knowledge search and context selection |
| `src/HealthWorkspace.tsx` | Interactive OKF lint findings |
| `src/MarkdownPreview.tsx` | Sanitized Markdown, Mermaid, images, and links |
| `src/*.ts` | Pure frontend domain helpers and typed native contracts |
| `src-tauri/src/lib.rs` | Shared feature boundary and command dispatch |
| `src-tauri/src/desktop.rs` | Tauri commands, filesystem, watcher, state, and Git |
| `src-tauri/src/okf.rs` | Shared tolerant OKF parser |
| `src-tauri/src/okf_lint.rs` | Stateless CLI validation |
| `src-tauri/src/okf_policy.rs` | `.constructignore` conformance policy |
| `src-tauri/src/index.rs` | Per-Location SurrealDB/SurrealKV retrieval index |
| `src-tauri/src/knowledge.rs` | Local knowledge service and client |
| `src-tauri/src/mcp.rs` | Read-only MCP stdio adapter |
| `tests/` | Frontend logic tests and shared fixtures |
| `docs/` | User, contributor, product, architecture, and proposal docs |

Keep pure logic outside `App.tsx` so it remains testable without a webview.
Keep privileged inputs and path validation in Rust.

## Product invariants

Changes must preserve these defaults unless an explicit product decision says
otherwise:

- Markdown files are the source of truth.
- Saves are explicit; there is no autosave.
- Git integration is read-only.
- Document content stays local.
- OKF metadata is open-ended and never rewritten automatically.
- Derived indexes can be rebuilt without changing source files.
- Agent access is read-only and allowlisted.
- User-facing interface text and errors are in English.

Update [product-spec.md](product-spec.md) when behavior or product decisions
change.

## Icons

Do not edit generated icons directly. Update:

```text
src-tauri/app-icon.svg
```

Then regenerate:

```bash
npm exec tauri icon src-tauri/app-icon.svg
```

Review the generated diff and do not include unrelated assets.

## Manual validation

For user-visible desktop changes, exercise the relevant journey in the app:

- add and restore a Location;
- open Preview, Edit, Review, Source, and Diff;
- save explicitly and simulate an external edit conflict;
- quick-open and knowledge search;
- OKF Explore List, Graph, and Health;
- index rebuild and failure recovery;
- dark and light themes;
- Finder and external-link actions;
- local MCP startup when the change touches retrieval.

Use synthetic repositories and documents in screenshots or bug reports. Do not
publish private paths or user content.

## Release work

Normal pull requests should not create tags or publish binaries. Maintainers
should follow [releasing.md](releasing.md), which covers version alignment,
draft artifacts, checksums, smoke tests, signing boundaries, and publication.
