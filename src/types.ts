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

export type TabMode = "source" | "preview" | "diff";

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
  collapsedSections: Record<string, boolean>;
  theme: "dark" | "light";
};

export type FileSystemChange = { paths: string[]; kind: string };
