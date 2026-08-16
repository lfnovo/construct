# Construct documentation

Construct is a local-first desktop knowledge workspace for Markdown created and
used by people and coding agents.

The current preview combines five connected workflows:

- a multi-Location Markdown workspace with explicit saves, History, and
  read-only Git freshness;
- Preview, rich Edit, anchored Review, raw Source, and Git Diff;
- local full-text retrieval, links, backlinks, and bounded context packs;
- tolerant OKF exploration plus a stateless CLI and CI linter;
- read-only, allowlisted MCP access and safe handoff to supported terminals.

## Start here

- [User guide](user-guide.md) — install and open Construct, add Locations,
  navigate, edit safely, review documents, search knowledge, and inspect OKF
  bundles.
- [CLI and OKF lint](cli.md) — install the executable, open desktop Locations
  or Markdown files from a terminal, validate OKF repositories locally or in
  CI, configure exclusions, and interpret exit codes.
- [Local MCP access](mcp.md) — connect a coding agent to explicitly allowed
  Construct Locations through the read-only stdio server.

## Contribute and operate

- [Contributing](../CONTRIBUTING.md) — contribution expectations, validation,
  pull requests, and security-sensitive changes.
- [Development guide](development.md) — toolchain setup, common commands,
  project structure, tests, and local build output.
- [Release process](releasing.md) — tagged GitHub Releases, app and CLI
  artifacts, checksums, signing gates, and the maintainer checklist.
- [Security policy](../SECURITY.md) — supported versions, vulnerability
  reporting, and the product's security boundaries.
- [Changelog](../CHANGELOG.md) — notable user-visible changes by version.

## Product and architecture

- [Product specification](product-spec.md) — accepted product behavior,
  requirements, and current boundaries.
- [Architecture](architecture.md) — current modules, persistence, filesystem
  authority, retrieval service, and security constraints.

## Design proposals

- [Anchored review experience](proposals/anchored-review.md) — accepted design
  for cross-mode reading continuity, durable passage locators, highlights, and
  bidirectional review navigation.
- [Local retrieval and agent access](proposals/retrieval/README.md) — the RFC
  set and delivery map for the local knowledge layer:
  - [research baseline](proposals/retrieval/00-research-baseline.md);
  - [OKF compatibility](proposals/retrieval/01-okf-compatibility.md);
  - [local Markdown index](proposals/retrieval/02-local-markdown-index.md);
  - [knowledge search experience](proposals/retrieval/03-knowledge-search-experience.md);
  - [graph and context retrieval](proposals/retrieval/04-graph-context-retrieval.md);
  - [local agent access](proposals/retrieval/05-local-agent-access.md);
  - [stateless OKF linter](proposals/retrieval/06-okf-linter.md);
  - [review integration](proposals/retrieval/07-review-integration.md).
- [Terminal integration](proposals/terminal-integration.md) — accepted external
  terminal handoff plus a proposed path to a PTY-backed terminal tab if product
  evidence justifies it.

Documents marked **Current** describe accepted behavior. Documents marked
**Proposed** are discussion material and may intentionally disagree with the
application today.
