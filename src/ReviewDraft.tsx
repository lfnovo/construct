import { createContext, useContext, useState, type ReactNode } from "react";
import type { ReviewAnchor } from "./reviewAnchors";

export type ReviewSelection = { quote: string; anchor: ReviewAnchor | null };
type ReviewDraft = { selection: ReviewSelection | null; comment: string };
type DraftStore = { read: () => ReviewDraft; write: (draft: ReviewDraft) => void };
const ReviewDraftContext = createContext<DraftStore | null>(null);

// This owner lives above the document boundary. Event-time writes preserve the
// latest keystroke even if the next child render throws, without rerendering App.
export function ReviewDraftProvider({ children }: { children: ReactNode }) {
  const [store] = useState<DraftStore>(() => {
    let draft: ReviewDraft = { selection: null, comment: "" };
    return { read: () => draft, write: (next) => { draft = next; } };
  });
  return <ReviewDraftContext.Provider value={store}>{children}</ReviewDraftContext.Provider>;
}

export function useReviewDraft() {
  const store = useContext(ReviewDraftContext);
  const [draft, setDraft] = useState<ReviewDraft>(() => store?.read() ?? { selection: null, comment: "" });
  const update = (next: ReviewDraft) => {
    store?.write(next);
    setDraft(next);
  };
  return { draft, update };
}
