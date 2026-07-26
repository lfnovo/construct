import { useMemo, useRef, useState } from "react";
import { MarkdownPreview } from "./MarkdownPreview";
import {
  buildReviewPrompt,
  setReviewComments,
  splitReviewDocument,
  type ReviewComment,
} from "./review";

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
  return value.replace(/\s+/g, " ").trim();
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
  const review = useMemo(() => splitReviewDocument(content), [content]);
  const [quote, setQuote] = useState("");
  const [comment, setComment] = useState("");

  const captureSelection = () => {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || !selection.anchorNode || !selection.focusNode) return;
    if (!documentRef.current?.contains(selection.anchorNode) || !documentRef.current.contains(selection.focusNode)) return;
    setQuote(normalizeQuote(selection.toString()).slice(0, 2_000));
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
    if (!quote || !note || readOnly || review.error) return;
    applyComments([
      ...review.comments,
      {
        id: crypto.randomUUID(),
        quote,
        comment: note,
        createdAt: new Date().toISOString(),
      },
    ]);
    window.getSelection()?.removeAllRanges();
    setQuote("");
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
      <div className="review-document" ref={documentRef} onMouseUp={captureSelection}>
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
              <button onClick={() => { setQuote(""); setComment(""); }}>Cancel</button>
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
            <article className="review-comment" key={item.id}>
              <div className="review-comment-number">{index + 1}</div>
              <blockquote>{item.quote}</blockquote>
              <p>{item.comment}</p>
              <footer>
                <time dateTime={item.createdAt}>{new Date(item.createdAt).toLocaleString()}</time>
                <button disabled={readOnly} onClick={() => applyComments(review.comments.filter((commentItem) => commentItem.id !== item.id))}>Remove</button>
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
