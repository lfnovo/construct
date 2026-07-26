# RFC 07 — Review integration

**Status:** Proposed

**Decision owner:** Review workflow and retrieval architecture

## Question

How should `construct-review:v1` comments participate in indexing, search,
context assembly, and agent access without becoming false document knowledge?

## Current behavior

Review comments are stored in a versioned HTML comment immediately after YAML
frontmatter or at the start of a document without frontmatter.

Each open comment contains:

- ID;
- selected quote snapshot;
- feedback;
- creation time.

The block:

- remains invisible in Preview and rich Edit;
- is visible in Source and readable by agents;
- shares the normal explicit-save buffer;
- can be removed exactly;
- is already excluded from OKF link extraction.

This RFC does not change that source format.

## Problem

Review text is intentionally about the document, but it is not the document's
knowledge.

If indexed as ordinary body text:

- quoted content may be counted twice;
- feedback can rank a document for words it does not actually assert;
- Markdown links in feedback can create false graph edges;
- an unresolved instruction may be presented as factual context.

If stripped completely:

- agents cannot discover that a document has feedback to address;
- context assembly may omit the most relevant human instruction;
- the product loses a natural review queue.

## Proposed decision

Parse Review as a separate typed channel.

### Normal document retrieval

- Review blocks are excluded from FTS body, snippets, headings, and metadata
  boosts.
- Links inside Review are excluded from the knowledge graph.
- Review quotes are not used as duplicate source content.
- A document may expose a neutral `open_review_count` filter or badge.

### Review-aware retrieval

- `get` can include open reviews in a separate `reviews` field.
- `build_context` includes reviews only when explicitly requested.
- Review output labels quotes and feedback as user annotations.
- A future Review queue may search feedback through a dedicated field that does
  not affect knowledge rank.

## Indexed review record

The derived index may store:

- document identity;
- review ID;
- quote snapshot;
- feedback;
- created time;
- parser version.

This record is disposable and derives from the saved Markdown block. Removing
the block removes derived review records.

The index does not invent resolved status. In v1, presence means open and
absence means removed or addressed.

## Search experience

The first retrieval UI may show:

- an “Open reviews” badge on results;
- a `Has open reviews` filter;
- open-review count in document details.

Ordinary query ranking remains based on actual document content and metadata.
A dedicated feedback search is deferred until a real workflow requires it.

## Context behavior

When `include_reviews` is false or omitted:

- the context pack contains document content only;
- it may report an open-review count without including feedback.

When `include_reviews` is true:

- reviews appear after the relevant document item in a separate annotation
  structure;
- quote and feedback retain their IDs;
- the prompt or transport instructs the caller not to treat feedback as source
  claims;
- review characters count against the output budget;
- truncation reports omitted reviews explicitly.

## Agent workflow

A read-only Construct MCP does not resolve a review itself.

The expected loop is:

1. an agent retrieves a document and its open reviews;
2. it validates each request against the document and project context;
3. it edits the Markdown using its normal workspace tools;
4. it removes only comments it addressed;
5. it preserves unresolved comments;
6. the saved file triggers Construct's normal watcher and reindex flow.

This matches the copyable Review handoff already produced by the desktop app.

## Malformed blocks

- A malformed review block is never rewritten by indexing.
- Document content remains available when it can be separated safely.
- Review-aware operations return a finding and no parsed review list.
- Source remains the recovery path.
- Ordinary body search must not silently index malformed review payload text as
  knowledge.

The exact fallback depends on whether the parser can identify the block
boundary safely.

## Privacy

Review feedback can be more sensitive than the source document because it may
contain human instructions or critique.

- It remains local and receives the same cache protections as document bodies.
- It is returned only to callers explicitly requesting reviews.
- It is omitted from logs and normal search snippets.
- Cache deletion removes derived review records.

## Non-goals

- Collaborative identities, replies, mentions, or remote sync.
- Review history after comments are removed.
- Resolving comments through MCP.
- Automatically editing quoted text when a review is created.
- Treating quote snapshots as stable source anchors after arbitrary edits.
- Ranking ordinary knowledge by review feedback.

## Acceptance criteria

- Review text cannot affect normal document FTS rank.
- Review links cannot create knowledge-graph edges.
- Open-review count can be derived without modifying the source file.
- Review-aware `get` and context output clearly separate annotations from
  document content.
- Removing the last review removes its derived records after indexing.
- Malformed review data is never automatically rewritten.
- Review output obeys normal location scope, privacy, and size limits.

## Open decisions

- Whether Phase 1 exposes only counts or full derived review records.
- Whether Search includes the `Has open reviews` filter immediately.
- How quote snapshots are shown when the quoted source changes.
- Whether a future format adds explicit resolved state instead of removal.
- Whether feedback-specific search is useful enough to justify a separate FTS
  channel.

## Dependencies and handoff

Review parsing exists today in `src/review.ts`. Any native retrieval parser must
match its lossless boundary and serialization rules. This RFC affects the
[local index](02-local-markdown-index.md),
[graph/context retrieval](04-graph-context-retrieval.md), and
[local agent access](05-local-agent-access.md).
