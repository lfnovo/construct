import {
  AlertTriangle,
  Check,
  ChevronDown,
  Clipboard,
  FileText,
  Search,
  SlidersHorizontal,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { api } from "./api";
import {
  activeFilterCount,
  emptyKnowledgeFilters,
  resultIdentity,
  serializeSearchReferences,
  toggleSearchFilter,
} from "./search";
import type {
  FacetCount,
  IndexStatus,
  KnowledgeSearchFilters,
  KnowledgeSearchResult,
  LocationRecord,
  RecentKnowledgeSearch,
  SearchFacets,
} from "./types";

const emptyFacets: SearchFacets = {
  types: [],
  tags: [],
  roles: [],
  statuses: [],
  trust: [],
  freshness: [],
  unavailableLocationIds: [],
};

function scopeLabel(selected: string[], locations: LocationRecord[]) {
  if (selected.length === locations.length && locations.length > 1) return "All Locations";
  if (selected.length === 1) {
    return locations.find((location) => location.id === selected[0])?.name || "1 Location";
  }
  return `${selected.length} Locations`;
}

function filterLabel(label: string, selected: string[]) {
  return selected.length ? `${label} · ${selected.length}` : label;
}

function displayFacet(value: string) {
  return value.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^./, (letter) => letter.toUpperCase());
}

function HighlightedSnippet({ value, query }: { value: string; query: string }) {
  const terms = [...new Set(query
    .split(/\s+/)
    .map((term) => term.replace(/^"+|"+$/g, "").trim())
    .filter((term) => term.length >= 2))]
    .sort((left, right) => right.length - left.length);
  if (!terms.length) return value;
  const expression = new RegExp(`(${terms.map((term) => term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "gi");
  return value.split(expression).map((part, index) =>
    terms.some((term) => term.toLowerCase() === part.toLowerCase())
      ? <mark key={`${part}-${index}`}>{part}</mark>
      : part);
}

function FacetMenu({ label, values, selected, onToggle, prefix }: {
  label: string;
  values: FacetCount[];
  selected: string[];
  onToggle: (value: string) => void;
  prefix?: string;
}) {
  return <details className="search-filter-menu">
    <summary>{filterLabel(label, selected)} <ChevronDown size={12} /></summary>
    <div className="search-filter-popover">
      {values.length ? values.map((item) => <button
        key={item.value}
        className={selected.includes(item.value) ? "selected" : ""}
        aria-pressed={selected.includes(item.value)}
        onClick={() => onToggle(item.value)}
      >
        <span className="filter-check">{selected.includes(item.value) && <Check size={11} />}</span>
        <span>{prefix}{displayFacet(item.value)}</span>
        <small>{item.count}</small>
      </button>) : <p>No values in this scope.</p>}
    </div>
  </details>;
}

function FilterChips({ label, values, selected, onToggle }: {
  label: string;
  values: FacetCount[];
  selected: string[];
  onToggle: (value: string) => void;
}) {
  if (!values.length) return null;
  return <section className="advanced-filter-group">
    <h3>{label}</h3>
    <div>{values.map((item) => <button
      key={item.value}
      className={selected.includes(item.value) ? "selected" : ""}
      aria-pressed={selected.includes(item.value)}
      onClick={() => onToggle(item.value)}
    >{displayFacet(item.value)} <small>{item.count}</small></button>)}</div>
  </section>;
}

function formatRecentTime(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(timestamp);
}

export function SearchWorkspace({
  locations,
  initialLocationId,
  indexStatuses,
  recentSearches,
  rememberRecentSearches,
  focusSignal,
  onRememberSearch,
  onRecentSearches,
  onRememberRecentSearches,
  onOpen,
  onClose,
  onNotify,
}: {
  locations: LocationRecord[];
  initialLocationId: string | null;
  indexStatuses: Record<string, IndexStatus>;
  recentSearches: RecentKnowledgeSearch[];
  rememberRecentSearches: boolean;
  focusSignal: number;
  onRememberSearch: (query: string, locationIds: string[], filters: KnowledgeSearchFilters) => void;
  onRecentSearches: (searches: RecentKnowledgeSearch[]) => void;
  onRememberRecentSearches: (remember: boolean) => void;
  onOpen: (result: KnowledgeSearchResult, newPane?: boolean) => void;
  onClose: () => void;
  onNotify: (message: string) => void;
}) {
  const availableLocations = useMemo(() => locations.filter((location) => location.available), [locations]);
  const [locationIds, setLocationIds] = useState<string[]>(() => {
    if (initialLocationId && availableLocations.some((location) => location.id === initialLocationId)) {
      return [initialLocationId];
    }
    return availableLocations.map((location) => location.id);
  });
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<KnowledgeSearchFilters>(emptyKnowledgeFilters);
  const [facets, setFacets] = useState<SearchFacets>(emptyFacets);
  const [results, setResults] = useState<KnowledgeSearchResult[]>([]);
  const [unavailable, setUnavailable] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Map<string, KnowledgeSearchResult>>(new Map());
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, [focusSignal]);

  useEffect(() => {
    let cancelled = false;
    if (!locationIds.length) return;
    void api.getSearchFacets(locationIds)
      .then((next) => {
        if (!cancelled) setFacets(next);
      })
      .catch(() => {
        if (!cancelled) setFacets(emptyFacets);
      });
    return () => { cancelled = true; };
  }, [locationIds]);

  useEffect(() => {
    let cancelled = false;
    const trimmed = query.trim();
    if (!trimmed || !locationIds.length) return;
    const timer = window.setTimeout(() => {
      setLoading(true);
      setError(null);
      void api.searchKnowledge({ locationIds, query: trimmed, filters, limit: 100 })
        .then((response) => {
          if (cancelled) return;
          setResults(response.results);
          setUnavailable(response.unavailableLocationIds);
        })
        .catch((caught) => {
          if (!cancelled) {
            setResults([]);
            setError(caught instanceof Error ? caught.message : String(caught));
          }
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [filters, locationIds, query]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (query.trim() && rememberRecentSearches) {
      onRememberSearch(query, locationIds, filters);
    }
  };

  const toggleLocation = (locationId: string) => {
    const next = locationIds.includes(locationId)
      ? (locationIds.length === 1 ? locationIds : locationIds.filter((id) => id !== locationId))
      : [...locationIds, locationId];
    setLocationIds(next);
    setFacets(emptyFacets);
    setSelected((current) => new Map(
      [...current].filter(([, result]) => next.includes(result.locationId)),
    ));
  };

  const updateList = (field: "types" | "tags" | "roles" | "statuses" | "trust" | "freshness", value: string) => {
    setFilters((current) => ({ ...current, [field]: toggleSearchFilter(current[field], value) }));
  };

  const restoreRecent = (recent: RecentKnowledgeSearch) => {
    const validLocations = recent.locationIds.filter((id) => availableLocations.some((location) => location.id === id));
    const nextLocations = validLocations.length ? validLocations : availableLocations.map((location) => location.id);
    setLocationIds(nextLocations);
    setFacets(emptyFacets);
    setSelected((current) => new Map(
      [...current].filter(([, result]) => nextLocations.includes(result.locationId)),
    ));
    setFilters({ ...emptyKnowledgeFilters(), ...recent.filters });
    setQuery(recent.query);
  };

  const selectedResults = [...selected.values()];
  const copyReferences = async () => {
    await navigator.clipboard.writeText(serializeSearchReferences(selectedResults, locations));
    onNotify(`${selectedResults.length} reference${selectedResults.length === 1 ? "" : "s"} copied.`);
  };
  const readyCount = locationIds.filter((id) => ["ready", "degraded"].includes(indexStatuses[id]?.state)).length;
  const filterCount = activeFilterCount(filters);

  return <section className="search-workspace">
    <header className="search-header">
      <div>
        <h1>Search</h1>
        <p>Find knowledge across saved Markdown and OKF metadata.</p>
      </div>
      <button className="toolbar-button" onClick={onClose}>Back to workspace</button>
    </header>

    <form className="knowledge-search-form" onSubmit={submit}>
      <Search size={18} />
      <input
        ref={inputRef}
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          if (!event.target.value.trim()) {
            setResults([]);
            setUnavailable([]);
            setLoading(false);
            setError(null);
          }
        }}
        onKeyDown={(event) => { if (event.key === "Escape") onClose(); }}
        placeholder="Search concepts, claims, headings, tags…"
        aria-label="Search knowledge"
      />
      {query && <button type="button" aria-label="Clear search" onClick={() => {
        setQuery("");
        setResults([]);
        setUnavailable([]);
        setLoading(false);
        setError(null);
      }}><X size={15} /></button>}
      <kbd>⌘⇧F</kbd>
    </form>

    <div className="search-filter-bar">
      <details className="search-filter-menu">
        <summary>{scopeLabel(locationIds, availableLocations)} <ChevronDown size={12} /></summary>
        <div className="search-filter-popover">
          {availableLocations.map((location) => <button
            key={location.id}
            className={locationIds.includes(location.id) ? "selected" : ""}
            aria-pressed={locationIds.includes(location.id)}
            onClick={() => toggleLocation(location.id)}
          >
            <span className="filter-check">{locationIds.includes(location.id) && <Check size={11} />}</span>
            <span>{location.name}</span>
            <small>{indexStatuses[location.id]?.indexedDocuments || 0}</small>
          </button>)}
        </div>
      </details>
      <FacetMenu label="Types" values={facets.types} selected={filters.types} onToggle={(value) => updateList("types", value)} />
      <FacetMenu label="Tags" values={facets.tags} selected={filters.tags} onToggle={(value) => updateList("tags", value)} prefix="#" />
      <details className="search-more-filters">
        <summary><SlidersHorizontal size={13} /> More filters{filterCount > filters.types.length + filters.tags.length ? ` · ${filterCount - filters.types.length - filters.tags.length}` : ""}</summary>
        <div className="advanced-filter-panel">
          <label>Path prefix<input value={filters.pathPrefix} onChange={(event) => setFilters((current) => ({ ...current, pathPrefix: event.target.value }))} placeholder="people/" /></label>
          <label>Findings<select value={filters.findings} onChange={(event) => setFilters((current) => ({ ...current, findings: event.target.value as KnowledgeSearchFilters["findings"] }))}><option value="any">Any</option><option value="with">With findings</option><option value="without">Without findings</option></select></label>
          <FilterChips label="Document role" values={facets.roles} selected={filters.roles} onToggle={(value) => updateList("roles", value)} />
          <FilterChips label="Lifecycle status" values={facets.statuses} selected={filters.statuses} onToggle={(value) => updateList("statuses", value)} />
          <FilterChips label="Trust" values={facets.trust} selected={filters.trust} onToggle={(value) => updateList("trust", value)} />
          <FilterChips label="Freshness" values={facets.freshness} selected={filters.freshness} onToggle={(value) => updateList("freshness", value)} />
        </div>
      </details>
      {filterCount > 0 && <button className="clear-search-filters" onClick={() => setFilters(emptyKnowledgeFilters())}>Clear filters</button>}
      <span className="search-index-summary">{readyCount}/{locationIds.length} indexes ready</span>
    </div>

    {!query.trim() ? <div className="search-empty">
      <section>
        <div className="search-empty-heading"><div><h2>Recent searches</h2><p>Stored only on this device.</p></div><label><input type="checkbox" checked={rememberRecentSearches} onChange={(event) => onRememberRecentSearches(event.target.checked)} /> Remember</label></div>
        {rememberRecentSearches && recentSearches.length ? <div className="recent-searches">{recentSearches.map((recent) => <button key={recent.id} onClick={() => restoreRecent(recent)}><Search size={13} /><span><strong>{recent.query}</strong><small>{recent.locationIds.length} Location{recent.locationIds.length === 1 ? "" : "s"} · {formatRecentTime(recent.searchedAt)}</small></span></button>)}</div> : <p className="search-empty-copy">{rememberRecentSearches ? "Run a search and press Enter to keep it here." : "Recent searches are disabled."}</p>}
        {!!recentSearches.length && <button className="clear-recent-searches" onClick={() => onRecentSearches([])}>Clear recent searches</button>}
      </section>
      <section>
        <h2>Browse the current scope</h2>
        <p>Start with a common type or tag, then add a query.</p>
        <div className="search-empty-facets">{facets.types.slice(0, 8).map((facet) => <button key={`type-${facet.value}`} onClick={() => updateList("types", facet.value)}>{facet.value}<small>{facet.count}</small></button>)}{facets.tags.slice(0, 8).map((facet) => <button key={`tag-${facet.value}`} onClick={() => updateList("tags", facet.value)}>#{facet.value}<small>{facet.count}</small></button>)}</div>
      </section>
    </div> : <div className="search-results-region">
      <div className="search-results-heading">
        <div><h2>{loading ? "Searching…" : `${results.length} result${results.length === 1 ? "" : "s"}`}</h2>{unavailable.length > 0 && <p><AlertTriangle size={12} /> {unavailable.length} Location index{unavailable.length === 1 ? " is" : "es are"} unavailable.</p>}</div>
        {selectedResults.length > 0 && <div className="search-selection-actions"><span>{selectedResults.length} selected</span><button onClick={() => void copyReferences()}><Clipboard size={13} /> Copy references</button><button aria-label="Clear selection" onClick={() => setSelected(new Map())}><X size={13} /></button></div>}
      </div>
      {error ? <div className="search-message error"><AlertTriangle size={17} /><div><strong>Search failed</strong><p>{error}</p></div></div>
        : !loading && !results.length ? <div className="search-message"><Search size={17} /><div><strong>No knowledge found</strong><p>Try fewer filters or a different phrase.</p></div></div>
          : <div className="knowledge-results">{results.map((result) => {
            const identity = resultIdentity(result);
            const location = locations.find((item) => item.id === result.locationId);
            const isSelected = selected.has(identity);
            return <article key={identity} className={isSelected ? "selected" : ""}>
              <button className="result-selector" aria-label={`${isSelected ? "Remove" : "Add"} ${result.title} ${isSelected ? "from" : "to"} selection`} aria-pressed={isSelected} onClick={() => setSelected((current) => {
                const next = new Map(current);
                if (next.has(identity)) next.delete(identity);
                else next.set(identity, result);
                return next;
              })}>{isSelected && <Check size={12} />}</button>
              <button className="result-main" onClick={() => onOpen(result)}>
                <div className="result-title-line"><FileText size={14} /><strong>{result.title}</strong>{result.type && <span>{result.type}</span>}</div>
                {result.snippet && <p><HighlightedSnippet value={result.snippet} query={query} /></p>}
                <footer><span>{location?.name || result.locationId} · {result.relativePath}</span><span>{result.matchReason}</span>{result.findingCount > 0 && <span className="result-warning">{result.findingCount} finding{result.findingCount === 1 ? "" : "s"}</span>}</footer>
              </button>
              <button className="result-open-right" onClick={() => onOpen(result, true)}>Open right</button>
            </article>;
          })}</div>}
    </div>}
  </section>;
}
