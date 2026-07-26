# RFC 04 — Graph and context retrieval

**Status:** Proposed

**Decision owner:** Retrieval architecture and product

## Question

How should Construct use explicit links and repository structure to find
supporting knowledge and assemble bounded context for people and agents?

## Product opportunity

Search finds likely entry points. Agents still lose time when they must open
those files, inspect links, find directory indexes, follow backlinks, and decide
how much context is enough.

Construct can make that traversal deterministic and visible. Its distinctive
output is not a generated answer; it is a context pack:

- a small ordered set of saved documents or sections;
- a reason for every inclusion;
- visible document boundaries and provenance;
- explicit limits and truncation.

## Graph model

Documents are nodes. Resolved internal Markdown links are directed edges.

The retrieval graph records:

- source and target relative identities;
- raw link target and fragment;
- resolved or unresolved state;
- link text when available;
- source range;
- syntactic origin such as Markdown body or a defined metadata field.

Unresolved links remain findings and graph records. They do not make a bundle
invalid.

Technical roles such as root index, directory index, log, concept, and ordinary
Markdown remain visible. A technical role is not a semantic relationship type.

## Retrieval graph versus visual graph

The indexed graph is complete for the saved corpus. The visual graph is always
a bounded projection.

Current or future rendering limits must never limit:

- backlinks;
- outgoing-link queries;
- related-document ranking;
- context assembly.

The UI requests a subgraph around a query or selected node. It reports visible
and omitted counts and provides a list alternative.

## Related-document retrieval

For a text query:

1. lexical search produces a bounded seed set;
2. direct outgoing links and backlinks are collected;
3. structurally related directory indexes are considered;
4. candidates are scored by hop, direction, supporting seeds, and explicit
   structure;
5. lexical and structural ranks are fused;
6. strict node, edge, depth, time, and result limits apply.

One hop is the interactive default. A second hop requires an explicit request
or remaining context budget. Cycles are always detected.

The first fusion strategy should be deterministic and explainable, such as
reciprocal rank fusion across:

- lexical rank;
- exact title/path rank;
- direct graph-neighbor rank;
- relevant ancestor or linked index rank.

Personalized PageRank and similar global algorithms remain experiments until
they beat bounded expansion in the relevance set.

## Explanations

Every related result identifies why it appeared:

- linked from a seed;
- links to a seed;
- referenced by a relevant directory index;
- shares an exact OKF tag;
- connected through one additional bounded hop.

The explanation names the source relative path and link text when safe and
available.

## Context pack input

A caller provides:

- query text, starting document, or both;
- location scope;
- optional metadata filters;
- maximum documents;
- maximum characters or approximate tokens;
- maximum graph depth within server limits;
- full-body or selected-section preference;
- whether findings and source metadata are included;
- whether open Review comments are included.

## Context selection

The default planner:

1. resolves scope and any starting document;
2. selects high-confidence lexical seeds;
3. identifies matched headings or sections;
4. includes relevant root or directory indexes only when structurally related;
5. adds strongly supported direct links and backlinks;
6. preserves explicit Review comments when requested;
7. orders documents deterministically;
8. stops before the content budget;
9. records candidates omitted by budget or limits.

An `index.md` does not receive automatic priority merely because of its
filename. It must be an ancestor, an explicit link, or otherwise connected to
the selected concepts.

## Context pack output

The output contains:

- original query and effective limits;
- index generation and completeness;
- ordered manifest;
- location and relative path for every item;
- title, technical role, selected headings, and inclusion reason;
- a body, excerpt, or section with visible document boundaries;
- relevant internal links;
- requested findings and lifecycle metadata;
- requested open Review comments as a separate field;
- approximate character or token count and estimator;
- truncation and omission summary.

Content from different files is never merged without visible provenance.
Ordering is stable for the same saved corpus, query, configuration, and indexer
version.

## Budgets

The core always supports character budgets. Token budgets are estimates unless
a named local tokenizer is configured. Responses state the estimator and keep
a safety margin.

Proposed server maxima:

- 50 search results;
- 200 graph nodes;
- 500 graph edges;
- two graph hops;
- 20 documents in a default context pack;
- a configurable maximum output size.

Defaults are benchmark inputs, not permanent product promises.

## Review behavior

Review payloads must not become ordinary content or graph edges:

- quotes and comments are excluded from lexical seed ranking;
- Markdown links inside comments are excluded from the knowledge graph;
- open reviews may be included explicitly in a context pack;
- their provenance identifies them as user feedback, not document content.

Detailed behavior belongs to
[RFC 06](06-review-integration.md).

## Non-goals

- Generating an answer with an LLM.
- Inferring a universal semantic relationship taxonomy.
- Fetching external links.
- Unbounded graph traversal.
- Treating every shared tag as a strong relationship.
- Replacing the user's source files with assembled context.
- Requiring embeddings.

## Evaluation

Use the same labeled question set as knowledge search and compare:

- filename/path lookup;
- lexical search;
- lexical plus direct outgoing links;
- lexical plus outgoing links and backlinks;
- index-aware context assembly;
- any later semantic experiment.

Measure relevant-document recall, files opened, tokens returned, time to useful
context, and answer sufficiency in agent trials.

## Acceptance criteria

- Backlinks and outgoing links remain available independently of visual graph
  limits.
- Expansion is cycle-safe and bounded.
- Every related result has a human-readable structural explanation.
- Context packs preserve document boundaries and relative provenance.
- Context assembly respects the configured budget within its documented margin.
- Truncation and omissions are explicit.
- The same saved corpus and inputs produce stable ordering.
- Review comments are included only when explicitly requested.

## Open decisions

- Initial fusion algorithm and weights.
- Section extraction rules and anchor stability.
- Character-only budget versus bundled local tokenizer.
- Whether people can manually pin documents into a context pack.
- How graph projections are laid out without blocking the UI thread.
- Whether cross-location links are ever resolved.

## Dependencies and handoff

This RFC depends on the [local index](02-local-markdown-index.md) and
[knowledge search](03-knowledge-search-experience.md). Its operations are
exposed to agents through [RFC 05](05-local-agent-access.md).
