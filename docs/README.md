# Construct documentation

Construct is a local-first desktop knowledge workspace for Markdown created and
used by people and coding agents.

## Start here

- [User guide](user-guide.md) — install and open Construct, add Locations,
  navigate, edit safely, review documents, search knowledge, and inspect OKF
  bundles.
- [CLI and OKF lint](cli.md) — install the standalone executable, validate OKF
  repositories locally or in CI, configure exclusions, and interpret exit
  codes.
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

- [Local retrieval and agent access](proposals/retrieval/README.md) — focused
  RFCs covering OKF compatibility, local indexing, search, graph-aware context,
  MCP, linting, and review integration.

Documents marked **Current** describe accepted behavior. Documents marked
**Proposed** are discussion material and may intentionally disagree with the
application today.
