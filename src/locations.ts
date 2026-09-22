import type { LocationRecord } from "./types";

export function locationNameFromPath(path: string) {
  return path.split(/[\\\\/]/).filter(Boolean).at(-1) || path;
}

export function normalizeLocationName(name: unknown, path: string) {
  return typeof name === "string" && name.trim()
    ? name.trim()
    : locationNameFromPath(path);
}

export function normalizeLocationRecord(location: LocationRecord): LocationRecord {
  return { ...location, name: normalizeLocationName(location.name, location.path) };
}

export function renameLocation(location: LocationRecord, name: string) {
  const trimmed = name.trim();
  return trimmed ? { ...location, name: trimmed } : null;
}
