# Contributing to Construct

Thanks for helping make terminal-first agent workflows easier to navigate.

Construct has a small surface over sensitive local files, so focused changes,
clear product intent, and explicit validation matter more than large rewrites.

## Find or propose work

- Search [existing issues](https://github.com/lfnovo/construct/issues) before
  starting.
- Comment on the relevant issue or open a focused one for a bug or bounded
  improvement.
- Start with a design discussion before changing persistence, security
  boundaries, supported file formats, source mutation, agent permissions, or
  the local-first privacy model.
- Keep proposals compatible with explicit saves, read-only Git, open-ended OKF
  metadata, and files as the source of truth.

Do not include private repositories, user documents, credentials, local absolute
paths, or screenshots containing sensitive content.

## Set up development

Install Node.js 22, `rustup`, Xcode Command Line Tools, and the Tauri platform
prerequisites:

```bash
git clone https://github.com/lfnovo/construct.git
cd construct
npm ci
npm run dev
```

The pinned Rust toolchain installs from `rust-toolchain.toml`.

Read the [development guide](docs/development.md) for focused commands, project
structure, MCP and lint smoke tests, local build output, and manual validation.
Read [architecture.md](docs/architecture.md) before changing module boundaries
or persistence.

## Make a focused change

- Branch from the latest `main`.
- Keep unrelated changes out of the diff.
- Put pure domain logic outside `App.tsx` and add regression tests.
- Validate native inputs and filesystem boundaries in Rust.
- Keep application interface text and user-facing errors in English.
- Update [product-spec.md](docs/product-spec.md) when behavior or a product
  decision changes.
- Update the relevant user guide and [documentation index](docs/README.md) when
  a feature changes.

Do not commit:

- `dist/`;
- `node_modules/`;
- `src-tauri/target/`;
- local environment or application-data files;
- derived indexes;
- user content.

Do not modify generated icons directly. Edit `src-tauri/app-icon.svg` and run:

```bash
npm exec tauri icon src-tauri/app-icon.svg
```

## Validate

Every pull request must pass:

```bash
npm ci
npm run validate
npm run build
```

For a focused iteration:

```bash
# TypeScript and frontend
npm run check
npm run lint:web
npm run test:web

# Rust
npm run format:check
npm run lint:rust
npm run test:rust
```

Run the MCP smoke test when changing the knowledge service or agent contract:

```bash
cargo build --manifest-path src-tauri/Cargo.toml
npm run test:mcp
```

## Open a pull request

The pull request should explain:

- the user-visible outcome;
- why the chosen scope is appropriate;
- important implementation or compatibility decisions;
- automated validation run;
- manual desktop validation performed;
- known limitations or follow-up work.

Add before-and-after screenshots for visual changes using synthetic content.
Keep the working tree free of generated build output.

## Coding and product conventions

- Preserve explicit saves; do not introduce autosave without a product
  decision.
- Keep Git integration read-only.
- Do not introduce remote processing or telemetry without an explicit product,
  privacy, and security decision.
- Treat OKF metadata as open-ended. Never impose a closed taxonomy or
  automatically rewrite a bundle.
- Keep derived indexes disposable and source documents authoritative.
- Scope MCP access through registered, explicit Location allowlists.
- Prefer small module extractions over a broad `App.tsx` rewrite.

## Report security issues privately

Do not open public issues for vulnerabilities involving local file access, path
traversal, document execution, agent data exposure, or private data. Follow
[SECURITY.md](SECURITY.md).
