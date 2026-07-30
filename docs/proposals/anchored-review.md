# Anchored review experience

**Status:** Accepted

**Date:** 2026-07-30

## Summary

Construct should preserve the reader's place when changing document modes and
make Review comments visibly and bidirectionally connected to their passages.

The existing `construct-review:v1` block remains the source of truth. New
comments add an optional, backward-compatible locator inside that block.
Existing quote-only comments remain readable and are located on a best-effort
basis without rewriting the document.

## Problem

Changing between Preview, Edit, Review, and Source currently unmounts one
surface and mounts another at the top. The document buffer is shared, but its
reading position is not.

Review comments currently persist:

- an ID;
- a normalized quote;
- the feedback;
- the creation time.

That is enough for an agent to understand the feedback, but not enough for the
interface to distinguish repeated passages or recover confidently after edits.
The rendered document therefore has no visible review annotation, and the
comment panel has no navigation target.

## Decisions

### Position continuity

- Construct keeps the latest scroll state for every open tab and mode.
- An explicit mode change transfers a semantic text anchor from the old surface
  to the new one.
- Returning to an already mounted or revisited mode restores its own position
  when no mode-transfer anchor is pending.
- A bounded proportional position is the fallback when the semantic anchor
  cannot be found.
- These positions are runtime UI state and are not persisted to Markdown.

### Review locators

New comments may include:

```json
{
  "anchor": {
    "start": 128,
    "end": 162,
    "prefix": "text immediately before",
    "suffix": "text immediately after"
  }
}
```

Offsets refer to the normalized rendered text at creation time. The quote
remains the human- and agent-readable snapshot. Resolution proceeds in order:

1. the original range when it still contains the exact quote;
2. an exact quote whose surrounding context matches;
3. a unique exact quote;
4. otherwise the comment is detached.

The interface must never silently attach an ambiguous comment to an arbitrary
occurrence.

### Presentation and navigation

- Resolved passages are highlighted at render time; source Markdown is not
  decorated with visible inline markup.
- Clicking a highlight selects and reveals its comment.
- Clicking a comment scrolls to and focuses its highlight.
- The active pair has a stronger visual treatment.
- Detached comments remain visible with an explicit “Passage changed” state.
- Highlights are keyboard focusable and do not rely on color alone.

### Compatibility

- `construct-review:v1` continues to accept the original four required fields.
- `anchor` is optional and additive.
- Parsing and serializing an old comment does not invent an anchor.
- A malformed review block is never rewritten automatically.
- Adding, removing, or navigating reviews still respects explicit save.

## Delivery

### Change 1 — document position continuity

- capture and restore per-mode scroll state;
- transfer a semantic block anchor on explicit mode changes;
- cover anchor matching and fallback behavior with pure tests.

### Change 2 — anchored reviews

- capture locators for new selections;
- resolve old and new comments;
- render highlights and detached states;
- add bidirectional mouse and keyboard navigation;
- update product, architecture, and user documentation.

## Acceptance criteria

- Switching Edit → Review keeps the same passage in view when it still exists.
- Switching back does not reset the document to the top.
- A newly created comment immediately highlights its selected passage.
- Highlight → comment and comment → highlight navigation both work.
- Repeated quotes are resolved only with sufficient context.
- Changed or removed passages produce a detached state.
- Existing quote-only comments continue to parse and can be highlighted when
  their quote is unique.
- No navigation or highlighting action changes the Markdown buffer.
