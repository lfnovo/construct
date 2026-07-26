# RFC 03 — Knowledge search experience

**Status:** Accepted — implementation in progress

**Decision owner:** Product and desktop UX

## Question

How should people search document content and metadata without weakening the
fast file-navigation workflow Construct already provides?

## Product distinction

Construct has two related but different jobs:

- **Open a known file:** the user remembers enough of its name or path.
- **Find relevant knowledge:** the user remembers a topic, claim, type, tag, or
  question but not the document.

`⌘P` should remain optimized for the first job. Knowledge search should be
optimized for the second. They may share a command surface later, but their
result semantics and keyboard behavior must remain clear.

## Accepted first experience

Construct adds a dedicated Search workspace available from the sidebar and
`⌘⇧F`. `⌘P` remains the fast filename/path opener, while Search answers topic,
claim, metadata, and question-shaped discovery needs. `⌘F` remains reserved for
finding text inside the current document.

The surface contains:

- a query input with immediate keyboard focus;
- visible Location, type, and tag filters;
- path, role, finding, lifecycle, trust, and freshness filters under
  `More filters`;
- ranked results with title, relative path, snippet, and match explanation;
- an index-state indicator;
- keyboard navigation and open-in-pane actions;
- an ephemeral manual selection with `Copy references`;
- future pivots from a result to related documents or a graph subset.

Empty search should present useful navigation rather than every document body:

- up to 20 recent queries stored locally when the user enables retention;
- active location filters;
- common OKF types and tags;
- recently changed indexed documents;
- an explanation when indexing is incomplete.

## Search scope and federation

- Search opened from a Location defaults to that Location.
- If no Location is active, Search defaults to all available Locations.
- The scope is a visible multiselect, so users can search one, several, or all
  Locations.
- Scope is always visible and can be changed without rewriting the query.
- Results never silently mix identities from locations with the same name.
- Cross-Location search fans out to physically isolated per-Location indexes.
  It does not create a global database.
- Raw full-text scores from separate corpora are not compared directly.
  Construct converts each local result list into a rank and applies reciprocal
  rank fusion before presenting one list.

The app remembers scope inside the current Search session. A recent query
restores its explicit scope and filters only when the user selects it.

## Query behavior

The first query language supports:

- ordinary free text;
- quoted exact phrases;
- visible multiselect filter controls;
- an optional path subtree;
- arbitrary OKF type and tag values;
- inclusion or exclusion of documents with parse findings;
- official OKF v0.2 lifecycle, trust, and freshness interpretation.

Advanced operators are deferred until normal queries are measured. User input
is translated into safe parameterized SurrealQL full-text expressions; raw
database syntax and errors are never exposed.

Filter values use OR inside one category and AND between categories. For
example, `Person OR Project` combined with `content OR strategy` means:

```text
(Person OR Project) AND (content OR strategy)
```

Documents without a type remain searchable until the user explicitly applies a
type filter.

The first visible filters are:

- Locations;
- types;
- tags.

`More filters` contains:

- path prefix;
- document role: concept, index, or log;
- with or without findings;
- OKF `status`: draft, stable, or deprecated;
- trust tier derived from `verified`: unverified, machine-confirmed, or
  human-reviewed;
- freshness derived from `stale_after`: current, stale, or unspecified.

Absent OKF `status` means stable. Absent `stale_after` means unspecified, not
proven current. `generated`, `sources`, and the complete open-ended
frontmatter remain indexed without becoming first-release visible filters.

Portuguese and English queries, diacritics, Unicode filenames, and case
behavior require explicit tests.

## Ranking

The initial candidate rank uses SurrealDB full-text indexes with BM25 signals
and deterministic field weights. The weights are benchmark defaults rather
than semantic truth:

| Field | Starting relative weight |
| --- | ---: |
| Title | 8 |
| Description | 5 |
| Type | 4 |
| Tags | 4 |
| Headings | 3 |
| Relative path | 2 |
| Body | 1 |
| Other frontmatter | 1 |

Exact title, exact path, and exact tag matches may receive deterministic
documented boosts.

Freshness, verification, status, or popularity must not silently override
textual relevance. If used later, each signal is visible in the explanation and
can be evaluated independently.

## Result contract

Each result displays:

- title or filename fallback;
- location name and relative path;
- a bounded highlighted snippet;
- matched fields;
- relevant type, tags, and technical role;
- warnings for stale, incomplete, or malformed data when applicable;
- a short explanation such as “title match”, “matched heading”, or “related to
  two top results”.

Result identity is location ID plus relative path. The React UI can ask native
code to open it; normal external result payloads do not expose absolute paths.

## Navigation

From a result, the user can:

- open in the active pane;
- open to the right;
- reveal its containing location in the file tree;
- add or remove it from the current Search selection;
- copy selected relative identities;
- inspect related documents;
- open a result-centered graph view;
- add it to a future context assembly.

The first delivery includes open in the active pane, open to the right,
ephemeral multiselection, and `Copy references`. The copied representation
contains Location name or ID, relative path, title, and match reason. It does
not expose an absolute path or copy document contents.

Selection belongs to the current Search session and is never populated
automatically from rank. Persistent collections and `Build context` belong to
[RFC 04](04-graph-context-retrieval.md).

## Recent searches

Construct can retain the latest 20 submitted searches locally. A recent entry
contains query, explicit Location scope, filters, and time. Recent searches:

- appear only in the empty Search state;
- can be cleared together;
- can be disabled through `Remember recent searches`;
- are deleted when retention is disabled;
- never contain result snippets or document contents;
- never leave the device.

## Index states

Search remains usable while indexing and communicates:

- ready;
- indexing with progress;
- incomplete;
- stale;
- failed with a recovery action.

Results include the active index generation. A rebuild action states clearly
that it removes only derived local data.

## Accessibility

- Every result and filter is keyboard reachable.
- Match reasons are text, not color alone.
- Screen readers receive result count, title, path, and matched fields.
- Focus returns predictably after opening and closing a result.
- Index progress uses text and determinate counts when available.
- Graph results always have a list alternative.

## Privacy

- Queries and result interactions remain local.
- Query history is limited, local, optional, and user-clearable.
- No remote autocomplete or analytics receive query text.
- Snippets are created from the local saved index.

## Non-goals

- Replacing `⌘P`.
- Natural-language answer generation inside Search.
- Hosted search.
- Semantic search in the first delivery.
- A complex public query language.
- Searching unsaved buffers through the shared index.
- Presenting every graph relationship in the result list.

## Validation

Build a local relevance set with at least 20 realistic questions from two or
three locations. For each query, label useful entry documents and supporting
documents.

Compare:

- filename/path navigation;
- lexical content search;
- lexical search with exact metadata boosts;
- later lexical-plus-graph results.

Record top-1, top-3, and top-10 recall, reciprocal rank, time to first useful
result, and files opened.

## Acceptance criteria

- A user can distinguish file navigation from knowledge search.
- Search covers ordinary Markdown and OKF documents.
- Scope and filters are visible and keyboard operable.
- Every result includes provenance, relative identity, highlighted snippet, and match
  explanation.
- Incomplete indexing is visible without making available results unusable.
- Query text and snippets never leave the device.
- Opening a result uses the existing pane and explicit-save model.
- Search opened from a Location defaults to that Location.
- Multiple Location indexes are combined without creating shared storage or
  comparing raw scores directly.
- Filters implement OR inside a category and AND between categories.
- Official OKF v0.2 status, trust, and freshness semantics are available.
- Manual selection is ephemeral and copied references contain no absolute path.
- Recent queries are bounded, local, optional, and clearable.

## Accepted decisions

- Search is a dedicated workspace, not a command-palette mode.
- `⌘⇧F` opens or focuses Search.
- The active Location is the default scope and the visible scope control is a
  multiselect.
- Cross-Location results use rank fusion over isolated indexes.
- Locations, types, and tags are first-level filters; advanced filters are
  grouped under `More filters`.
- The latest 20 searches may be retained locally and cleared or disabled.
- The first release includes ephemeral manual selection and `Copy references`.
- Context assembly remains in RFC 04.

## Dependencies and handoff

This RFC depends on the
[local Markdown index](02-local-markdown-index.md). Its ranked seeds become
inputs to [graph and context retrieval](04-graph-context-retrieval.md).
