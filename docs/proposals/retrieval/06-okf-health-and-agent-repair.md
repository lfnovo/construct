# RFC 06 — OKF health and agent repair

**Status:** Proposed

**Decision owner:** Product, OKF compatibility, retrieval, and agent access

## Question

How should Construct turn its existing OKF findings into an understandable
Location health report and a safe, bounded repair workflow for coding agents?

## Product goal

A user should be able to select an OKF Location and answer:

- Is this bundle conformant enough to be consumed safely?
- Which documents need attention?
- Which problems are specification failures, compatibility observations, or
  optional quality improvements?
- What should a coding agent fix next?
- Did the repair improve the bundle without erasing its custom metadata or
  taxonomy?

The workflow should make older bundles progressively easier for agents to
navigate without requiring Construct to rewrite them.

## Current foundation

RFC 01 already provides a shared native OKF parser with:

- tolerant v0.1 and v0.2 consumption;
- open-ended typed YAML metadata;
- stable finding codes;
- error, warning, and info severities;
- relative document identity;
- optional source ranges;
- bundle-level link resolution and broken-link findings.

The per-Location index already stores a finding count and the native OKF
inspection result used to derive it. Search can include or exclude documents
with findings, Explore shows an aggregate count, and the document inspector
shows findings for an open file.

The missing product layer is aggregation. Construct can currently say that a
Location has findings, but it does not provide a queryable repair queue grouped
by rule, severity, or path. MCP exposes the aggregate count through the Location
overview but not the detailed diagnostics.

## Inspiration and boundary

The Obsidian community plugin
[OKF Enforcer](https://community.obsidian.md/plugins/okf-enforcer)
demonstrates useful interaction patterns:

- a vault-wide report hidden until requested;
- errors before warnings;
- a compact conformant/error/warning summary;
- navigation from a finding to the affected note;
- separate commands for validation and repair;
- non-blocking scans for large vaults.

Construct may adopt those interaction patterns without depending on the plugin
or reproducing its mutation model. The plugin targets OKF v0.1 and offers
on-save hooks, default types, index generation, log updates, and bulk fixes.
Construct consumes v0.1 and v0.2 and preserves explicit saves, open taxonomies,
and agent-owned source changes.

`okflint` remains a useful independent conformance oracle and CI tool. It is not
a required packaged runtime for the first delivery.

## Proposed decision

Add a read-only **Health** surface for each OKF Location and expose the same
findings through the local MCP.

Health is derived from saved files in the active index generation. It never
edits a document, inserts frontmatter, selects a default `type`, regenerates an
index, or appends to a log.

The first repair path is agent-assisted:

1. inspect the Location;
2. choose a bounded batch of findings;
3. copy a structured repair request or retrieve it through MCP;
4. let the coding agent inspect project context and edit source files normally;
5. let the watcher reconcile saved changes;
6. inspect the new active generation and confirm what improved or remains.

This keeps Construct authoritative for inspection and the repository
authoritative for changes.

## Rule model

Every diagnostic belongs to one visible tier.

### Conformance

Rules required to consume a document as OKF, including:

- parseable YAML frontmatter on concept documents;
- a non-empty, scalar `type`;
- valid reserved-document boundaries;
- required reserved-document structure;
- paths that remain inside the registered Location.

Conformance failures are errors when Construct cannot safely interpret the
required OKF structure. Readable Markdown remains openable.

### Compatibility

Rules that describe version behavior without rejecting the document, including:

- unsupported future `okf_version`;
- invalid shapes for normalized v0.1 or v0.2 fields;
- unknown lifecycle values when the declared version defines a vocabulary;
- malformed structured provenance, generation, verification, status, or
  freshness metadata.

Unknown metadata keys and free-form `type` values are never compatibility
failures.

### Hygiene

Non-blocking observations that can improve progressive disclosure or agent
navigation, including:

- missing recommended title or description;
- a missing or weak directory index;
- unconventional log date headings;
- unresolved internal links;
- stale metadata when the bundle explicitly declares lifecycle information.

Broken links are permitted by OKF and therefore remain warnings or information,
not conformance failures.

### Optional profile

A profile may describe conventions chosen by a team or bundle family:

- recommended fields;
- preferred metadata shapes;
- allowed lifecycle values;
- directory expectations;
- relationship fields;
- naming conventions.

Profile findings are never presented as base OKF failures. A profile is
explicitly selected, versioned, and identified in every result.

The `knowledge` Location may be used during evaluation as a high-quality
reference bundle. It must not silently become Construct's universal schema.
Comparing another Location with `knowledge` is descriptive until the user
defines or selects a profile.

## Finding contract

The existing stable finding model should grow into a queryable record with:

- Location ID;
- active index generation;
- relative document path;
- stable finding code;
- rule tier;
- severity;
- English title and message;
- optional source range;
- optional specification or profile reference;
- repair classification;
- parser or profile version.

Repair classification is one of:

- `manual`: requires domain or structural judgment;
- `agent`: suitable for an agent after reading repository context;
- `safe_candidate`: mechanically obvious but still not applied by the first
  delivery.

The classification describes likely handling. It does not authorize mutation.

Finding identity must be deterministic enough to compare two completed
generations. A stable identity may combine Location, relative path, code, source
range, and a normalized discriminator. Messages are not identity because copy
can improve without creating a new logical issue.

## Derived persistence

Detailed findings should become first-class derived records in each Location's
existing SurrealDB index rather than remaining only an aggregate count.

They follow normal generation semantics:

- a reader sees findings from one complete active generation;
- a partial rebuild never replaces the last healthy report;
- fixing or removing a source issue removes its derived finding in the next
  active generation;
- deleting the derived index deletes all health data without touching files;
- profile findings record the profile and version that produced them.

The index may retain a normalized document inspection snapshot, but Health and
MCP should not need to parse opaque JSON client-side to filter findings.

No document body, absolute path, or human feedback is copied into the finding
record.

## Health experience

An OKF Location adds a **Health** action beside Search and Explore.

### Summary

The header shows:

- documents inspected;
- documents with errors;
- documents with warnings only;
- conformant documents;
- total findings by severity and tier;
- active generation and last reconciliation time.

The summary uses labels and icons in addition to color. A single opaque
“quality score” is deferred because arbitrary weights could suggest more
precision than the rules provide.

An overall state may use deterministic language:

- **Conformant:** no conformance errors;
- **Needs attention:** at least one conformance error;
- **Inspection incomplete:** active data is unavailable or still building;
- **Unavailable:** the last healthy index cannot be read.

Warnings do not make an otherwise conformant bundle fail.

### Report

The report supports:

- grouping by rule or document;
- errors before warnings before information;
- filters for tier, severity, code, path prefix, and profile;
- text search over code, title, message, and relative path;
- collapse and expand by group;
- keyboard navigation;
- opening the affected document in the current pane;
- positioning near the source range when it is available;
- copying one finding, one group, a selected set, or a bounded repair batch.

The initial report is on demand and does not block workspace restoration.
Indexing continues to use the native background path.

### Document state

An open OKF document may show a compact status:

- conformant;
- warning count;
- error count.

Its existing inspector remains the detailed single-document view. Health is the
Location-wide view and must not duplicate a second parser in React.

## Agent repair handoff

**Copy for agent** emits a deterministic, bounded Markdown payload:

```text
Repair the selected OKF findings in Location "example".

Constraints:
- Preserve unknown frontmatter fields and valid source content.
- Do not impose a closed type taxonomy.
- Do not invent missing domain values.
- Treat broken links as findings to inspect, not automatic deletions.
- Keep reserved index and log documents compatible with the declared OKF version.
- Work only on the listed relative paths.
- Run the available validation after editing.

Active generation: 12
Selected findings: 8
Omitted findings: 31

1. OKF_TYPE_REQUIRED [error, conformance]
   path: projects/example.md
   range: 1:1
   message: Concept documents must have a non-empty type.
```

The payload includes:

- Location name and ID;
- active generation;
- selected filters;
- finding code, tier, severity, path, range, and message;
- profile identity when applicable;
- count and reason for omitted findings;
- repair constraints;
- a request to report changed files and remaining uncertainty.

It does not include absolute paths or unrelated document bodies. The agent
already working in that repository resolves relative paths through its
workspace.

Batch ordering is stable:

1. conformance errors;
2. compatibility errors or warnings;
3. hygiene warnings;
4. information;
5. relative path and source position.

The default batch is intentionally small enough for an agent to inspect each
change rather than apply a blind repository-wide rewrite.

## MCP contract

Add a read-only operation:

| Operation | Purpose |
| --- | --- |
| `construct_get_okf_findings` | Return a bounded, filterable Location repair queue |

Proposed input:

```text
locationId
severities?
tiers?
codes?
pathPrefix?
profile?
limit?
cursor?
```

The response contains:

- Location identity;
- active generation and completeness;
- aggregate counts for the effective filter;
- ordered finding records;
- next cursor;
- truncation state;
- applied profile identity.

The operation follows the current MCP allowlist, relative-path, output-size,
timeout, and no-network rules. It does not read arbitrary source paths and does
not expose a fix, write, save, shell, Git, or index-rebuild operation.

`construct_get_location_overview` may add a compact health summary, but detailed
findings belong to the dedicated operation so hot-memory calls remain bounded.

## CLI and export

A future CLI adapter may expose:

```text
construct okf health --location <id>
construct okf lint --location <id> --format json
```

JSON is the preferred first machine-readable export because it reuses the typed
MCP contract. SARIF and direct CI integration remain follow-up decisions.

The desktop and MCP slice does not require a public CLI.

## Repair policy

### First delivery

All rules are read-only. Construct can:

- inspect;
- filter;
- navigate;
- copy;
- serve findings to an allowed agent;
- verify the next saved generation.

### Deferred safe fixes

A later decision may allow narrowly mechanical, explicit fixes such as:

- inserting a closing delimiter only when the intended boundary is
  unambiguous;
- converting an exact malformed reserved heading;
- adding a user-provided field value;
- regenerating a selected directory index after previewing the diff.

Any future fix must:

- preview the exact patch;
- preserve explicit save;
- preserve unknown metadata and formatting where possible;
- never infer a domain `type` from a filename without confirmation;
- be individually undoable;
- keep Git integration read-only;
- have fixture-backed losslessness tests.

On-save enforcement, a default `type`, silent bulk repair, automatic log
entries, and automatic bundle normalization are outside this RFC.

## External validation

Construct's native inspection is the product contract used by UI, index, CLI,
and MCP. Independent tools remain valuable:

- `okflint` can act as a CI or research oracle;
- OKF Enforcer can inform Obsidian workflows;
- the official OKF specification remains authoritative when tools disagree.

The conformance corpus should record intentional differences between Construct
and an external validator. Construct must not execute an external tool against
user repositories automatically or require Python, Obsidian, or another runtime
to show Health.

## Privacy and security

- Health is local derived data.
- Findings use relative paths in normal UI exports and MCP.
- Messages never include document content beyond a minimal invalid value when
  needed to explain the problem.
- Copy for agent is an explicit user action.
- An MCP client may send returned findings to its configured model; the existing
  MCP trust disclosure applies.
- Health never follows external links or executes Markdown, YAML, Mermaid, code,
  or profile content.
- Profile files are data, not executable validation code.

## Reliability and scale

- Health reads one complete active generation.
- A report remains usable while a newer generation builds.
- Large reports are grouped, paginated, and virtualized when necessary.
- Filtering occurs in the native index rather than transferring every finding
  to React.
- A 10,000-document corpus must not block the UI thread.
- Report state distinguishes zero findings from unavailable findings.
- Unknown finding codes remain displayable after an app downgrade.

## Non-goals

- Replacing the tolerant OKF parser from RFC 01.
- Making every warning a conformance error.
- Defining a universal `type`, tag, status, or relationship vocabulary.
- Treating `knowledge` or any other Location as an implicit schema.
- Automatically repairing user files.
- Adding on-save enforcement or autosave.
- Generating or rewriting every `index.md` or `log.md`.
- Following broken links to fabricate missing concepts.
- Sending repositories to a hosted validation service.
- Making Obsidian or `okflint` a runtime dependency.
- Searching document knowledge through finding messages.

## Acceptance criteria

- A user can open Health for an OKF Location and distinguish conformance,
  compatibility, hygiene, and profile findings.
- Counts, report rows, document status, and MCP use the same active generation
  and native finding records.
- A conformance warning never becomes an error merely because a profile prefers
  a different taxonomy.
- Unknown frontmatter fields and free-form `type` values do not fail base OKF
  validation.
- The report can filter, group, keyboard-navigate, and open the affected
  relative document.
- Copy for agent is stable, bounded, explicit about omissions, and contains no
  absolute paths.
- MCP can retrieve a bounded repair queue without gaining source mutation.
- After an external repair and saved reconciliation, addressed findings
  disappear and unresolved findings remain.
- Index rebuilds do not fabricate source changes or findings.
- Health failure does not prevent reading, editing, or explicitly saving a
  document.
- The first delivery cannot modify source files.

## Evaluation

Evaluate against:

- the synthetic v0.1/v0.2 compatibility corpus;
- `knowledge` as a high-quality real bundle;
- at least two older partially conforming bundles;
- a synthetic 10,000-document bundle;
- independent `okflint` output where compatible.

Measure:

- correct error/warning classification;
- false positives and false negatives;
- time to identify the first actionable problem;
- report latency and memory;
- findings repaired per agent batch;
- accidental or unnecessary file changes;
- whether repeated inspection converges cleanly;
- differences between base conformance and optional profiles.

## Open decisions

- The minimum set of hygiene rules in the first release.
- Whether the first profile format is a small Construct manifest or compatible
  with an existing `okflint` manifest.
- Whether `knowledge` comparison remains a research-only benchmark or becomes
  an explicit user-created profile.
- The exact cursor and finding identity contract for MCP.
- Whether agent handoff supports manual multi-selection in the first slice.
- Whether a future health history should show generation-to-generation trends
  without retaining source content.
- Which safe fix, if any, is valuable enough to justify a separate mutation
  RFC.

## Dependencies and handoff

This RFC depends on:

- [RFC 01](01-okf-compatibility.md) for tolerant parsing and stable findings;
- [RFC 02](02-local-markdown-index.md) for per-Location generations and derived
  persistence;
- [RFC 03](03-knowledge-search-experience.md) for shared filtering and
  navigation patterns;
- [RFC 05](05-local-agent-access.md) for the MCP allowlist and read-only trust
  boundary.

Its implementation should precede
[RFC 07](07-review-integration.md). Health diagnostics and Review comments are
both structured human/agent work queues, but findings remain derived parser
output while Review remains user-authored source annotation.
