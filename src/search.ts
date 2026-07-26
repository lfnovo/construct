import type {
  KnowledgeSearchFilters,
  KnowledgeSearchResult,
  LocationRecord,
  RecentKnowledgeSearch,
} from "./types";

export const emptyKnowledgeFilters = (): KnowledgeSearchFilters => ({
  types: [],
  tags: [],
  roles: [],
  statuses: [],
  trust: [],
  freshness: [],
  pathPrefix: "",
  findings: "any",
});

export function toggleSearchFilter(values: string[], value: string) {
  return values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value];
}

export function activeFilterCount(filters: KnowledgeSearchFilters) {
  return filters.types.length
    + filters.tags.length
    + filters.roles.length
    + filters.statuses.length
    + filters.trust.length
    + filters.freshness.length
    + Number(Boolean(filters.pathPrefix))
    + Number(filters.findings !== "any");
}

export function rememberSearch(
  recent: RecentKnowledgeSearch[],
  search: Omit<RecentKnowledgeSearch, "id" | "searchedAt">,
  now = Date.now(),
) {
  const signature = JSON.stringify({
    query: search.query.trim(),
    locationIds: [...search.locationIds].sort(),
    filters: search.filters,
  });
  const withoutDuplicate = recent.filter((item) => JSON.stringify({
    query: item.query.trim(),
    locationIds: [...item.locationIds].sort(),
    filters: item.filters,
  }) !== signature);
  return [{
    ...search,
    id: crypto.randomUUID(),
    query: search.query.trim(),
    searchedAt: now,
  }, ...withoutDuplicate].slice(0, 20);
}

export function resultIdentity(result: Pick<KnowledgeSearchResult, "locationId" | "relativePath">) {
  return `${result.locationId}:${result.relativePath}`;
}

export function serializeSearchReferences(
  results: KnowledgeSearchResult[],
  locations: LocationRecord[],
) {
  const locationNames = new Map(locations.map((location) => [location.id, location.name]));
  const lines = results.map((result) => {
    const location = locationNames.get(result.locationId) || result.locationId;
    return `- ${result.title} — ${location}:${result.relativePath} (${result.matchReason.toLowerCase()})`;
  });
  return [
    "Selected Construct references:",
    "",
    ...lines,
  ].join("\n");
}
