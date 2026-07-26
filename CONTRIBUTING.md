# Contributing to Construct

Thanks for helping make terminal-first agent workflows easier to navigate.

## Before opening a change

- Search existing issues before starting substantial work.
- Keep proposals aligned with the local-first privacy model.
- Open a discussion issue before changing persistence, security boundaries, or supported file formats.
- Never include project documents, private paths, tokens, or screenshots containing sensitive content.

## Development setup

Install Node.js 22, Xcode Command Line Tools, and `rustup`, then run:

```bash
npm ci
npm run dev
```

The pinned Rust toolchain installs automatically from `rust-toolchain.toml`.

## Validation

Every pull request must pass:

```bash
npm run validate
npm run build
```

`validate` runs ESLint, Clippy, TypeScript, Node tests, Rust tests, Rust formatting checks, and the web production build.

## Pull requests

- Keep the scope focused and explain the user-visible outcome.
- Add a regression test when changing pure logic.
- Update the product specification when behavior changes.
- Include before/after screenshots for visual changes.
- Note any manual macOS validation performed.
- Leave the working tree free of generated build output.

## Coding conventions

- User-facing interface and errors are written in English.
- Prefer typed domain helpers over adding more orchestration to `App.tsx`.
- Keep Tauri commands narrow and validate native inputs in Rust.
- Use explicit saves and conflict states for file writes.
- Do not introduce remote processing or telemetry without an explicit product and privacy decision.
- Do not assign semantic meaning to OKF type colors.

## Reporting security issues

Do not open public issues for vulnerabilities involving local file access, path traversal, content execution, or private data. Follow [SECURITY.md](SECURITY.md).
