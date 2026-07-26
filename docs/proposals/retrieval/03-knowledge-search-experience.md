# RFC 03 — Knowledge search experience

**Status:** Proposed

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

## Proposed first experience

Add a dedicated Search workspace available from the sidebar and a keyboard
shortcut. It searches every indexed registered location and can be scoped to
the active location.

The surface contains:

- a query input with immediate keyboard focus;
- location, path, type, tag, role, status, and finding filters;
- ranked results with title, relative path, snippet, and match explanation;
- an index-state indicator;
- keyboard navigation and open-in-pane actions;
- pivots from a result to related documents or a graph subset.

Empty search should present useful navigation rather than every document body:

- recent queries stored locally;
- active location filters;
- common OKF types and tags;
- recently changed indexed documents;
- an explanation when indexing is incomplete.

## Search scope

The default scope is a product decision:

- Search opened from a location defaults to that location.
- Search opened globally defaults to all registered locations.
- Scope is always visible and can be changed without rewriting the query.
- Results never silently mix identities from locations with the same name.

The app should remember the last scope for the current search session but avoid
creating persistent hidden filters that surprise users later.

## Query behavior

The first query language supports:

- ordinary free text;
- quoted exact phrases;
- visible filter controls;
- an optional path subtree;
- arbitrary OKF type and tag values;
- inclusion or exclusion of documents with parse findings.

Advanced operators are deferred until normal queries are measured. User input
is translated into safe parameterized FTS expressions; raw SQLite syntax and
errors are never exposed.

Portuguese and English queries, diacritics, Unicode filenames, and case
behavior require explicit tests.

## Ranking

The initial candidate rank is FTS5 BM25 with field weights that are benchmark
defaults rather than semantic truth:

| Field | Starting relative weight |
| --- | ---: |
| Title | 8 |
| Description | 5 |
| Tags | 4 |
| Headings | 3 |
| Relative path | 2 |
| Body | 1 |

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
- copy its relative identity;
- inspect related documents;
- open a result-centered graph view;
- add it to a future context selection.

The first delivery needs only open-in-pane and related-document navigation.
Manual context selection is useful but can follow the context RFC.

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
- Query history, if retained, is local and user-clearable.
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
- Every result includes provenance, relative identity, snippet, and match
  explanation.
- Incomplete indexing is visible without making available results unusable.
- Query text and snippets never leave the device.
- Opening a result uses the existing pane and explicit-save model.

## Open decisions

- Separate Search workspace versus a mode inside the command palette.
- Global or active-location default scope.
- Dedicated keyboard shortcut.
- Whether recent queries are persisted.
- Which filters belong in the first visible UI.
- Whether the first release includes manual context selection.

## Dependencies and handoff

This RFC depends on the
[local Markdown index](02-local-markdown-index.md). Its ranked seeds become
inputs to [graph and context retrieval](04-graph-context-retrieval.md).
