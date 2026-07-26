import type { HistoryEvent } from "./types";

function historyPathKey(event: Pick<HistoryEvent, "locationId">, path: string) {
  return `${event.locationId}\0${path}`;
}

export function deduplicateHistory(events: HistoryEvent[]) {
  const parent = new Map<string, string>();

  const find = (key: string): string => {
    const current = parent.get(key);
    if (!current) {
      parent.set(key, key);
      return key;
    }
    if (current === key) return key;
    const root = find(current);
    parent.set(key, root);
    return root;
  };

  const union = (left: string, right: string) => {
    const leftRoot = find(left);
    const rightRoot = find(right);
    if (leftRoot !== rightRoot) parent.set(rightRoot, leftRoot);
  };

  for (const event of events) {
    const current = historyPathKey(event, event.path);
    find(current);
    if (event.previousPath) union(current, historyPathKey(event, event.previousPath));
  }

  const latestByFile = new Map<string, HistoryEvent>();
  for (const event of [...events].sort((left, right) => right.observedAt - left.observedAt)) {
    const identity = find(historyPathKey(event, event.path));
    if (!latestByFile.has(identity)) latestByFile.set(identity, event);
  }

  return [...latestByFile.values()].sort((left, right) => right.observedAt - left.observedAt);
}
