import { useEffect, useMemo, useRef, useState } from "react";
import { MarkdownPreview } from "./MarkdownPreview";
import {
  buildReviewPrompt,
  setReviewComments,
  splitReviewDocument,
  type ReviewComment,
} from "./review";
import {
  buildRenderedTextIndex,
  captureReviewAnchor,
  clearReviewHighlights,
  highlightReviewRange,
} from "./reviewDom";
import { normalizeReviewText, resolveReviewAnchor, type ReviewAnchor } from "./reviewAnchors";

type Props = {
  content: string;
  relativePath: string;
  sourcePath: string;
  bundleRoot?: string;
  readOnly: boolean;
  onChange: (content: string) => void;
  onOpenInternal: (path: string) => void;
  onRequestSource: () => void;
  onNotify: (message: string) => void;
};

function normalizeQuote(value: string) {
  return normalizeReviewText(value);
}

export function ReviewEditor({
  content,
  relativePath,
  sourcePath,
  bundleRoot,
  readOnly,
  onChange,
  onOpenInternal,
  onRequestSource,
  onNotify,
}: Props) {
  const documentRef = useRef<HTMLDivElement>(null);
  const commentRefs = useRef(new Map<string, HTMLElement>());
  const review = useMemo(() => splitReviewDocument(content), [content]);
  const [selectionDraft, setSelectionDraft] = useState<{ quote: string; anchor: ReviewAnchor | null } | null>(null);
  const [comment, setComment] = useState("");
  const [activeReviewId, setActiveReviewId] = useState<string | null>(null);
  const [resolvedComments, setResolvedComments] = useState<Record<string, boolean>>({});
  const quote = selectionDraft?.quote || "";

  const captureSelection = () => {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || !selection.anchorNode || !selection.focusNode) return;
    if (!documentRef.current?.contains(selection.anchorNode) || !documentRef.current.contains(selection.focusNode)) return;
    const preview = documentRef.current.querySelector<HTMLElement>(".markdown-preview");
    if (!preview) return;
    const selectedQuote = normalizeQuote(selection.toString()).slice(0, 2_000);
    if (!selectedQuote) return;
    setSelectionDraft({
      quote: selectedQuote,
      anchor: captureReviewAnchor(preview, selection.getRangeAt(0), selectedQuote),
    });
  };

  useEffect(() => {
    const preview = documentRef.current?.querySelector<HTMLElement>(".markdown-preview");
    if (!preview) return;
    clearReviewHighlights(preview);
    const fullText = buildRenderedTextIndex(preview).text;
    const nextResolved: Record<string, boolean> = {};
    review.comments.forEach((item, index) => {
      const resolved = resolveReviewAnchor(fullText, item.quote, item.anchor);
      nextResolved[item.id] = Boolean(resolved);
      if (resolved) highlightReviewRange(preview, resolved, item.id, index + 1);
    });
    setResolvedComments(nextResolved);
    setActiveReviewId((current) => (
      current && review.comments.some((item) => item.id === current) ? current : null
    ));
    return () => clearReviewHighlights(preview);
  }, [review.body, review.comments]);

  useEffect(() => {
    const preview = documentRef.current?.querySelector<HTMLElement>(".markdown-preview");
    if (!preview) return;
    preview.querySelectorAll<HTMLElement>("mark[data-review-id]").forEach((mark) => {
      mark.classList.toggle("active", mark.dataset.reviewId === activeReviewId);
    });
  }, [activeReviewId, resolvedComments]);

  const selectCommentFromDocument = (reviewId: string) => {
    setActiveReviewId(reviewId);
    commentRefs.current.get(reviewId)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  };

  const handleDocumentClick = (event: React.MouseEvent<HTMLDivElement>) => {
    const mark = (event.target as Element).closest<HTMLElement>("mark[data-review-id]");
    if (mark?.dataset.reviewId) selectCommentFromDocument(mark.dataset.reviewId);
  };

  const handleDocumentKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    const mark = (event.target as Element).closest<HTMLElement>("mark[data-review-id]");
    if (!mark?.dataset.reviewId) return;
    event.preventDefault();
    selectCommentFromDocument(mark.dataset.reviewId);
  };

  const revealPassage = (reviewId: string) => {
    setActiveReviewId(reviewId);
    const mark = documentRef.current?.querySelector<HTMLElement>(
      `mark[data-review-id="${CSS.escape(reviewId)}"]`,
    );
    mark?.scrollIntoView({ block: "center", behavior: "smooth" });
    mark?.focus({ preventScroll: true });
  };

  const applyComments = (comments: ReviewComment[]) => {
    const result = setReviewComments(content, comments);
    if (result.error) {
      onNotify(result.error);
      return;
    }
    onChange(result.content);
  };

  const addComment = () => {
    const note = comment.trim();
    if (!selectionDraft || !note || readOnly || review.error) return;
    const id = crypto.randomUUID();
    applyComments([
      ...review.comments,
      {
        id,
        quote: selectionDraft.quote,
        comment: note,
        createdAt: new Date().toISOString(),
        ...(selectionDraft.anchor ? { anchor: selectionDraft.anchor } : {}),
      },
    ]);
    setActiveReviewId(id);
    window.getSelection()?.removeAllRanges();
    setSelectionDraft(null);
    setComment("");
  };

  const copyForAgent = async () => {
    try {
      await navigator.clipboard.writeText(buildReviewPrompt(relativePath, review.comments));
      onNotify("Review prompt copied.");
    } catch (cause) {
      onNotify(`Could not copy the review prompt: ${cause instanceof Error ? cause.message : String(cause)}`);
    }
  };

  if (review.error) {
    return (
      <div className="review-error">
        <div>
          <strong>Review is unavailable for this document.</strong>
          <p>{review.error}</p>
          <button className="toolbar-button" onClick={onRequestSource}>Open Source</button>
        </div>
      </div>
    );
  }

  return (
    <div className="review-workspace">
      <div
        className="review-document"
        ref={documentRef}
        onMouseUp={captureSelection}
        onClick={handleDocumentClick}
        onKeyDown={handleDocumentKeyDown}
      >
        <div className="review-selection-hint">Select text in the document to leave feedback.</div>
        <MarkdownPreview
          content={`${review.frontmatter}${review.body}`}
          sourcePath={sourcePath}
          bundleRoot={bundleRoot}
          onOpenInternal={onOpenInternal}
        />
      </div>
      <aside className="review-panel">
        <header>
          <div>
            <h2>Review</h2>
            <p>{review.comments.length} open {review.comments.length === 1 ? "comment" : "comments"}</p>
          </div>
          <button disabled={!review.comments.length} onClick={() => void copyForAgent()}>Copy for agent</button>
        </header>

        {quote && (
          <section className="review-composer">
            <span>Selected text</span>
            <blockquote>{quote}</blockquote>
            <textarea
              autoFocus
              value={comment}
              placeholder="What should change?"
              onChange={(event) => setComment(event.target.value)}
              onKeyDown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key === "Enter") addComment();
              }}
            />
            <footer>
              <button onClick={() => { setSelectionDraft(null); setComment(""); }}>Cancel</button>
              <button className="primary-button" disabled={!comment.trim() || readOnly} onClick={addComment}>Add comment</button>
            </footer>
          </section>
        )}

        <div className="review-comments">
          {!quote && !review.comments.length && (
            <div className="review-empty">
              <strong>No review comments yet</strong>
              <p>Select a passage on the left, then describe what the agent should change.</p>
            </div>
          )}
          {review.comments.map((item, index) => (
            <article
              className={`review-comment ${activeReviewId === item.id ? "active" : ""} ${resolvedComments[item.id] === false ? "detached" : ""}`}
              key={item.id}
              ref={(element) => {
                if (element) commentRefs.current.set(item.id, element);
                else commentRefs.current.delete(item.id);
              }}
              role="button"
              tabIndex={0}
              aria-label={`Go to review comment ${index + 1}`}
              onClick={() => revealPassage(item.id)}
              onKeyDown={(event) => {
                if (event.target !== event.currentTarget) return;
                if (event.key !== "Enter" && event.key !== " ") return;
                event.preventDefault();
                revealPassage(item.id);
              }}
            >
              <div className="review-comment-number">{index + 1}</div>
              <blockquote>{item.quote}</blockquote>
              <p>{item.comment}</p>
              {resolvedComments[item.id] === false && <div className="review-comment-detached">Passage changed</div>}
              <footer>
                <time dateTime={item.createdAt}>{new Date(item.createdAt).toLocaleString()}</time>
                <button
                  disabled={readOnly}
                  onClick={(event) => {
                    event.stopPropagation();
                    applyComments(review.comments.filter((commentItem) => commentItem.id !== item.id));
                  }}
                >
                  Remove
                </button>
              </footer>
            </article>
          ))}
        </div>

        {!!review.comments.length && (
          <footer className="review-panel-footer">
            <button
              disabled={readOnly}
              onClick={() => {
                if (window.confirm("Remove every review comment from this document?")) applyComments([]);
              }}
            >
              Clear all comments
            </button>
          </footer>
        )}
      </aside>
    </div>
  );
}
