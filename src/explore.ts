export type ExploreFilters = {
  types: string[];
  tag?: string;
};

export function toggleFilterValue(values: string[], value: string) {
  return values.includes(value)
    ? values.filter((item) => item !== value)
    : [...values, value];
}
