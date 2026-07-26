# RFC 01 — OKF compatibility

**Status:** Proposed

**Decision owner:** Product and native architecture

## Question

How should Construct consume OKF v0.1, v0.2, partially conforming bundles, and
future metadata without losing information or imposing a taxonomy?

## Context

Construct currently inspects OKF in TypeScript with a deliberately small
frontmatter parser. It represents values as strings or string arrays and
normalizes the fields needed by the current Explore experience.

That was appropriate for the first OKF cut, but it is not a durable
compatibility boundary:

- nested YAML objects cannot be represented faithfully;
- structured v0.2 fields such as `sources`, `generated`, and `verified` require
  mappings and sequences;
- a malformed unfamiliar value can prevent a concept from reaching the index;
- UI inspection, future indexing, CLI, and MCP would otherwise implement
  different interpretations.

## Proposed decision

Construct consumes OKF v0.1 and v0.2 tolerantly in a shared native parser.
Future versions are read on a best-effort basis and produce compatibility
findings rather than becoming inaccessible.

The parser has two representations:

1. an open-ended typed YAML tree that preserves every key and value;
2. a normalized convenience view for fields Construct understands.

The normalized view never replaces the original metadata.

## Compatibility rules

### Bundle detection

- A root `index.md` declaring `okf_version` is an automatic signal.
- A location with typed concept documents may be detected as OKF even when the
  optional version declaration is absent.
- Users can explicitly mark or unmark a location.
- A user decision takes precedence over automatic detection.
- Detection never rewrites `index.md` or concept metadata.

### Documents

Construct distinguishes technical roles from semantic types:

- bundle or directory `index.md`;
- directory `log.md`;
- concept document;
- other Markdown inside an explicitly marked bundle.

`type` remains a free-form semantic value. Construct does not maintain an
allowlist of valid types.

### Metadata

Construct should normalize these fields when present:

- `type`
- `title`
- `description`
- `resource`
- `tags`
- `timestamp`
- `okf_version`
- `sources`
- `generated`
- `verified`
- `status`
- `stale_after`

Typed metadata supports YAML nulls, booleans, numbers, strings, sequences, and
mappings. Unknown fields remain available to inspectors and future features.

For v0.2, `generated.at` is the preferred generation timestamp and legacy
`timestamp` remains a fallback. The original values are retained even when
Construct derives a preferred display value.

### Links

The parser records:

- inline and reference-style Markdown links;
- bundle-root-relative links;
- links relative to the current document;
- fragments;
- unresolved links;
- links in structured OKF fields only when that field defines a path contract.

Broken links are findings, not fatal errors. Wikilinks are outside the base
contract and require a separate compatibility decision.

### Findings

Findings use stable codes, an English message, severity, document identity, and
an optional source range.

- **Error:** Construct cannot safely interpret enough of the document for the
  requested operation.
- **Warning:** a version-specific or required field is missing or invalid.
- **Info:** a compatibility or hygiene observation.

Readable Markdown remains openable even when OKF inspection fails.

## Native boundary

Retrieval-critical parsing belongs in Rust because the desktop UI, CLI, and MCP
must share one interpretation. React receives typed inspection results and
renders them; it does not become the authoritative parser.

The current lossless editor boundary remains separate:

- source bytes and explicit saves stay authoritative;
- parsing never normalizes or rewrites YAML;
- rich editing continues to preserve frontmatter byte-for-byte;
- diagnostics must not modify malformed documents.

## Non-goals

- Repairing or upgrading bundles automatically.
- Enforcing a closed schema or vocabulary.
- Converting unknown YAML fields into strings.
- Rejecting broken links, missing indexes, or unknown types.
- Supporting every community link syntax in the initial parser.
- Replacing an independent conformance tool such as `okflint`.

## Experiments

1. Build synthetic fixtures for minimal and comprehensive v0.1/v0.2 bundles.
2. Include nested mappings, arbitrary keys, invalid YAML, unknown versions,
   broken links, cycles, reserved documents, and mixed line endings.
3. Compare current Construct results with the official specification and an
   independent validator.
4. Confirm parsing libraries preserve source ranges and enforce size/nesting
   limits.
5. Measure parse time and allocation on a synthetic 10,000-document corpus.

## Acceptance criteria

- Unknown keys and nested YAML values survive inspection without flattening.
- v0.1 and v0.2 fixtures produce the expected normalized view.
- Unknown future versions remain readable with a compatibility finding.
- Missing optional fields, unknown types, and broken links never hide a
  readable document.
- Root-relative links cannot escape the registered location.
- UI, index, CLI, and MCP consume the same parsed model.
- No parse or validation path writes to a source document.

## Open decisions

- Which Rust YAML library best combines typed values, limits, and source ranges?
- Which Markdown parser should own headings and link extraction?
- Should initial v0.2 lifecycle metadata be displayed or only indexed?
- Should wikilinks be an opt-in location compatibility setting?
- Which public fixtures can be included under a compatible license?

## Dependencies and handoff

This RFC is a prerequisite for OKF enrichment in the
[local Markdown index](02-local-markdown-index.md). Accepted decisions must
later update the current product specification and architecture before code is
implemented.
