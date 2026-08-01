export type ExploreFilters = {
  types: string[];
  tag?: string;
};

export type FacetCount = readonly [name: string, count: number];

export const TAG_PREVIEW_LIMIT = 20;

const OKF_TYPE_COLORS = [
  "#8b8cf8",
  "#45bfa9",
  "#e4a85e",
  "#db6f91",
  "#63a9ee",
  "#a67de0",
  "#71b96b",
  "#dd7a62",
  "#5fc0db",
  "#c99b45",
];

export function toggleFilterValue(values: string[], value: string) {
  return values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value];
}

export function buildTypeColorMap(types: string[]) {
  const uniqueTypes = [...new Set(types)].sort((left, right) => left.localeCompare(right));
  return Object.fromEntries(uniqueTypes.map((type, index) => [
    type,
    OKF_TYPE_COLORS[index] || `hsl(${Math.round((index * 137.508) % 360)} 62% 62%)`,
  ]));
}

export function sortFacetsByCount(facets: FacetCount[]) {
  return [...facets].sort(
    ([leftName, leftCount], [rightName, rightCount]) => rightCount - leftCount || leftName.localeCompare(rightName),
  );
}

export function visibleTagFacets(
  tags: FacetCount[],
  selectedTag: string | undefined,
  expanded: boolean,
  limit = TAG_PREVIEW_LIMIT,
) {
  if (expanded || tags.length <= limit) return tags;

  const visible = tags.slice(0, limit);
  const selected = selectedTag && tags.find(([tag]) => tag === selectedTag);
  return selected && !visible.some(([tag]) => tag === selectedTag)
    ? [...visible, selected]
    : visible;
}
