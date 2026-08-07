import assert from "node:assert/strict";
import test from "node:test";
import { gitStatusPresentation, mergeLocationGitStatus, remoteStatusDescription } from "../src/gitStatus.ts";

const clean = {
  available: true,
  repoRoot: "/repo",
  branch: "main",
  upstream: "origin/main",
  headRevision: "head-1",
  trackingRevision: "head-1",
  dirty: false,
  changedFiles: 0,
  ahead: 0,
  behind: 0,
  remoteState: "matchesHead",
  checkedAtMs: 1,
  message: null,
};

test("hides Git status for folders outside a repository", () => {
  assert.equal(gitStatusPresentation({ ...clean, available: false }), null);
});

test("prioritizes a changed remote over a dirty working tree", () => {
  assert.deepEqual(
    gitStatusPresentation({ ...clean, dirty: true, changedFiles: 2, remoteState: "changed" }),
    {
      label: "↓",
      tone: "remote",
      title: "The remote branch has changed. 2 uncommitted files.",
    },
  );
});

test("shows local and remote attention together", () => {
  assert.equal(
    gitStatusPresentation({ ...clean, ahead: 2, remoteState: "changed" })?.label,
    "↑↓",
  );
});

test("shows reliable ahead and behind counts from the tracking ref", () => {
  assert.equal(
    gitStatusPresentation({ ...clean, ahead: 3, behind: 1, remoteState: "matchesTracking" })?.label,
    "↑3↓1",
  );
});

test("describes a failed remote check without losing local state", () => {
  const status = { ...clean, ahead: 1, remoteState: "unavailable", message: "Remote check timed out." };
  assert.equal(gitStatusPresentation(status)?.label, "?");
  assert.equal(remoteStatusDescription(status), "Remote check timed out.");
});

test("a local refresh preserves the last remote observation", () => {
  const previous = { ...clean, checkedAtMs: 42, remoteState: "changed" };
  const next = { ...clean, dirty: true, changedFiles: 1, checkedAtMs: null, remoteState: "notChecked" };
  assert.deepEqual(mergeLocationGitStatus(previous, next, false), {
    ...next,
    checkedAtMs: 42,
    remoteState: "changed",
  });
  assert.equal(mergeLocationGitStatus(previous, next, true), next);
});

test("a new local revision invalidates an earlier synchronized observation", () => {
  const previous = { ...clean, checkedAtMs: 42, remoteState: "matchesHead" };
  const next = {
    ...clean,
    headRevision: "head-2",
    ahead: 1,
    checkedAtMs: null,
    remoteState: "notChecked",
  };
  assert.equal(mergeLocationGitStatus(previous, next, false), next);
  assert.equal(gitStatusPresentation(next)?.label, "↑1");
});
