export type FileEntry = {
  path: string;
  relativePath: string;
  name: string;
  modifiedAtMs: number;
  size: number;
};

export type FileContent = {
  content: string;
  lineEnding: "LF" | "CRLF";
  modifiedAtMs: number;
};

export type GitInfo = {
  available: boolean;
  repoRoot: string | null;
  status: string | null;
  hasHead: boolean;
};

export type GitDiff = {
  available: boolean;
  diff: string;
  message: string | null;
};

export type LocationRecord = {
  id: string;
  path: string;
  name: string;
  available: boolean;
  okfBundle?: boolean;
  okfMode?: "auto" | "manual" | "disabled";
};

export type TerminalApplicationId =
  | "apple-terminal"
  | "iterm2"
  | "ghostty"
  | "wezterm"
  | "windows-terminal";

export type TerminalApplication = {
  id: TerminalApplicationId;
  label: string;
};

export type OpenTerminalResult = {
  application: TerminalApplication;
  locationId: string;
  relativeDirectory: string;
};

export type FileFingerprint = Pick<FileEntry, "path" | "relativePath" | "modifiedAtMs" | "size">;

export type HistoryKind = "created" | "modified" | "renamed" | "removed";

export type HistoryEvent = {
  id: string;
  locationId: string;
  path: string;
  previousPath?: string;
  relativePath: string;
  kind: HistoryKind;
  observedAt: number;
  source: "external" | "app" | "reconciliation";
  available: boolean;
};

export type TabMode = "source" | "preview" | "edit" | "review" | "diff";

export type DocumentTab = {
  id: string;
  path: string;
  locationId: string;
  title: string;
  relativePath: string;
  mode: TabMode;
  content: string;
  baseContent: string;
  lineEnding: "LF" | "CRLF";
  diskModifiedAtMs: number;
  dirty: boolean;
  conflict: boolean;
  deleted: boolean;
  git: GitInfo | null;
};

export type Pane = {
  id: string;
  tabs: DocumentTab[];
  activeTabId: string | null;
};

export type LayoutNode =
  | { type: "pane"; paneId: string }
  | { type: "split"; direction: "horizontal" | "vertical"; ratio: number; first: LayoutNode; second: LayoutNode };

export type SavedTab = Pick<DocumentTab, "id" | "path" | "locationId" | "title" | "relativePath" | "mode">;
export type SavedPane = Omit<Pane, "tabs"> & { tabs: SavedTab[] };

export type SavedWorkspace = {
  locations: LocationRecord[];
  history: HistoryEvent[];
  fingerprints: Record<string, FileFingerprint[]>;
  panes: SavedPane[];
  layout: LayoutNode;
  activePaneId: string;
  selectedLocationId: string | null;
  sidebarWidth: number;
  sidebarHidden?: boolean;
  collapsedSections: Record<string, boolean>;
  theme: "dark" | "light";
  terminalApplicationId?: TerminalApplicationId;
  rememberRecentSearches?: boolean;
  recentSearches?: RecentKnowledgeSearch[];
};

export type FileSystemChange = { paths: string[]; kind: string };

export type IndexState = "notIndexed" | "indexing" | "ready" | "degraded" | "failed";

export type IndexStatus = {
  locationId: string;
  state: IndexState;
  activeGeneration: number | null;
  buildingGeneration: number | null;
  discoveredDocuments: number;
  indexedDocuments: number;
  failedDocuments: number;
  changedDocuments: number;
  removedDocuments: number;
  complete: boolean;
  lastReconciledAt: string | null;
  storageBytes: number;
  error: string | null;
};

export type IndexSearchResult = {
  relativePath: string;
  title: string;
  description: string | null;
  type: string | null;
  tags: string[];
  score: number;
  snippet: string;
  generation: number;
};

export type IndexedDocument = {
  relativePath: string;
  title: string;
  description: string | null;
  type: string | null;
  tags: string[];
  role: string;
  headings: Array<{ level: number; text: string }>;
  frontmatter: unknown | null;
  body: string;
  generation: number;
};

export type RelatedDocument = {
  locationId: string;
  relativePath: string;
  title: string;
  type: string | null;
  tags: string[];
  role: string;
  direction: "outgoing" | "incoming" | "mutual";
  reason: string;
  fragment: string | null;
  generation: number;
};

export type RelatedDocumentsResponse = {
  documents: RelatedDocument[];
  outgoingCount: number;
  incomingCount: number;
  omittedCount: number;
  generation: number;
};

export type ContextDocumentRef = {
  locationId: string;
  relativePath: string;
  reason: string;
};

export type ContextPackItem = ContextDocumentRef & {
  title: string;
  role: string;
  content: string;
  characters: number;
  truncated: boolean;
  generation: number;
};

export type ContextPackOmission = {
  locationId: string;
  relativePath: string;
  reason: string;
};

export type ContextPackResponse = {
  query: string;
  items: ContextPackItem[];
  omitted: ContextPackOmission[];
  totalCharacters: number;
  maxCharacters: number;
  truncated: boolean;
  estimator: "characters";
  markdown: string;
};

export type KnowledgeSearchFilters = {
  types: string[];
  tags: string[];
  roles: string[];
  statuses: string[];
  trust: string[];
  freshness: string[];
  pathPrefix: string;
  findings: "any" | "with" | "without";
};

export type KnowledgeSearchResult = {
  locationId: string;
  relativePath: string;
  title: string;
  description: string | null;
  type: string | null;
  tags: string[];
  role: string;
  status: string | null;
  trust: string | null;
  freshness: "current" | "stale" | "unspecified";
  staleAfter: string | null;
  findingCount: number;
  snippet: string;
  matchedFields: string[];
  matchReason: string;
  score: number;
  rankScore: number;
  generation: number;
};

export type KnowledgeSearchResponse = {
  results: KnowledgeSearchResult[];
  unavailableLocationIds: string[];
};

export type FacetCount = {
  value: string;
  count: number;
};

export type SearchFacets = {
  types: FacetCount[];
  tags: FacetCount[];
  roles: FacetCount[];
  statuses: FacetCount[];
  trust: FacetCount[];
  freshness: FacetCount[];
  unavailableLocationIds: string[];
};

export type RecentKnowledgeSearch = {
  id: string;
  query: string;
  locationIds: string[];
  filters: KnowledgeSearchFilters;
  searchedAt: number;
};
