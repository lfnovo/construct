# Research baseline — OKF retrieval ecosystem

**Status:** Supporting research

**Snapshot date:** 2026-07-26

## Purpose

This document preserves the ecosystem evidence behind the retrieval RFCs. It
is not a dependency selection or a permanent maturity assessment. Every tool
must be rechecked when an implementation phase begins.

## Current Construct baseline

Construct currently:

- discovers and watches Markdown in Rust;
- parses a small OKF metadata subset in TypeScript;
- builds an in-memory OKF collection by reading candidate files;
- derives links and backlinks from Markdown;
- provides List and Graph views with type and tag filters;
- searches filenames and paths through quick open;
- excludes persisted Review blocks from OKF link extraction.

Known pressure points:

- nested OKF v0.2 metadata cannot be represented faithfully by the current
  string-or-string-array parser;
- a relevant filesystem signature change rebuilds the in-memory collection;
- content search is not implemented;
- graph layout work occurs in the webview and is visually bounded;
- agents do not have a stable retrieval surface;
- there is no labeled retrieval benchmark.

## OKF-native projects

| Project | Useful capability | Fit for Construct |
| --- | --- | --- |
| GoogleCloudPlatform `knowledge-catalog` | Authoritative specification, examples, and reference graph work | Primary conformance source, not an embedded incremental retrieval engine |
| `okflint` | Validation, audit, profiles, index generation, and JSON output | Useful external oracle; avoid shipping a Python runtime for the app core |
| `okf-gem` | Local validation, search, graph, serving, and CLI interaction ideas | Strong UX reference; Ruby packaging and v0.1 focus limit direct reuse |
| `okf-ingest` / R `okf` | DuckDB ingest, lexical search, graph traversal, context assembly, and semantic experiments | Best comparison implementation; useful for pilots, too young for tight runtime coupling |
| `okf-mcp` | Read-only agent operations over `okf-ingest` | Useful tool-granularity reference and low-risk external pilot |
| W4G1 `okf` | Early pure-Rust parser and CLI shape | Language fit, but insufficient maturity and version coverage for dependency use |

## Adjacent tools and engines

| Approach | Strength | Current decision |
| --- | --- | --- |
| SQLite + FTS5 | Embedded, transactional, mature, easy distribution, sufficient lexical ranking | Recommended default for the first local index |
| Tantivy | High-performance Rust search and richer indexing primitives | Reconsider only if benchmarks show SQLite is insufficient |
| DuckDB FTS | Strong analytical environment and proven by `okf-ingest` | Use in external experiments; continuous desktop updates and extension packaging add complexity |
| `sqlite-vec` | Potential vectors beside lexical data | Experimental; maturity and extension packaging need validation |
| Qdrant, LanceDB, Meilisearch | Capable specialized retrieval services | Avoid as a default because they add a service lifecycle |
| Ollama embeddings | Local models without hosted content transfer | Optional experiment after lexical-plus-graph evaluation |
| `kcmd` / mdcode | CLI and MCP ideas in the official repository | Targets Google Cloud catalog workflows and includes mutable/cloud-coupled behavior; not the local bundle engine |

## Maturity interpretation

- Young OKF tools are valuable references and benchmark implementations.
- A shipping desktop dependency has a higher bar than a separately installed
  research tool.
- Bundling Python, R, Ruby, Docker, a cloud account, or a second service would
  add disproportionate support cost to Construct's default path.
- An application-owned SQLite index has moderate implementation cost but the
  lowest continuing deployment cost.
- The official OKF specification remains authoritative when implementations
  disagree.

## Architectural implications

The research supports these choices:

1. Consume OKF v0.1 and v0.2 tolerantly.
2. Preserve arbitrary typed YAML instead of maintaining a closed schema.
3. Index all Markdown and treat OKF as structured enrichment.
4. Use SQLite/FTS5 for the first persistent lexical index.
5. Keep graph expansion bounded and explainable.
6. Measure lexical plus graph before adding local embeddings.
7. Share one Rust retrieval core across UI, CLI, and MCP.
8. Keep agent tools read-only for source documents.
9. Use external OKF projects as oracles and pilots, not mandatory runtime
   dependencies.

## Recommended low-risk research

### Parser matrix

Run a synthetic v0.1/v0.2 corpus through current Construct, the selected native
libraries, `okflint`, and `okf-ingest`. Record metadata loss, findings, and link
behavior.

### External agent pilot

Use `okf-ingest` and `okf-mcp` separately against local representative bundles.
Compare filesystem traversal, lexical search, link following, context size, and
task correctness.

### Capacity baseline

Generate non-sensitive 1,000- and 10,000-document locations with controlled
metadata and link density. Measure current scan, memory, and graph behavior.

### SQLite spike

Validate Rust packaging, FTS5 availability, Unicode behavior, weighted fields,
snippets, one-file updates, WAL concurrency, rebuild time, and database size in
a disposable experiment.

## Primary references

- [Official Open Knowledge Format specification](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- [Official knowledge-catalog repository](https://github.com/GoogleCloudPlatform/knowledge-catalog)
- [Official Metadata-as-Code source](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/toolbox/mdcode)
- [`okflint`](https://github.com/mattdav/okflint)
- [`okf-gem`](https://github.com/serradura/okf-gem)
- [`okf-ingest`](https://github.com/travisjakel/okf-ingest)
- [`okf-mcp`](https://github.com/travisjakel/okf-mcp)
- [W4G1 `okf`](https://github.com/W4G1/okf)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)
- [SQLite WAL](https://www.sqlite.org/wal.html)
- [Tantivy](https://github.com/quickwit-oss/tantivy)
- [DuckDB full-text search](https://duckdb.org/docs/stable/core_extensions/full_text_search.html)
- [Ollama embeddings](https://docs.ollama.com/capabilities/embeddings)
- [Model Context Protocol specification](https://modelcontextprotocol.io/specification/)
