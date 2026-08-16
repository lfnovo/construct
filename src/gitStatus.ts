import type { LocationGitStatus } from "./types";

export type GitStatusTone = "clean" | "local" | "remote" | "diverged" | "muted" | "warning";

export type GitStatusPresentation = {
  label: string;
  tone: GitStatusTone;
  title: string;
};

export function mergeLocationGitStatus(
  previous: LocationGitStatus | undefined,
  next: LocationGitStatus,
  checkedRemote: boolean,
) {
  const sameRevisions = previous?.headRevision === next.headRevision
    && previous?.trackingRevision === next.trackingRevision;
  if (checkedRemote || !previous?.available || previous.repoRoot !== next.repoRoot || !sameRevisions) return next;
  return {
    ...next,
    remoteState: previous.remoteState,
    checkedAtMs: previous.checkedAtMs,
    message: previous.message,
  };
}

export function gitStatusPresentation(
  status: LocationGitStatus | undefined,
): GitStatusPresentation | null {
  if (!status?.available) return null;

  const dirtySuffix = status.dirty
    ? ` ${status.changedFiles} uncommitted ${status.changedFiles === 1 ? "file" : "files"}.`
    : "";

  if (status.remoteState === "unavailable") {
    return {
      label: "?",
      tone: "warning",
      title: `${status.message || "Could not check the Git remote."}${dirtySuffix}`,
    };
  }
  if (status.remoteState === "noUpstream") {
    return { label: "—", tone: "muted", title: `No upstream branch configured.${dirtySuffix}` };
  }
  if (status.remoteState === "changed") {
    return status.ahead > 0
      ? { label: "↑↓", tone: "diverged", title: `Local commits and remote updates need attention.${dirtySuffix}` }
      : { label: "↓", tone: "remote", title: `The remote branch has changed.${dirtySuffix}` };
  }
  if (status.remoteState === "matchesHead") {
    return status.dirty
      ? { label: "●", tone: "local", title: `${status.changedFiles} uncommitted ${status.changedFiles === 1 ? "file" : "files"}.` }
      : { label: "✓", tone: "clean", title: "Local and remote commits are synchronized." };
  }

  if (status.ahead > 0 && status.behind > 0) {
    return { label: `↑${status.ahead}↓${status.behind}`, tone: "diverged", title: `The branch has diverged from its last fetched upstream.${dirtySuffix}` };
  }
  if (status.ahead > 0) {
    return { label: `↑${status.ahead}`, tone: "local", title: `${status.ahead} local ${status.ahead === 1 ? "commit" : "commits"} to push.${dirtySuffix}` };
  }
  if (status.behind > 0) {
    return { label: `↓${status.behind}`, tone: "remote", title: `${status.behind} fetched upstream ${status.behind === 1 ? "commit is" : "commits are"} not local.${dirtySuffix}` };
  }
  if (status.dirty) {
    return { label: "●", tone: "local", title: `${status.changedFiles} uncommitted ${status.changedFiles === 1 ? "file" : "files"}.` };
  }
  if (status.remoteState === "notChecked") {
    return { label: "?", tone: "muted", title: "The Git remote has not been checked yet." };
  }
  return { label: "✓", tone: "clean", title: "The working tree and tracked remote are synchronized." };
}

export function remoteStatusDescription(status: LocationGitStatus) {
  switch (status.remoteState) {
    case "matchesHead":
      return "Synchronized";
    case "matchesTracking":
      return status.ahead > 0
        ? `Unchanged · ${status.ahead} to push`
        : status.behind > 0
          ? `Unchanged · ${status.behind} to pull`
          : "Unchanged";
    case "changed":
      return "Changed since the last fetch";
    case "unavailable":
      return status.message || "Unavailable";
    case "noUpstream":
      return "No upstream configured";
    case "notChecked":
      return "Not checked";
  }
}
