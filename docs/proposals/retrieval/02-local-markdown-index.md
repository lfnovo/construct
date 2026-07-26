# RFC 02 — Local Markdown index

**Status:** Proposed

**Decision owner:** Native architecture

## Question

How should Construct make every registered Markdown location incrementally
searchable while preserving local-first behavior and explicit saves?

## Proposed decision

Construct maintains an application-owned SQLite/FTS5 index for all registered
Markdown files. OKF documents receive additional structured metadata from
[RFC 01](01-okf-compatibility.md), but basic indexing does not require OKF.

The index is:

- derived from saved files;
- stored outside user repositories;
- disposable and rebuildable;
- incrementally updated by the existing filesystem watcher;
- shared by desktop retrieval and future local agent adapters.

## Why index all Markdown

The original Construct workflow is broader than OKF. Plans, specifications,
reports, and agent context files benefit from content search even when they have
no frontmatter.

A generic Markdown core with OKF enrichment gives one coherent product:

| Capability | Ordinary Markdown | OKF Markdown |
| --- | --- | --- |
| Filename and path | Yes | Yes |
| Body and heading search | Yes | Yes |
| Markdown link graph | Yes | Yes |
| Type and tag filters | When present as generic metadata | Structured OKF behavior |
| Lifecycle and source metadata | No special meaning | Structured OKF behavior |
| Bundle-aware context | Location and path structure | Location, path, index, and OKF metadata |

## Source-of-truth boundary

- The shared index represents saved files only.
- Unsaved tab buffers remain in React and are never written into the persistent
  index.
- The UI may search an active unsaved buffer separately if it labels that
  result.
- Index refreshes never trigger a save or resolve a conflict.
- Deleting or rebuilding the index never changes a registered file.

## Storage model

The recommended starting point is one database per Construct profile with
location isolation inside the schema. This enables cross-location search while
keeping schema migrations and process concurrency in one place.

The physical choice must still be benchmarked against a database per location.
Whichever model is selected must support removing all cached data for one
location without affecting its files.

The database should use:

- transactional schema changes;
- explicit schema and indexer versions;
- parameterized SQL;
- WAL mode when supported;
- bounded busy timeouts;
- platform-appropriate user-only permissions;
- integrity checks and rebuildable generations.

## Logical records

### Location

- stable Construct location ID;
- root path, stored only inside the private local database;
- display name;
- OKF detection mode and version when applicable;
- current scan generation and state;
- last successful reconciliation;
- indexer version and error summary.

### Document

- stable local identity;
- location ID and normalized relative path;
- technical role;
- title and description when derivable;
- saved Markdown body or indexed text;
- normalized metadata plus complete typed frontmatter;
- content hash, size, and filesystem modification time;
- parse state and active generation.

### Derived records

- headings with level, order, anchor, and source range;
- tags with exact and normalized values;
- links with raw target, resolution, fragment, origin, and source range;
- findings with stable code and severity;
- an FTS5 row for title, description, tags, headings, relative path, and body.

Semantic chunks and embeddings, if ever added, live in separately versioned
tables that can be deleted without affecting lexical retrieval.

## Indexable content

The index includes:

- visible Markdown body;
- headings;
- relative path;
- supported frontmatter fields;
- OKF metadata when the document belongs to a bundle.

The index excludes:

- YAML delimiters and raw metadata serialization;
- `construct-review:v1` payloads from normal body ranking;
- generated preview HTML;
- Mermaid render output;
- content reached through external URLs;
- unsaved buffers;
- files outside registered locations.

Review state is handled separately by
[RFC 06](06-review-integration.md).

## Incremental update flow

1. The native watcher emits candidate file changes.
2. The coordinator debounces duplicate events.
3. Size and modification time identify cheap candidates.
4. A content hash confirms whether saved bytes changed.
5. Parsing occurs outside the UI thread.
6. Document, metadata, headings, links, findings, and FTS rows update in one
   transaction.
7. Deletes remove the document and repair affected link resolution.
8. Renames may preserve continuity when known but remain correct as
   delete-plus-create.
9. A bounded event tells the UI which index state changed.

A periodic reconciliation compares registered files with indexed identities to
recover from missed watcher events.

## Generations and recovery

An initial or full rebuild is progressive:

- path identities become queryable first;
- parsed metadata and lexical content appear as indexing proceeds;
- results identify an incomplete generation;
- the previous healthy generation remains usable until replacement is ready.

A corrupt or incompatible index is quarantined or replaced after an integrity
check. Failure falls back to current file navigation.

## Privacy and security

- No content, query, path, count, or metric is sent remotely.
- Absolute paths stay inside native storage and commands that require them.
- Normal UI/CLI/MCP result identities are location ID plus relative path.
- Path resolution happens in Rust and rejects escapes from a registered root.
- Symlink traversal remains disabled unless a separate security decision
  changes it.
- Logs contain operation IDs and counts, not document content.
- YAML and Markdown parsing have explicit size and nesting limits.

## Performance targets

Provisional targets for a completed 10,000-document index:

| Operation | Target |
| --- | --- |
| Warm top-20 lexical query | p95 under 150 ms |
| One saved-file update visible | within 2 seconds after debounce |
| Backlinks for one document | p95 under 100 ms |
| UI work caused by indexing | no main-thread task over 50 ms |

The benchmark must also record cold start, full rebuild, peak memory, database
size, and interrupted indexing.

## Non-goals

- Autosave.
- Storing historical document snapshots.
- Remote search or hosted indexing.
- Mandatory embeddings.
- Arbitrary SQL access.
- Treating the database as a new source of truth.
- Indexing arbitrary filesystem paths outside registered locations.

## Acceptance criteria

- Ordinary Markdown and OKF documents are searchable through one index.
- Deleting the database and rebuilding produces equivalent saved-corpus
  results without changing source files.
- A normal one-file save updates only affected records.
- Reconciliation repairs missed create, change, rename, and delete events.
- Readers never observe a partially committed generation.
- Index failure does not prevent opening or editing files.
- Removing a location deletes its cached records without deleting its files.
- `construct-review:v1` text does not affect normal FTS rank or graph edges.

## Open decisions

- SQLite crate and how FTS5 availability is guaranteed on each platform.
- One profile database versus one database per location.
- Whether full saved bodies or only FTS content are retained.
- Cache retention and user-visible deletion controls.
- Stable document identity across renames.
- How partial index state is represented in Tauri commands.

## Dependencies and handoff

The index provides data to the
[knowledge search experience](03-knowledge-search-experience.md),
[graph and context retrieval](04-graph-context-retrieval.md), and
[local agent access](05-local-agent-access.md).
