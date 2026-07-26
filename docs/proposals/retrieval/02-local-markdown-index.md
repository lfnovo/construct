# RFC 02 — Local Markdown index

**Status:** Accepted — implementation in progress

**Decision owner:** Native architecture

## Question

How should Construct make every registered Markdown location incrementally
searchable while preserving local-first behavior and explicit saves?

## Proposed decision

Construct maintains one application-owned embedded SurrealDB index for each
registered Location. SurrealKV is the initial storage engine; RocksDB remains a
fallback if validation reveals a durability or packaging blocker. OKF documents
receive additional structured metadata from
[RFC 01](01-okf-compatibility.md), but basic indexing does not require OKF.

The index is:

- derived from saved files;
- stored outside user repositories;
- disposable and rebuildable;
- incrementally updated by the existing filesystem watcher;
- physically isolated by Location;
- owned exclusively by a transport-neutral native `IndexService`;
- shared by desktop retrieval and future local agent adapters through typed
  commands rather than direct database access.

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

Each Location has a stable Construct ID and a separate physical database
directory under the platform application-data directory:

```text
Construct/
└── indexes/
    └── <location-id>/
        └── surrealdb/
```

Names and filesystem paths are not storage identities. Moving a Location may
retain its index when its Construct ID is preserved. Removing a Location deletes
only its database directory and never changes source files.

Cross-location search is an explicit fan-out over allowed Location indexes
followed by rank fusion. It is not implemented by mixing all records into one
physical database.

The initial embedded stack is:

- SurrealDB as the document, full-text, graph, and future vector query engine;
- SurrealKV as the pure-Rust local storage engine;
- RocksDB as a documented fallback, not a second active implementation;
- explicit schema and indexer versions;
- transactional mutations and generation activation;
- platform-appropriate user-only permissions;
- integrity probes and complete rebuild from source.

The current Business Source License terms for the SurrealDB dependency must be
recorded in distribution notices and confirmed for Construct's MIT desktop
distribution before release. Construct never exposes raw database-service
functionality to users or agents.

## Index ownership

`IndexService` is the only owner of embedded databases.

- During RFC 02 it runs inside the Tauri native process.
- React accesses it only through typed commands.
- Parsing, schema, ingestion, and retrieval do not depend on React or Tauri.
- One per-Location worker serializes writes and owns that database connection.
- CLI and MCP never open SurrealKV directories directly.
- RFC 05 may host the same service in a sidecar or daemon and add local IPC
  without changing the index schema or retrieval rules.

The first implementation does not require Construct to index while the desktop
is closed. Independent background ownership belongs to RFC 05.

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

- canonical identity: Location ID plus normalized relative path;
- disposable internal continuity UUID;
- location ID and normalized relative path;
- technical role;
- title and description when derivable;
- complete saved Markdown body;
- normalized metadata plus complete typed frontmatter;
- a clean search projection derived from metadata, headings, and body;
- content hash, size, and filesystem modification time;
- parse state and active generation.

The canonical identity is deterministic and survives an index rebuild. When the
watcher identifies a rename with high confidence, the internal continuity UUID
may be preserved for tabs and history. If confidence is insufficient, correctness
wins and the change is represented as delete plus create. Content hashes are
evidence, not identity. Construct never inserts an identity field into user
frontmatter.

### Derived records

- headings with level, order, anchor, and source range;
- tags with exact and normalized values;
- links with raw target, resolution, fragment, origin, and source range;
- findings with stable code and severity;
- a SurrealDB full-text index over the clean search projection.

Semantic chunks and embeddings, if ever added, live in separately versioned
tables that can be deleted without affecting lexical retrieval.

## Indexable content

The index includes:

- the complete visible Markdown body;
- headings;
- relative path;
- the complete typed frontmatter tree;
- normalized and flattened frontmatter values in the search projection;
- OKF metadata when the document belongs to a bundle.

Normal full-text ranking excludes:

- YAML delimiters and raw metadata serialization;
- `construct-review:v1` payloads from normal body ranking;
- generated preview HTML;
- Mermaid render output;
- content reached through external URLs;
- unsaved buffers;
- files outside registered locations.

The typed frontmatter and complete body remain retrievable even when a value is
not included in the default weighted search projection. Exact source bytes are
not a recovery source and are never written back from the index.

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

Initial scans, full rebuilds, and incompatible migrations create a new complete
generation. Ordinary saved-file changes update the active generation in one
transaction.

During a rebuild:

1. the last healthy active generation continues serving reads;
2. the building generation is populated separately;
3. watcher changes observed during the scan are accumulated;
4. accumulated changes and a final reconciliation are applied;
5. active generation changes atomically;
6. obsolete generations are garbage-collected after activation.

Partial results are exposed only when no healthy generation exists yet. Every
query and status response declares its generation and completeness.

The public state vocabulary is:

- `notIndexed`;
- `indexing`;
- `ready`;
- `degraded`;
- `failed`.

A corrupt or incompatible index is quarantined or replaced after an integrity
probe. Failure falls back to current file navigation. SurrealKV's optional
temporal versioning is not a substitute for Construct generations; generation
semantics remain storage-engine independent.

## Retention and user controls

- An active Location retains its index indefinitely.
- A temporarily unavailable local or remote source retains its last healthy
  generation and is marked unavailable or stale.
- Removing a Location deletes its physical index by default.
- Interrupted and obsolete generations are cleaned after recovery.
- Semantic data, when introduced, can be deleted independently.
- Low disk space pauses indexing before the last healthy generation is harmed.
- No TTL or silent LRU eviction is used initially.

Per-Location controls expose index size, document count, generation, freshness,
`Rebuild index`, and `Delete index`. Global controls expose total storage,
`Rebuild all`, and `Delete all indexes`. Deleting or rebuilding derived data is
always described as a non-destructive operation.

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
- Two Locations never share a physical database or query scope implicitly.
- Full body and typed frontmatter remain retrievable from the active generation.
- Only `IndexService` opens embedded database directories.

## Accepted decisions

- SurrealDB embedded is the preferred engine; SQLite is no longer the planned
  production implementation.
- SurrealKV is the initial backend and RocksDB is the fallback.
- Each Location owns one physical index.
- Complete body and typed frontmatter are retained with a separate clean search
  projection.
- Canonical path identity and disposable continuity identity coexist.
- Rebuilds use generations; normal updates use transactions.
- Active and unavailable Locations retain indexes without TTL.
- `IndexService` is the exclusive storage owner and runs in Tauri initially.

## Implementation validation

Before declaring RFC 02 implemented, the SurrealDB integration must measure:

- release bundle-size and clean-build impact;
- initial ingestion for 1,000 and 10,000 documents;
- one-file create, change, rename, and delete;
- lexical top-20 search and backlink latency;
- physical storage size;
- restart, interrupted rebuild, and corrupt-store recovery;
- one isolated database per Location;
- packaging on supported desktop targets;
- the ownership boundary needed for the future RFC 05 sidecar.

### First implementation checkpoint

The initial native slice on 2026-07-26 established:

- successful macOS release packaging with SurrealDB 3.2.3 and SurrealKV;
- a clean release build time of approximately 2 minutes 36 seconds on the
  development machine;
- a 71 MB unsigned macOS `.app` bundle before release-profile size tuning;
- passing isolation tests for two physical Location stores;
- passing incremental one-document update and same-process close/reopen tests;
- passing exclusion of `construct-review:v1` payloads from normal full-text
  results;
- passing retrieval of the complete visible body and typed frontmatter.

These numbers are a baseline, not an optimization target or proof of the
10,000-document acceptance threshold. Capacity, interrupted rebuild, corruption,
rename/delete, and cross-platform packaging remain open validation work.

## Dependencies and handoff

The index provides data to the
[knowledge search experience](03-knowledge-search-experience.md),
[graph and context retrieval](04-graph-context-retrieval.md), and
[local agent access](05-local-agent-access.md).
