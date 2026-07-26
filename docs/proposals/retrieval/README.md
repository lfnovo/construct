# Local retrieval and agent access

**Status:** In progress — RFC 01 implemented; RFCs 02–06 proposed

**Date:** 2026-07-26

**Scope:** Product and architecture reasoning only. This proposal does not
authorize implementation.

## Purpose

Construct already makes local Markdown comfortable for people to browse, edit,
review, and explore. The next opportunity is to make the same knowledge easier
for agents to retrieve without repeatedly traversing large directory trees.

The proposed direction is a local retrieval core that:

1. indexes every registered Markdown location;
2. enriches Open Knowledge Format bundles with structured metadata and graph
   behavior;
3. provides explainable lexical search before considering embeddings;
4. builds bounded context packs with explicit provenance;
5. serves the same behavior to the desktop UI and read-only local agent tools.

Markdown files remain authoritative. Every index is derived, disposable, and
must never cause an implicit document write.

## Product position

> Construct is a local Markdown knowledge workspace with an OKF-aware retrieval
> layer shared by people and agents.

This framing avoids two unhelpful extremes:

- Construct should not limit basic retrieval to OKF repositories. Search,
  headings, and Markdown links are useful in every registered location.
- Construct should not flatten OKF into generic text. Types, tags, lifecycle
  metadata, directory indexes, and explicit links make OKF retrieval richer and
  more explainable.

## Documents

The [research baseline](00-research-baseline.md) preserves the ecosystem
evidence and low-risk experiments behind these RFCs.

| RFC | Status | Question it owns | Depends on |
| --- | --- | --- | --- |
| [01 — OKF compatibility](01-okf-compatibility.md) | Implemented | How should Construct consume OKF v0.1, v0.2, and future metadata safely? | Current Markdown boundary rules |
| [02 — Local Markdown index](02-local-markdown-index.md) | Proposed | How should all registered Markdown become incrementally searchable? | RFC 01 for OKF enrichment |
| [03 — Knowledge search experience](03-knowledge-search-experience.md) | Proposed | How should people search content without weakening quick file navigation? | RFC 02 |
| [04 — Graph and context retrieval](04-graph-context-retrieval.md) | Proposed | How should links improve discovery and produce bounded context packs? | RFCs 01–03 |
| [05 — Local agent access](05-local-agent-access.md) | Proposed | How should agents use the same retrieval core through CLI and MCP? | RFCs 02 and 04 |
| [06 — Review integration](06-review-integration.md) | Proposed | How should persisted review comments participate without polluting knowledge? | RFCs 02, 04, and 05 |

## Shared invariants

Every RFC in this set must preserve these rules:

- Processing is local. Construct does not send document contents, filenames,
  paths, queries, or derived indexes to remote services.
- Registered files remain the source of truth.
- Derived state can be deleted and rebuilt without touching source documents.
- Saving remains explicit. Indexing never saves an editor buffer.
- Git integration remains read-only.
- OKF metadata is open-ended; unknown types and fields are preserved.
- Broken links and partial bundles do not make readable documents unavailable.
- UI labels and user-facing errors remain English.
- Normal retrieval responses use location identity plus relative path rather
  than exposing absolute paths.
- Work must degrade back to ordinary file navigation when retrieval is
  unavailable.

## Recommended delivery order

### Phase 0 — Compatibility and measurement

- Build synthetic OKF v0.1/v0.2 fixtures.
- Measure current scans on representative 1,000- and 10,000-file locations.
- Define a small relevance question set from real workflows.
- Decide native YAML, Markdown, and SQLite libraries.

### Phase 1 — Local index and search

- Add tolerant native Markdown and OKF parsing.
- Maintain an incremental SQLite/FTS5 index outside user repositories.
- Ship content search and index status in the desktop app.

### Phase 1.5 — Minimal agent pilot

- Expose local read-only `search`, `get`, and `related` operations.
- Validate whether agents open fewer files and use less context.
- Keep packaging experimental; do not promise a stable public MCP contract yet.

### Phase 2 — Graph and context

- Add bounded graph expansion and related-document explanations.
- Assemble context packs with stable ordering, provenance, and budgets.
- Add `build_context` to the agent pilot.

### Phase 3 — Supported CLI and MCP

- Stabilize installation, permissions, concurrency, limits, and parity tests.
- Document the trust boundary between Construct and external model clients.

### Phase 4 — Optional semantic retrieval

Consider local embeddings only when measured failures show a material gap after
lexical and graph retrieval. Lexical-only operation remains first-class.

## Cross-cutting decisions

These decisions apply to several RFCs and should be resolved before Phase 1:

1. **Index scope:** all registered Markdown, with OKF as enrichment.
2. **Physical storage:** one application-owned database with bundle/location
   isolation, unless measurement shows per-location databases are safer.
3. **Saved state:** the shared index represents saved files only.
4. **Agent timing:** a minimal agent surface follows lexical search rather than
   waiting for every graph and context feature.
5. **Semantic search:** no hosted embeddings and no mandatory local model.
6. **Reviews:** review text is separately represented and excluded from normal
   document ranking and graph edges.

## How to use these RFCs

Each RFC should be discussed and accepted independently. Acceptance means its
decisions can be incorporated into the current product specification and
architecture; it does not automatically authorize every later phase.

Before implementation, the selected RFC needs:

- an explicit product decision;
- dependency and migration choices;
- testable acceptance criteria;
- a scoped implementation plan;
- reconciliation with [the current architecture](../../architecture.md) and
  [product specification](../../product-spec.md).
