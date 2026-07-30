# Changelog

All notable changes to Construct will be documented in this file.

The project follows [Semantic Versioning](https://semver.org/) once public releases begin.

## [Unreleased]

## [0.1.6] - 2026-07-30

### Added

- Added a reviewed external-terminal handoff for registered Location roots and
  document directories, with remembered adapters for supported macOS and
  Windows terminals.
- Added anchored Review highlights with bidirectional passage/comment
  navigation, keyboard access, and an explicit detached state for changed or
  ambiguous passages.

### Changed

- Preserved per-mode scroll state and transferred the visible semantic passage
  when switching between Preview, Edit, Review, Source, and Diff.
- Extended new `construct-review:v1` comments with optional contextual locators
  while keeping existing quote-only comments compatible.

### Fixed

- Prevented the Windows desktop application from opening a companion console
  window while preserving normal console behavior for CLI commands.

## [0.1.5] - 2026-07-28

### Added

- Added a standalone Linux x64 OKF linter archive built on Ubuntu 22.04 and
  included in the release `SHA256SUMS` manifest.
- Added a release-pinned GitHub Action that verifies the selected CLI archive,
  caches it, runs JSON linting, publishes job summaries and source annotations,
  and preserves the CLI exit code.

### Changed

- Isolated the stateless OKF parser and linter behind a lightweight `okf-cli`
  build feature that excludes Tauri, SurrealDB, the local service, and MCP.
- Added Linux CLI compilation and public exit-code smoke tests to pull request
  CI.

## [0.1.4] - 2026-07-28

### Added

- Added bounded local JSONL diagnostics for desktop/service startup, index
  reconciliation, full-text schema and analyzer failures, field-level probes,
  and lexical fallback recovery without logging queries or repository content.

### Changed

- Limited the local lexical fallback to explicit full-text failures or
  demonstrably unhealthy search schema; a valid empty result remains empty.

## [0.1.3] - 2026-07-28

### Fixed

- Kept desktop and MCP knowledge search working when the embedded full-text
  indexes are unavailable by falling back to a
  batched, local lexical scan of the active Location generation.

## [0.1.2] - 2026-07-27

### Changed

- Made pull request CI path-aware so documentation-only changes avoid
  dependency installation, Rust compilation, and desktop bundling.
- Split web, native, Windows, and macOS bundle validation into focused jobs
  behind a stable aggregate CI gate.
- Enabled the isolated local knowledge index and read-only MCP server on
  Windows through authenticated local named-pipe IPC.

### Fixed

- Normalized Windows canonical path identities so files returned with the
  `\\?\` prefix still open inside their registered Location.

### Documentation

- Clarified public pre-release installation on Windows, including the correct
  NSIS asset, checksum verification, SmartScreen handling, and the distinction
  between private draft URLs, desktop installers, CLI archives, and source
  archives.
- Reorganized the repository around clear user, CLI, MCP, contributor, product,
  architecture, security, and release journeys.
- Added a complete first-run and workflow guide for Locations, Markdown
  editing, Review, search, OKF Health, local data, and troubleshooting.
- Added standalone CLI and local MCP guides with CI, allowlist, privacy, and
  tool-reference examples.
- Reconciled the product specification with the implemented search, indexing,
  OKF lint, MCP, and Windows preview boundaries.
- Updated issue and pull request templates for the current product areas and
  cross-platform preview.

## [0.1.1] - 2026-07-27

### Added

- Local-first macOS workspace for recursively discovered Markdown files.
- Tabs, split panes, editable source, rendered preview, Mermaid, and local images.
- Rich Markdown editing with contextual formatting and lossless YAML frontmatter preservation.
- Persistent document review comments with agent-ready clipboard handoff and exact cleanup.
- Filesystem monitoring with deduplicated recent history.
- Read-only Git status and diff.
- Open Knowledge Format detection, metadata inspection, navigation, filtering, and graph exploration.
- Stateless `construct okf lint` validation with deterministic text/JSON output and CI exit codes.
- Explore Health view with lint summaries, scopes, filters, source navigation, explicit refresh, and agent handoff.
- Repository-owned `.constructignore` rules that skip OKF conformance checks while preserving internal-link resolution.
- Tag-driven GitHub Release candidates with macOS DMGs, a Windows NSIS installer, standalone CLI archives, and SHA-256 checksums.
- Light and dark themes, keyboard shortcuts, Finder integration, and workspace restoration.

### Changed

- Renamed the application from Agent Context to Construct.

### Fixed

- Windows release compilation now excludes the Unix-only local knowledge
  transport and reports its unsupported-platform boundary explicitly.

### Security

- Markdown preview is sanitized and file access is limited to user-selected locations.
