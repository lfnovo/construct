import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { Bot, ChevronDown, ChevronRight, CirclePlus, Clipboard, Columns2, FileText, Folder, FolderOpen, History, List, MapPin, MoreHorizontal, Moon, Network, PanelLeftClose, PanelLeftOpen, Rows3, Search as SearchIcon, Settings2, ShieldCheck, SquareTerminal, Sun, X } from "lucide-react";
import { api } from "./api";
import { CodeEditor } from "./CodeEditor";
import { DocumentModeSurface } from "./DocumentModeSurface";
import { buildTypeColorMap, toggleFilterValue, type ExploreFilters } from "./explore";
import { HealthWorkspace } from "./HealthWorkspace";
import { deduplicateHistory } from "./history";
import { KnowledgeGraph } from "./KnowledgeGraph";
import { MarkdownPreview } from "./MarkdownPreview";
import { formatOkfValue, type OkfBundleIndex, type OkfConcept, type OkfInspection } from "./okf";
import { ReviewEditor } from "./ReviewEditor";
import { splitReviewDocument } from "./review";
import { SearchWorkspace } from "./SearchWorkspace";
import { rememberSearch } from "./search";
import { moveQuickOpenSelection } from "./quickOpen";
import { pathBelongsToLocation, pathIdentity, pathsEqual } from "./paths";
import {
  defaultSidebarPanelSizes,
  resizeSidebarPanelPair,
  sanitizeSidebarPanelSizes,
  sidebarSectionIds,
} from "./sidebar";
import { relativeDirectoryForFile, selectedTerminal } from "./terminal";
import type {
  DocumentTab, FileEntry, FileFingerprint, FileSystemChange, HistoryEvent, HistoryKind,
  IndexStatus, KnowledgeSearchFilters, KnowledgeSearchResult, LayoutNode, LocationRecord,
  Pane, RecentKnowledgeSearch, SavedPane, SavedWorkspace, SidebarPanelSizes, SidebarSectionId,
  TabMode, TerminalApplication, TerminalApplicationId,
} from "./types";
import type { DocumentModeTransfer, DocumentViewState } from "./documentPosition";

const VisualEditor = lazy(() => import("./VisualEditor").then(({ VisualEditor: Component }) => ({ default: Component })));
const modeLabels: Record<TabMode, string> = {
  preview: "Preview",
  edit: "Edit",
  review: "Review",
  source: "Source",
  diff: "Diff",
};

const emptyPane = (id: string = crypto.randomUUID()): Pane => ({ id, tabs: [], activeTabId: null });
const defaultPane = emptyPane("main");
const defaultLayout: LayoutNode = { type: "pane", paneId: "main" };

function getPaneIds(node: LayoutNode): string[] {
  return node.type === "pane" ? [node.paneId] : [...getPaneIds(node.first), ...getPaneIds(node.second)];
}

function updateLayoutPane(node: LayoutNode, paneId: string, replacement: LayoutNode): LayoutNode {
  if (node.type === "pane") return node.paneId === paneId ? replacement : node;
  return { ...node, first: updateLayoutPane(node.first, paneId, replacement), second: updateLayoutPane(node.second, paneId, replacement) };
}

function removeLayoutPane(node: LayoutNode, paneId: string): LayoutNode | null {
  if (node.type === "pane") return node.paneId === paneId ? null : node;
  const first = removeLayoutPane(node.first, paneId);
  const second = removeLayoutPane(node.second, paneId);
  if (!first) return second;
  if (!second) return first;
  return { ...node, first, second };
}

function updateSplitRatio(node: LayoutNode, target: LayoutNode, ratio: number): LayoutNode {
  if (node === target && node.type === "split") return { ...node, ratio };
  if (node.type === "pane") return node;
  return { ...node, first: updateSplitRatio(node.first, target, ratio), second: updateSplitRatio(node.second, target, ratio) };
}

function basename(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}

function normalizeForSave(content: string, ending: "LF" | "CRLF") {
  const normalized = content.replace(/\r\n/g, "\n");
  return ending === "CRLF" ? normalized.replace(/\n/g, "\r\n") : normalized;
}

function formatWhen(timestamp: number) {
  const delta = Math.max(0, Date.now() - timestamp);
  if (delta < 60_000) return "now";
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)}m`;
  if (delta < 86_400_000) return `${Math.floor(delta / 3_600_000)}h`;
  return `${Math.floor(delta / 86_400_000)}d`;
}

function statusLabel(kind: HistoryKind) {
  return { created: "New", modified: "Changed", renamed: "Renamed", removed: "Removed" }[kind];
}

function indexStatusTitle(status: IndexStatus | undefined) {
  if (!status || status.state === "notIndexed") return "Local index has not been built yet.";
  const size = status.storageBytes < 1024 * 1024
    ? `${Math.max(1, Math.round(status.storageBytes / 1024))} KB`
    : `${(status.storageBytes / 1024 / 1024).toFixed(1)} MB`;
  const label = status.state === "indexing" ? "Indexing"
    : status.state === "ready" ? "Index ready"
      : status.state === "degraded" ? "Index ready with warnings"
        : "Index failed";
  return `${label} · ${status.indexedDocuments} documents · ${size}. Click to rebuild.`;
}

type TreeNode = { children: Map<string, TreeNode>; entry?: FileEntry };
type TerminalTarget = { locationId: string; relativeDirectory: string };
type McpAccessMode = "current" | "custom" | "all";
type McpDialogState = { mode: McpAccessMode; locationIds: string[] };

function makeTree(entries: FileEntry[]) {
  const root: TreeNode = { children: new Map() };
  for (const entry of entries) {
    const pieces = entry.relativePath.split(/[\\/]/);
    let node = root;
    for (const piece of pieces) {
      if (!node.children.has(piece)) node.children.set(piece, { children: new Map() });
      node = node.children.get(piece)!;
    }
    node.entry = entry;
  }
  return root;
}

function FileTree({ entries, onOpen, onContext }: { entries: FileEntry[]; onOpen: (file: FileEntry, newPane?: boolean) => void; onContext: (event: React.MouseEvent, file: FileEntry) => void }) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const tree = useMemo(() => makeTree(entries), [entries]);
  const render = (node: TreeNode, prefix: string, depth = 0): React.ReactNode[] => Array.from(node.children.entries())
    .sort(([leftName, left], [rightName, right]) => Number(!left.entry) - Number(!right.entry) || leftName.localeCompare(rightName))
    .flatMap(([name, child]) => {
      const key = `${prefix}/${name}`;
      if (child.entry) {
        return <button className="file-row" style={{ paddingLeft: 8 + depth * 15 }} key={key} title={child.entry.relativePath} onClick={() => onOpen(child.entry!)} onContextMenu={(event) => onContext(event, child.entry!)}>
          <FileText className="file-icon" size={14} strokeWidth={1.8} /><span>{name}</span>
        </button>;
      }
      const isOpen = expanded.has(key);
      return [
        <button className="folder-row" style={{ paddingLeft: 7 + depth * 15 }} key={key} onClick={() => setExpanded((current) => { const next = new Set(current); if (isOpen) next.delete(key); else next.add(key); return next; })}>
          {isOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />} {isOpen ? <FolderOpen className="folder-icon" size={14} /> : <Folder className="folder-icon" size={14} />}<span>{name}</span>
        </button>,
        ...(isOpen ? render(child, key, depth + 1) : []),
      ];
    });
  return <div className="file-tree">{entries.length ? render(tree, "") : <p className="empty-sidebar">No Markdown files found.</p>}</div>;
}

function BundleExplorer({ location, index, filters, onFilters, onOpen, onOpenFinding, onRefreshHealth, onNotify, onClose }: {
  location: LocationRecord;
  index: OkfBundleIndex | undefined;
  filters: ExploreFilters;
  onFilters: (filters: ExploreFilters) => void;
  onOpen: (path: string) => void;
  onOpenFinding: (relativePath: string) => void;
  onRefreshHealth: () => Promise<void>;
  onNotify: (message: string) => void;
  onClose: () => void;
}) {
  const [view, setView] = useState<"list" | "graph" | "health">("list");
  if (!index || index.status === "scanning") return <section className="bundle-explorer"><header><div><h1>{location.name}</h1><p>Building the local knowledge index…</p></div><button className="toolbar-button" onClick={onClose}>Back to workspace</button></header></section>;
  if (index.status === "error") return <section className="bundle-explorer"><header><div><h1>{location.name}</h1><p>Could not build the knowledge index.</p></div><button className="toolbar-button" onClick={onClose}>Back to workspace</button></header></section>;
  const typeCounts = new Map<string, number>();
  const tagCounts = new Map<string, number>();
  for (const concept of index.concepts) {
    typeCounts.set(concept.type, (typeCounts.get(concept.type) || 0) + 1);
    for (const tag of concept.tags) tagCounts.set(tag, (tagCounts.get(tag) || 0) + 1);
  }
  const concepts = index.concepts.filter((concept) => (!filters.types.length || filters.types.includes(concept.type)) && (!filters.tag || concept.tags.includes(filters.tag)));
  const types = [...typeCounts.entries()].sort(([left], [right]) => left.localeCompare(right));
  const typeColors = buildTypeColorMap(types.map(([type]) => type));
  const tags = [...tagCounts.entries()].sort(([leftName, leftCount], [rightName, rightCount]) => rightCount - leftCount || leftName.localeCompare(rightName));
  return <section className="bundle-explorer">
    <header><div><h1>{location.name}</h1><p>{index.declaredVersion ? `OKF ${index.declaredVersion}` : "OKF"} bundle · {index.concepts.length} concepts · {types.length} types · {tags.length} tags{index.findingCount ? ` · ${index.findingCount} findings` : ""}</p></div><div className="explore-header-actions"><div className="explore-view-switch" aria-label="Explore view"><button className={view === "list" ? "selected" : ""} onClick={() => setView("list")}><List size={13} /> List</button><button className={view === "graph" ? "selected" : ""} onClick={() => setView("graph")}><Network size={13} /> Graph</button><button className={view === "health" ? "selected" : ""} onClick={() => setView("health")}><ShieldCheck size={13} /> Health{index.findingCount ? <span>{index.findingCount}</span> : null}</button></div><button className="toolbar-button" onClick={onClose}>Back to workspace</button></div></header>
    {view === "health" ? <HealthWorkspace
      locationName={location.name}
      documents={index.documentCount || 0}
      findings={index.findings || []}
      ignoredPaths={index.ignoredPaths || []}
      onOpen={onOpenFinding}
      onRefresh={onRefreshHealth}
      onNotify={onNotify}
    /> : <>
      <div className="explore-facets"><section><h2>Browse by type</h2><div className="facet-list types">{types.map(([type, count]) => <button key={type} className={filters.types.includes(type) ? "selected" : ""} style={{ "--type-color": typeColors[type] } as CSSProperties} aria-pressed={filters.types.includes(type)} onClick={() => onFilters({ ...filters, types: toggleFilterValue(filters.types, type) })}><i className="type-color-dot" />{type}<span>{count}</span></button>)}</div></section><section><h2>Browse by tag</h2><div className="facet-list tags">{tags.map(([tag, count]) => <button key={tag} className={filters.tag === tag ? "selected" : ""} aria-pressed={filters.tag === tag} onClick={() => onFilters({ ...filters, tag: filters.tag === tag ? undefined : tag })}>#{tag}<span>{count}</span></button>)}</div></section></div>
      <div className="explore-results-heading"><h2>{filters.types.length || filters.tag ? `${concepts.length} matching concepts` : view === "graph" ? "Knowledge graph" : "All concepts"}</h2>{(filters.types.length || filters.tag) && <button onClick={() => onFilters({ types: [] })}>Clear filters</button>}</div>
      {view === "graph" ? <KnowledgeGraph concepts={concepts} typeColors={typeColors} onOpen={onOpen} /> : <div className="concept-results">{concepts.map((concept) => <button key={concept.path} onClick={() => onOpen(concept.path)}><div><strong>{concept.title}</strong>{concept.description && <p>{concept.description}</p>}<small>{concept.relativePath}</small></div><aside><span className="concept-type" style={{ "--type-color": typeColors[concept.type] } as CSSProperties}>{concept.type}</span>{concept.tags.slice(0, 3).map((tag) => <em key={tag}>#{tag}</em>)}</aside></button>)}</div>}
    </>}
  </section>;
}

function SplitView({ node, panes, activePaneId, onActivate, onRatio, children }: {
  node: LayoutNode;
  panes: Record<string, Pane>;
  activePaneId: string;
  onActivate: (id: string) => void;
  onRatio: (node: LayoutNode, ratio: number) => void;
  children: (pane: Pane, active: boolean) => React.ReactNode;
}) {
  const container = useRef<HTMLDivElement>(null);
  if (node.type === "pane") return <div className="pane-wrap" onMouseDown={() => onActivate(node.paneId)}>{children(panes[node.paneId], activePaneId === node.paneId)}</div>;
  const horizontal = node.direction === "horizontal";
  const resize = (event: React.PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const start = horizontal ? event.clientX : event.clientY;
    const bounds = container.current?.getBoundingClientRect();
    const size = horizontal ? bounds?.width : bounds?.height;
    const initial = node.ratio;
    const onMove = (move: PointerEvent) => {
      if (!size) return;
      const offset = (horizontal ? move.clientX : move.clientY) - start;
      onRatio(node, Math.max(0.18, Math.min(0.82, initial + offset / size)));
    };
    const onUp = () => { window.removeEventListener("pointermove", onMove); window.removeEventListener("pointerup", onUp); };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };
  return <div ref={container} className={`split ${horizontal ? "split-horizontal" : "split-vertical"}`}>
    <div style={horizontal ? { width: `${node.ratio * 100}%` } : { height: `${node.ratio * 100}%` }}><SplitView node={node.first} panes={panes} activePaneId={activePaneId} onActivate={onActivate} onRatio={onRatio}>{children}</SplitView></div>
    <div className={`split-handle ${horizontal ? "horizontal" : "vertical"}`} onPointerDown={resize} />
    <div style={horizontal ? { width: `${(1 - node.ratio) * 100}%` } : { height: `${(1 - node.ratio) * 100}%` }}><SplitView node={node.second} panes={panes} activePaneId={activePaneId} onActivate={onActivate} onRatio={onRatio}>{children}</SplitView></div>
  </div>;
}

export default function App() {
  const [locations, setLocations] = useState<LocationRecord[]>([]);
  const [filesByLocation, setFilesByLocation] = useState<Record<string, FileEntry[]>>({});
  const [fingerprints, setFingerprints] = useState<Record<string, FileFingerprint[]>>({});
  const [history, setHistory] = useState<HistoryEvent[]>([]);
  const [panes, setPanes] = useState<Record<string, Pane>>({ main: defaultPane });
  const [layout, setLayout] = useState<LayoutNode>(defaultLayout);
  const [activePaneId, setActivePaneId] = useState("main");
  const [selectedLocationId, setSelectedLocationId] = useState<string | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(295);
  const [sidebarHidden, setSidebarHidden] = useState(false);
  const [showOkfInspector, setShowOkfInspector] = useState(false);
  const [okfIndexes, setOkfIndexes] = useState<Record<string, OkfBundleIndex>>({});
  const [indexStatuses, setIndexStatuses] = useState<Record<string, IndexStatus>>({});
  const [okfInspections, setOkfInspections] = useState<Record<string, { content: string; inspection: OkfInspection }>>({});
  const [exploreLocationId, setExploreLocationId] = useState<string | null>(null);
  const [exploreFilters, setExploreFilters] = useState<ExploreFilters>({ types: [] });
  const [searchSession, setSearchSession] = useState<{ initialLocationId: string | null; focusSignal: number } | null>(null);
  const [recentSearches, setRecentSearches] = useState<RecentKnowledgeSearch[]>([]);
  const [rememberRecentSearches, setRememberRecentSearches] = useState(true);
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({});
  const [sidebarPanelSizes, setSidebarPanelSizes] = useState<SidebarPanelSizes>(
    defaultSidebarPanelSizes,
  );
  const [theme, setTheme] = useState<"dark" | "light">("dark");
  const [terminalApplications, setTerminalApplications] = useState<TerminalApplication[]>([]);
  const [terminalApplicationId, setTerminalApplicationId] = useState<TerminalApplicationId>();
  const [terminalPicker, setTerminalPicker] = useState<{ target: TerminalTarget | null } | null>(null);
  const [mcpDialog, setMcpDialog] = useState<McpDialogState | null>(null);
  const [ready, setReady] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [quickOpen, setQuickOpen] = useState(false);
  const [quickOpenSelection, setQuickOpenSelection] = useState(0);
  const [query, setQuery] = useState("");
  const [historyFilter, setHistoryFilter] = useState<HistoryKind | "all">("all");
  const [fileContext, setFileContext] = useState<{ file: FileEntry; locationId: string; x: number; y: number } | null>(null);
  const [tabContext, setTabContext] = useState<{ tab: DocumentTab; paneId: string; x: number; y: number } | null>(null);
  const [locationContext, setLocationContext] = useState<{ location: LocationRecord; x: number; y: number } | null>(null);
  const [pendingClose, setPendingClose] = useState<{ paneId: string; tabId: string } | null>(null);
  const locationsRef = useRef(locations);
  const filesRef = useRef(filesByLocation);
  const panesRef = useRef(panes);
  const refreshTimer = useRef<number | undefined>(undefined);
  const policyRefreshRequested = useRef(false);
  const okfIndexSignatures = useRef<Record<string, string>>({});
  const quickOpenResultRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const sidebarPanelRefs = useRef<Record<SidebarSectionId, HTMLElement | null>>({
    locations: null,
    files: null,
    history: null,
  });
  const documentViewStates = useRef(new Map<string, Partial<Record<TabMode, DocumentViewState>>>());
  const documentModeTransfers = useRef(new Map<string, DocumentModeTransfer>());
  locationsRef.current = locations;
  filesRef.current = filesByLocation;
  panesRef.current = panes;

  const notify = useCallback((message: string) => { setNotice(message); window.setTimeout(() => setNotice((current) => current === message ? null : current), 4200); }, []);

  const launchTerminal = useCallback(async (
    application: TerminalApplication,
    target: TerminalTarget,
  ) => {
    try {
      const result = await api.openTerminal({
        locationId: target.locationId,
        relativeDirectory: target.relativeDirectory,
        terminalApplicationId: application.id,
      });
      const location = locationsRef.current.find((item) => item.id === target.locationId);
      const directory = target.relativeDirectory
        ? `${location?.name || "Location"}/${target.relativeDirectory}`
        : location?.name || "the Location";
      notify(`Opened ${result.application.label} at ${directory}.`);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
      void api.listTerminalApplications().then(setTerminalApplications).catch(() => undefined);
    }
  }, [notify]);

  const requestTerminal = useCallback((target: TerminalTarget) => {
    if (!terminalApplications.length) {
      notify("No supported terminal application was found.");
      return;
    }
    const selected = selectedTerminal(terminalApplications, terminalApplicationId);
    if (selected) {
      void launchTerminal(selected, target);
      return;
    }
    if (terminalApplications.length === 1) {
      const [onlyApplication] = terminalApplications;
      setTerminalApplicationId(onlyApplication.id);
      void launchTerminal(onlyApplication, target);
      return;
    }
    setTerminalPicker({ target });
  }, [launchTerminal, notify, terminalApplicationId, terminalApplications]);

  const chooseTerminal = useCallback((application: TerminalApplication) => {
    const target = terminalPicker?.target;
    setTerminalApplicationId(application.id);
    setTerminalPicker(null);
    if (target) void launchTerminal(application, target);
  }, [launchTerminal, terminalPicker]);

  const openTerminalSettings = useCallback(() => {
    if (!terminalApplications.length) {
      notify("No supported terminal application was found.");
      return;
    }
    setTerminalPicker({ target: null });
  }, [notify, terminalApplications]);

  const addHistory = useCallback((events: HistoryEvent[]) => {
    if (!events.length) return;
    const cutoff = Date.now() - 30 * 86_400_000;
    setHistory((previous) => deduplicateHistory([...events, ...previous].filter((event) => event.observedAt >= cutoff)).slice(0, 5000));
  }, []);

  const reconcile = useCallback((location: LocationRecord, entries: FileEntry[], source: HistoryEvent["source"]) => {
    setFingerprints((previous) => {
      const before = previous[location.id] || [];
      const after = entries.map(({ path, relativePath, modifiedAtMs, size }) => ({ path, relativePath, modifiedAtMs, size }));
      const beforeByPath = new Map(before.map((file) => [file.path, file]));
      const afterByPath = new Map(after.map((file) => [file.path, file]));
      const added = after.filter((file) => !beforeByPath.has(file.path));
      const removed = before.filter((file) => !afterByPath.has(file.path));
      const changed = after.filter((file) => {
        const old = beforeByPath.get(file.path);
        return old && (old.modifiedAtMs !== file.modifiedAtMs || old.size !== file.size);
      });
      if (!Object.prototype.hasOwnProperty.call(previous, location.id)) {
        return { ...previous, [location.id]: after };
      }
      const now = Date.now();
      const events: HistoryEvent[] = [];
      const unmatchedAdded = [...added];
      for (const old of removed) {
        const index = unmatchedAdded.findIndex((file) => file.size === old.size && Math.abs(file.modifiedAtMs - old.modifiedAtMs) < 2500);
        if (index >= 0) {
          const renamed = unmatchedAdded.splice(index, 1)[0];
          events.push({ id: crypto.randomUUID(), locationId: location.id, path: renamed.path, previousPath: old.path, relativePath: renamed.relativePath, kind: "renamed", observedAt: now, source, available: true });
        } else {
          events.push({ id: crypto.randomUUID(), locationId: location.id, path: old.path, relativePath: old.relativePath, kind: "removed", observedAt: now, source, available: false });
        }
      }
      for (const file of unmatchedAdded) events.push({ id: crypto.randomUUID(), locationId: location.id, path: file.path, relativePath: file.relativePath, kind: "created", observedAt: now, source, available: true });
      for (const file of changed) events.push({ id: crypto.randomUUID(), locationId: location.id, path: file.path, relativePath: file.relativePath, kind: "modified", observedAt: now, source, available: true });
      addHistory(events);
      return { ...previous, [location.id]: after };
    });
  }, [addHistory]);

  const refreshOkfIndex = useCallback(async (location: LocationRecord, entries: FileEntry[], force = false) => {
    if (location.okfMode === "disabled" || (location.okfMode === "manual" && !location.okfBundle)) {
      setOkfIndexes((current) => { const next = { ...current }; delete next[location.id]; return next; });
      return false;
    }
    const signature = entries.map((entry) => `${entry.path}:${entry.modifiedAtMs}:${entry.size}`).join("|");
    if (!force && okfIndexSignatures.current[location.id] === signature) return Boolean(location.okfBundle);
    okfIndexSignatures.current[location.id] = signature;
    if (!force) {
      setOkfIndexes((current) => ({ ...current, [location.id]: { status: "scanning", concepts: current[location.id]?.concepts || [], signature } }));
    }
    try {
      const snapshot = await api.inspectOkfBundle(location.path);
      const enabled = location.okfMode === "manual" ? Boolean(location.okfBundle) : snapshot.detected;
      if (location.okfMode !== "manual") {
        setLocations((current) => current.map((item) => item.id === location.id
          ? { ...item, okfBundle: enabled, okfMode: "auto" }
          : item));
      }
      if (enabled) {
        setOkfIndexes((current) => ({ ...current, [location.id]: {
          status: "ready",
          concepts: snapshot.concepts,
          signature,
          declaredVersion: snapshot.declaredVersion,
          documentCount: snapshot.documentCount,
          findingCount: snapshot.findingCount,
          findings: snapshot.findings,
          ignoredPaths: snapshot.ignoredPaths,
        } }));
      } else {
        setOkfIndexes((current) => { const next = { ...current }; delete next[location.id]; return next; });
      }
      return enabled;
    } catch (error) {
      setOkfIndexes((current) => ({ ...current, [location.id]: { status: "error", concepts: [], signature, error: error instanceof Error ? error.message : String(error) } }));
      if (force) throw error;
      return Boolean(location.okfBundle);
    }
  }, []);

  const refreshLocation = useCallback(async (
    location: LocationRecord,
    source: HistoryEvent["source"] = "external",
    forceOkf = false,
  ) => {
    try {
      const entries = await api.listMarkdownFiles(location.path);
      setFilesByLocation((current) => ({ ...current, [location.id]: entries }));
      reconcile(location, entries, source);
      const okfBundle = await refreshOkfIndex(location, entries, forceOkf);
      try {
        const indexStatus = await api.syncLocationIndex({
          locationId: location.id,
          rootPath: location.path,
          displayName: location.name,
          okfBundle,
        });
        setIndexStatuses((current) => ({ ...current, [location.id]: indexStatus }));
      } catch {
        void api.getLocationIndexStatus(location.id)
          .then((indexStatus) => setIndexStatuses((current) => ({ ...current, [location.id]: indexStatus })))
          .catch(() => undefined);
      }
      const entriesByPath = new Map(entries.map((entry) => [pathIdentity(entry.path), entry]));
      const candidates = Object.values(panesRef.current).flatMap((pane) => pane.tabs.filter((tab) => tab.locationId === location.id).map((tab) => ({ paneId: pane.id, tab })));
      setPanes((current) => Object.fromEntries(Object.entries(current).map(([paneId, pane]) => [paneId, {
        ...pane,
        tabs: pane.tabs.flatMap((tab) => {
          if (tab.locationId !== location.id) return [tab];
          const entry = entriesByPath.get(pathIdentity(tab.path));
          if (entry) return [{ ...tab, path: entry.path, relativePath: entry.relativePath }];
          return tab.dirty ? [{ ...tab, deleted: true }] : [];
        }),
        activeTabId: pane.tabs.some((tab) => tab.id === pane.activeTabId && (tab.locationId !== location.id || entriesByPath.has(pathIdentity(tab.path)) || tab.dirty)) ? pane.activeTabId : pane.tabs.find((tab) => tab.locationId !== location.id || entriesByPath.has(pathIdentity(tab.path)) || tab.dirty)?.id || null,
      }])) as Record<string, Pane>);
      for (const { paneId, tab } of candidates) {
        const entry = entriesByPath.get(pathIdentity(tab.path));
        if (!entry || entry.modifiedAtMs === tab.diskModifiedAtMs) continue;
        void api.readMarkdownFile(entry.path).then((contents) => setPanes((current) => ({ ...current, [paneId]: {
          ...current[paneId], tabs: current[paneId].tabs.map((currentTab) => currentTab.id !== tab.id ? currentTab : currentTab.dirty
            ? { ...currentTab, conflict: true, diskModifiedAtMs: contents.modifiedAtMs }
            : { ...currentTab, path: entry.path, relativePath: entry.relativePath, content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, conflict: false, deleted: false }),
        } })));
      }
      setLocations((current) => current.map((item) => item.id === location.id ? { ...item, available: true } : item));
    } catch {
      setLocations((current) => current.map((item) => item.id === location.id ? { ...item, available: false } : item));
      setFilesByLocation((current) => ({ ...current, [location.id]: [] }));
      void api.getLocationIndexStatus(location.id)
        .then((indexStatus) => setIndexStatuses((current) => ({ ...current, [location.id]: indexStatus })))
        .catch(() => undefined);
    }
  }, [reconcile, refreshOkfIndex]);

  const refreshAll = useCallback((
    source: HistoryEvent["source"] = "external",
    forceOkf = false,
  ) => Promise.all(
    locationsRef.current.map((location) => refreshLocation(location, source, forceOkf)),
  ), [refreshLocation]);

  const configureLocations = useCallback(async (next: LocationRecord[]) => {
    try {
      const watched = await api.setWatchedLocations(next.map(({ id, path }) => ({ id, path })));
      setLocations((current) => current.map((location) => ({
        ...location,
        available: watched.includes(location.id),
      })));
      return watched;
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
      return [];
    }
  }, [notify]);

  const rebuildLocationIndex = useCallback(async (location: LocationRecord) => {
    if (!location.available) return notify("This Location is not available.");
    if (!window.confirm(`Rebuild the local index for “${location.name}”? No project files will be changed.`)) return;
    setIndexStatuses((current) => ({
      ...current,
      [location.id]: {
        ...(current[location.id] || {
          locationId: location.id,
          activeGeneration: null,
          discoveredDocuments: 0,
          indexedDocuments: 0,
          failedDocuments: 0,
          changedDocuments: 0,
          removedDocuments: 0,
          complete: false,
          lastReconciledAt: null,
          storageBytes: 0,
          error: null,
        }),
        state: "indexing",
        buildingGeneration: (current[location.id]?.activeGeneration || 0) + 1,
      },
    }));
    try {
      const status = await api.syncLocationIndex({
        locationId: location.id,
        rootPath: location.path,
        displayName: location.name,
        okfBundle: Boolean(location.okfBundle),
        rebuild: true,
      });
      setIndexStatuses((current) => ({ ...current, [location.id]: status }));
      notify(`Rebuilt the local index for “${location.name}”.`);
    } catch (error) {
      const status = await api.getLocationIndexStatus(location.id).catch(() => null);
      if (status) setIndexStatuses((current) => ({ ...current, [location.id]: status }));
      notify(error instanceof Error ? error.message : String(error));
    }
  }, [notify]);

  const setPane = useCallback((paneId: string, updater: (pane: Pane) => Pane) => {
    setPanes((current) => ({ ...current, [paneId]: updater(current[paneId]) }));
  }, []);

  const findTab = useCallback((paneId = activePaneId) => {
    const pane = panes[paneId];
    return pane?.tabs.find((tab) => tab.id === pane.activeTabId) || null;
  }, [activePaneId, panes]);

  const openFile = useCallback(async (file: FileEntry, newPane = false, initialMode?: TabMode) => {
    setExploreLocationId(null);
    setSearchSession(null);
    let targetId = activePaneId;
    if (newPane) {
      const nextId = crypto.randomUUID();
      setPanes((current) => ({ ...current, [nextId]: emptyPane(nextId) }));
      setLayout((current) => updateLayoutPane(current, activePaneId, { type: "split", direction: "horizontal", ratio: 0.5, first: { type: "pane", paneId: activePaneId }, second: { type: "pane", paneId: nextId } }));
      targetId = nextId;
      setActivePaneId(nextId);
    }
    const pane = panes[targetId];
    const existing = pane?.tabs.find((tab) => pathsEqual(tab.path, file.path));
    if (existing) {
      setPane(targetId, (current) => ({
        ...current,
        activeTabId: existing.id,
        tabs: current.tabs.map((tab) => tab.id === existing.id && initialMode ? { ...tab, mode: initialMode } : tab),
      }));
      return;
    }
    const location = locationsRef.current.find((item) => pathBelongsToLocation(file.path, item.path));
    if (!location) return notify("This file does not belong to an available location.");
    try {
      const [contents, git] = await Promise.all([api.readMarkdownFile(file.path), api.getGitInfo(file.path)]);
      const tab: DocumentTab = { id: crypto.randomUUID(), path: file.path, locationId: location.id, title: file.name, relativePath: file.relativePath, mode: initialMode || "preview", content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, dirty: false, conflict: false, deleted: false, git };
      setPane(targetId, (current) => ({ ...current, tabs: [...current.tabs, tab], activeTabId: tab.id }));
    } catch (error) { notify(error instanceof Error ? error.message : String(error)); }
  }, [activePaneId, notify, panes, setPane]);

  const openPath = useCallback((path: string) => {
    const file = Object.values(filesRef.current).flat().find((item) => pathsEqual(item.path, path));
    if (file) void openFile(file);
    else notify("The linked file does not exist in an available Location.");
  }, [notify, openFile]);

  const openExploreConcept = useCallback((path: string) => {
    const file = Object.values(filesRef.current).flat().find((item) => pathsEqual(item.path, path));
    if (!file) return notify("The selected concept is no longer available.");
    setExploreLocationId(null);
    setSearchSession(null);
    void openFile(file);
  }, [notify, openFile]);

  const openExploreFinding = useCallback((relativePath: string) => {
    if (!exploreLocationId) return;
    const file = (filesRef.current[exploreLocationId] || [])
      .find((entry) => entry.relativePath.replace(/\\/g, "/") === relativePath.replace(/\\/g, "/"));
    if (!file) return notify("The document referenced by this finding is no longer available.");
    setExploreLocationId(null);
    setSearchSession(null);
    void openFile(file, false, "source");
  }, [exploreLocationId, notify, openFile]);

  const refreshExploreHealth = useCallback(async () => {
    const location = locationsRef.current.find((item) => item.id === exploreLocationId);
    if (!location) throw new Error("This Location is no longer available.");
    const entries = await api.listMarkdownFiles(location.path);
    setFilesByLocation((current) => ({ ...current, [location.id]: entries }));
    await refreshOkfIndex(location, entries, true);
    notify(`Linted “${location.name}”. No project files were changed.`);
  }, [exploreLocationId, notify, refreshOkfIndex]);

  const openKnowledgeSearch = useCallback(() => {
    setExploreLocationId(null);
    setSearchSession((current) => current
      ? { ...current, focusSignal: current.focusSignal + 1 }
      : { initialLocationId: selectedLocationId, focusSignal: 1 });
  }, [selectedLocationId]);

  const openKnowledgeResult = useCallback((result: Pick<KnowledgeSearchResult, "locationId" | "relativePath">, newPane = false) => {
    const file = (filesRef.current[result.locationId] || [])
      .find((entry) => entry.relativePath.replace(/\\/g, "/") === result.relativePath);
    if (!file) return notify("The selected document is no longer available.");
    void openFile(file, newPane);
  }, [notify, openFile]);

  const rememberKnowledgeSearch = useCallback((
    searchQuery: string,
    locationIds: string[],
    filters: KnowledgeSearchFilters,
  ) => {
    setRecentSearches((current) => rememberSearch(current, {
      query: searchQuery,
      locationIds,
      filters,
    }));
  }, []);

  const updateTab = useCallback((paneId: string, tabId: string, updater: (tab: DocumentTab) => DocumentTab) => {
    setPane(paneId, (pane) => ({ ...pane, tabs: pane.tabs.map((tab) => tab.id === tabId ? updater(tab) : tab) }));
  }, [setPane]);

  const rememberDocumentViewState = useCallback((tabId: string, mode: TabMode, state: DocumentViewState) => {
    const current = documentViewStates.current.get(tabId) || {};
    documentViewStates.current.set(tabId, { ...current, [mode]: state });
  }, []);

  const consumeDocumentRestoreState = useCallback((tabId: string, mode: TabMode) => {
    const transfer = documentModeTransfers.current.get(tabId);
    if (transfer?.targetMode === mode) documentModeTransfers.current.delete(tabId);
    return {
      saved: documentViewStates.current.get(tabId)?.[mode] || null,
      transfer: transfer?.targetMode === mode ? transfer : null,
    };
  }, []);

  const changeTabMode = useCallback((paneId: string, tab: DocumentTab, mode: TabMode) => {
    if (tab.mode === mode) return;
    const current = documentViewStates.current.get(tab.id)?.[tab.mode];
    documentModeTransfers.current.set(tab.id, {
      targetMode: mode,
      anchor: current?.anchor || null,
      ratio: current?.ratio || 0,
    });
    updateTab(paneId, tab.id, (item) => ({ ...item, mode }));
  }, [updateTab]);

  const reloadTab = useCallback(async (paneId: string, tabId: string) => {
    const tab = panes[paneId]?.tabs.find((item) => item.id === tabId);
    if (!tab) return;
    if (tab.dirty && !window.confirm("Discard unsaved changes and reload this file from disk?")) return;
    try {
      const [contents, git] = await Promise.all([api.readMarkdownFile(tab.path), api.getGitInfo(tab.path)]);
      updateTab(paneId, tab.id, (current) => ({ ...current, content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, dirty: false, conflict: false, deleted: false, git }));
    } catch (error) { notify(error instanceof Error ? error.message : String(error)); }
  }, [notify, panes, updateTab]);

  const saveTab = useCallback(async (paneId = activePaneId, tabId?: string) => {
    const pane = panes[paneId];
    const tab = pane?.tabs.find((item) => item.id === (tabId || pane.activeTabId));
    if (!tab || !tab.dirty || tab.deleted) return false;
    if (tab.conflict && !window.confirm("This file changed outside the app. Saving will overwrite the external version. Continue?")) return false;
    try {
      await api.writeMarkdownFile(tab.path, normalizeForSave(tab.content, tab.lineEnding));
      const [verified, git] = await Promise.all([api.readMarkdownFile(tab.path), api.getGitInfo(tab.path)]);
      updateTab(paneId, tab.id, (current) => ({ ...current, content: verified.content, baseContent: verified.content, lineEnding: verified.lineEnding, diskModifiedAtMs: verified.modifiedAtMs, dirty: false, conflict: false, git }));
      const location = locationsRef.current.find((item) => item.id === tab.locationId);
      if (location) void refreshLocation(location, "app");
      return true;
    } catch (error) { notify(error instanceof Error ? error.message : String(error)); return false; }
  }, [activePaneId, notify, panes, refreshLocation, updateTab]);

  const discardTab = useCallback((paneId: string, tabId: string) => {
    const pane = panes[paneId];
    if (!pane) return;
    setPane(paneId, (current) => {
      const tabs = current.tabs.filter((item) => item.id !== tabId);
      return { ...current, tabs, activeTabId: current.activeTabId === tabId ? tabs.at(-1)?.id || null : current.activeTabId };
    });
    documentViewStates.current.delete(tabId);
    documentModeTransfers.current.delete(tabId);
  }, [panes, setPane]);

  const closeTab = useCallback((paneId: string, tabId: string) => {
    const tab = panes[paneId]?.tabs.find((item) => item.id === tabId);
    if (!tab) return;
    if (tab.dirty) { setPendingClose({ paneId, tabId }); return; }
    discardTab(paneId, tabId);
  }, [discardTab, panes]);

  const splitPane = useCallback((direction: "horizontal" | "vertical") => {
    const nextId = crypto.randomUUID();
    setPanes((current) => ({ ...current, [nextId]: emptyPane(nextId) }));
    setLayout((current) => updateLayoutPane(current, activePaneId, { type: "split", direction, ratio: 0.5, first: { type: "pane", paneId: activePaneId }, second: { type: "pane", paneId: nextId } }));
    setActivePaneId(nextId);
  }, [activePaneId]);

  const closePane = useCallback((paneId: string) => {
    const ids = getPaneIds(layout);
    if (ids.length === 1) return;
    const pane = panes[paneId];
    if (!pane) return;
    const target = ids.find((id) => id !== paneId)!;
    if (pane.tabs.some((tab) => tab.dirty) && !window.confirm("This pane has unsaved changes. Closing it will move its tabs to the other pane. Continue?")) return;
    setPanes((current) => {
      const { [paneId]: removed, ...rest } = current;
      return { ...rest, [target]: { ...rest[target], tabs: [...rest[target].tabs, ...removed.tabs], activeTabId: rest[target].activeTabId || removed.activeTabId } };
    });
    setLayout((current) => removeLayoutPane(current, paneId) || defaultLayout);
    setActivePaneId(target);
  }, [layout, panes]);

  const addLocation = useCallback(async () => {
    const selected = await open({ directory: true, multiple: false, title: "Add folder to watch" });
    if (typeof selected !== "string") return;
    if (locationsRef.current.some((location) => pathsEqual(location.path, selected))) return notify("This folder has already been added.");
    const location: LocationRecord = { id: crypto.randomUUID(), path: selected, name: basename(selected), available: true };
    const next = [...locationsRef.current, location];
    setLocations(next);
    await configureLocations(next);
    setSelectedLocationId(location.id);
    await refreshLocation(location, "reconciliation");
  }, [configureLocations, notify, refreshLocation]);

  const removeLocation = useCallback(async (locationId: string) => {
    const location = locationsRef.current.find((item) => item.id === locationId);
    if (!location || !window.confirm(`Remove “${location.name}” from Construct? Its files will not be deleted, and its derived local index will be removed.`)) return;
    try {
      await api.deleteLocationIndex(locationId);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
      return;
    }
    const next = locationsRef.current.filter((item) => item.id !== locationId);
    setLocations(next);
    setSelectedLocationId((current) => current === locationId ? next[0]?.id || null : current);
    setFilesByLocation((current) => { const nextFiles = { ...current }; delete nextFiles[locationId]; return nextFiles; });
    setIndexStatuses((current) => { const nextStatuses = { ...current }; delete nextStatuses[locationId]; return nextStatuses; });
    await configureLocations(next);
  }, [configureLocations, notify]);

  useEffect(() => {
    let mounted = true;
    let workspaceRevealed = false;
    (async () => {
      try {
        const saved = await api.loadState();
        if (!mounted) return;
        const restoredLocations = saved.locations || [];
        const restoredPanes = saved.panes?.length ? saved.panes : [{ ...defaultPane, tabs: [] } as SavedPane];
        const restoredLayout = saved.layout || defaultLayout;
        setLocations(restoredLocations);
        setHistory(deduplicateHistory((saved.history || []).filter((event) => event.observedAt >= Date.now() - 30 * 86_400_000)).slice(0, 5000));
        setFingerprints(saved.fingerprints || {});
        setSelectedLocationId(saved.selectedLocationId || restoredLocations[0]?.id || null);
        setSidebarWidth(saved.sidebarWidth || 295);
        setSidebarHidden(saved.sidebarHidden || false);
        setCollapsedSections(saved.collapsedSections || {});
        setSidebarPanelSizes(sanitizeSidebarPanelSizes(saved.sidebarPanelSizes));
        setTheme(saved.theme || "dark");
        setTerminalApplicationId(saved.terminalApplicationId);
        void api.listTerminalApplications().then((availableTerminals) => {
          if (!mounted) return;
          setTerminalApplications(availableTerminals);
          setTerminalApplicationId(
            selectedTerminal(availableTerminals, saved.terminalApplicationId)?.id,
          );
        }).catch(() => undefined);
        setRememberRecentSearches(saved.rememberRecentSearches !== false);
        setRecentSearches((saved.recentSearches || []).slice(0, 20));
        setLayout(restoredLayout);
        setActivePaneId(saved.activePaneId || "main");
        await configureLocations(restoredLocations);
        const hydrated: Record<string, Pane> = {};
        for (const savedPane of restoredPanes) {
          const tabs: DocumentTab[] = [];
          for (const savedTab of savedPane.tabs || []) {
            try {
              const [contents, git] = await Promise.all([api.readMarkdownFile(savedTab.path), api.getGitInfo(savedTab.path)]);
              tabs.push({ ...savedTab, content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, dirty: false, conflict: false, deleted: false, git });
            } catch { /* Missing files remain excluded from workspace restoration. */ }
          }
          hydrated[savedPane.id] = { id: savedPane.id, tabs, activeTabId: tabs.some((tab) => tab.id === savedPane.activeTabId) ? savedPane.activeTabId : tabs.at(-1)?.id || null };
        }
        if (!mounted) return;
        if (!Object.keys(hydrated).length) hydrated.main = defaultPane;
        setPanes(hydrated);
        setReady(true);
        workspaceRevealed = true;
        void Promise.allSettled(
          restoredLocations.map((location) => refreshLocation(location, "reconciliation")),
        );
      } catch (error) { notify(error instanceof Error ? error.message : String(error)); }
      finally {
        if (mounted && !workspaceRevealed) setReady(true);
      }
    })();
    return () => { mounted = false; };
  }, [configureLocations, notify, refreshLocation]);

  useEffect(() => {
    const unlisten = listen<FileSystemChange>("filesystem-change", (event) => {
      const policyChanged = event.payload.paths.some((path) => (
        path.replace(/\\/g, "/").split("/").at(-1) === ".constructignore"
      ));
      policyRefreshRequested.current ||= policyChanged;
      window.clearTimeout(refreshTimer.current);
      refreshTimer.current = window.setTimeout(() => {
        const forceOkf = policyRefreshRequested.current;
        policyRefreshRequested.current = false;
        void refreshAll("external", forceOkf);
      }, 450);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [refreshAll]);

  useEffect(() => {
    if (!ready) return;
    const handle = window.setTimeout(() => {
      const savedPanes: SavedPane[] = Object.values(panes).map((pane) => ({ ...pane, tabs: pane.tabs.map(({ id, path, locationId, title, relativePath, mode }) => ({ id, path, locationId, title, relativePath, mode })) }));
      const state: SavedWorkspace = {
        locations,
        history,
        fingerprints,
        panes: savedPanes,
        layout,
        activePaneId,
        selectedLocationId,
        sidebarWidth,
        sidebarHidden,
        collapsedSections,
        sidebarPanelSizes,
        theme,
        terminalApplicationId,
        rememberRecentSearches,
        recentSearches: rememberRecentSearches ? recentSearches.slice(0, 20) : [],
      };
      void api.saveState(state).catch(() => undefined);
    }, 450);
    return () => window.clearTimeout(handle);
  }, [activePaneId, collapsedSections, fingerprints, history, layout, locations, panes, ready, recentSearches, rememberRecentSearches, selectedLocationId, sidebarHidden, sidebarPanelSizes, sidebarWidth, terminalApplicationId, theme]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.metaKey) return;
      if (event.shiftKey && event.key.toLowerCase() === "f") { event.preventDefault(); openKnowledgeSearch(); }
      if (event.key.toLowerCase() === "p") {
        event.preventDefault();
        setQuickOpenSelection(0);
        setQuickOpen(true);
      }
      if (event.key.toLowerCase() === "s") { event.preventDefault(); void saveTab(); }
      if (event.key.toLowerCase() === "w") {
        event.preventDefault();
        event.stopPropagation();
        const tab = findTab();
        if (tab) closeTab(activePaneId, tab.id);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activePaneId, closeTab, findTab, openKnowledgeSearch, saveTab]);

  const inspectableOkfTabs = useMemo(() => {
    const locationsById = new Map(locations.filter((location) => location.okfBundle).map((location) => [location.id, location]));
    return Object.values(panes).flatMap((pane) => {
      const tab = pane.tabs.find((item) => item.id === pane.activeTabId);
      if (!tab) return [];
      const location = locationsById.get(tab.locationId);
      return location ? [{ tab, location }] : [];
    });
  }, [locations, panes]);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void Promise.all(inspectableOkfTabs.map(async ({ tab, location }) => {
        try {
          const inspection = await api.inspectOkfDocument({
            content: tab.content,
            relativePath: tab.relativePath,
            sourcePath: tab.path,
            bundleRoot: location.path,
            isBundleRoot: tab.relativePath.toLowerCase() === "index.md",
          });
          return [tab.id, { content: tab.content, inspection }] as const;
        } catch {
          return null;
        }
      })).then((results) => {
        if (cancelled) return;
        setOkfInspections(Object.fromEntries(results.filter((result): result is NonNullable<typeof result> => result !== null)));
      });
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [inspectableOkfTabs]);

  const activeLocation = locations.find((location) => location.id === selectedLocationId) || null;
  const exploreLocation = locations.find((location) => location.id === exploreLocationId) || null;
  const activeTerminal = selectedTerminal(terminalApplications, terminalApplicationId);
  const fileResults = useMemo(() => Object.entries(filesByLocation).flatMap(([locationId, files]) => files.map((file) => ({ ...file, locationId }))).filter((file) => `${file.name} ${file.relativePath}`.toLowerCase().includes(query.toLowerCase())).slice(0, 100), [filesByLocation, query]);
  const activeQuickOpenIndex = fileResults.length
    ? Math.min(quickOpenSelection, fileResults.length - 1)
    : 0;
  useEffect(() => {
    if (!quickOpen || !fileResults.length) return;
    quickOpenResultRefs.current[activeQuickOpenIndex]?.scrollIntoView({ block: "nearest" });
  }, [activeQuickOpenIndex, fileResults.length, quickOpen]);
  const visibleHistory = history.filter((event) => historyFilter === "all" || event.kind === historyFilter);
  const expandedSidebarSections = sidebarSectionIds.filter(
    (section) => !collapsedSections[section],
  );
  const previousExpandedSidebarSection = (section: SidebarSectionId) => {
    const index = expandedSidebarSections.indexOf(section);
    return index > 0 ? expandedSidebarSections[index - 1] : null;
  };

  const openMcpDialog = () => {
    setMcpDialog({
      mode: activeLocation ? "current" : "custom",
      locationIds: activeLocation ? [activeLocation.id] : [],
    });
  };

  const copyMcpConfiguration = async () => {
    if (!mcpDialog) return;
    const locationIds = mcpDialog.mode === "all"
      ? []
      : mcpDialog.mode === "current"
        ? activeLocation ? [activeLocation.id] : []
        : mcpDialog.locationIds;
    if (mcpDialog.mode !== "all" && !locationIds.length) {
      notify("Choose at least one Location for agent access.");
      return;
    }
    try {
      const configuration = await api.getMcpConfiguration({
        locationIds,
        allowAll: mcpDialog.mode === "all",
      });
      await navigator.clipboard.writeText(configuration);
      setMcpDialog(null);
      notify(mcpDialog.mode === "all"
        ? "MCP configuration copied for all registered Locations."
        : `MCP configuration copied for ${locationIds.length} Location${locationIds.length === 1 ? "" : "s"}.`);
    } catch (cause) {
      notify(`Could not copy the MCP configuration: ${cause instanceof Error ? cause.message : String(cause)}`);
    }
  };

  const resizeSidebar = (event: React.PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const onMove = (move: PointerEvent) => setSidebarWidth(Math.max(220, Math.min(520, move.clientX)));
    const onUp = () => { window.removeEventListener("pointermove", onMove); window.removeEventListener("pointerup", onUp); };
    window.addEventListener("pointermove", onMove); window.addEventListener("pointerup", onUp);
  };

  const resizeSidebarPanels = (
    event: React.PointerEvent<HTMLDivElement>,
    upper: SidebarSectionId,
    lower: SidebarSectionId,
  ) => {
    const upperPanel = sidebarPanelRefs.current[upper];
    const lowerPanel = sidebarPanelRefs.current[lower];
    if (!upperPanel || !lowerPanel) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const start = event.clientY;
    const upperHeight = upperPanel.getBoundingClientRect().height;
    const lowerHeight = lowerPanel.getBoundingClientRect().height;
    const initial = sidebarPanelSizes;
    const onMove = (move: PointerEvent) => {
      setSidebarPanelSizes(resizeSidebarPanelPair(
        initial,
        upper,
        lower,
        upperHeight,
        lowerHeight,
        move.clientY - start,
      ));
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const renderPane = (pane: Pane, active: boolean) => {
    const tab = pane.tabs.find((item) => item.id === pane.activeTabId) || null;
    const tabLocation = tab ? locationsRef.current.find((location) => location.id === tab.locationId) : null;
    const cachedInspection = tab ? okfInspections[tab.id] : undefined;
    const bufferInspection = tab && tabLocation?.okfBundle && cachedInspection?.content === tab.content ? cachedInspection.inspection : null;
    const review = tab ? splitReviewDocument(tab.content) : null;
    const bundleIndex = tabLocation ? okfIndexes[tabLocation.id] : undefined;
    const savedFindings = tab && !tab.dirty && bundleIndex?.status === "ready"
      ? (bundleIndex.findings || []).filter((finding) => finding.relativePath === tab.relativePath)
      : [];
    const okf = bufferInspection ? {
      ...bufferInspection,
      findings: [...bufferInspection.findings, ...savedFindings.filter((saved) => !bufferInspection.findings.some((item) => item.code === saved.code && item.message === saved.message))],
    } : null;
    const concept = tab && bundleIndex?.status === "ready" ? bundleIndex.concepts.find((item) => pathsEqual(item.path, tab.path)) : undefined;
    const outgoingConcepts = concept ? concept.outgoingPaths.map((path) => bundleIndex?.concepts.find((item) => pathsEqual(item.path, path))).filter((item): item is OkfConcept => Boolean(item)) : [];
    const incomingConcepts = concept ? concept.incomingPaths.map((path) => bundleIndex?.concepts.find((item) => pathsEqual(item.path, path))).filter((item): item is OkfConcept => Boolean(item)) : [];
    const changeContent = (content: string) => tab && updateTab(pane.id, tab.id, (current) => ({ ...current, content, dirty: content !== current.baseContent }));
    const reloadExternal = () => { if (tab) void reloadTab(pane.id, tab.id); };
    return <section className={`editor-pane ${active ? "active" : ""}`}>
      <div className="tab-bar" onDragOver={(event) => event.preventDefault()} onDrop={(event) => {
        const payload = event.dataTransfer.getData("application/construct-tab");
        if (!payload) return;
        const { paneId, tabId } = JSON.parse(payload) as { paneId: string; tabId: string };
        setPanes((current) => {
          const moving = current[paneId].tabs.find((item) => item.id === tabId);
          if (!moving) return current;
          const source = current[paneId]; const target = current[pane.id];
          if (paneId === pane.id) return { ...current, [pane.id]: { ...target, tabs: [...target.tabs.filter((item) => item.id !== tabId), moving], activeTabId: moving.id } };
          return { ...current, [paneId]: { ...source, tabs: source.tabs.filter((item) => item.id !== tabId), activeTabId: source.activeTabId === tabId ? source.tabs.find((item) => item.id !== tabId)?.id || null : source.activeTabId }, [pane.id]: { ...target, tabs: [...target.tabs, moving], activeTabId: moving.id } };
        });
      }}>
        <div className="tabs-scroll">{pane.tabs.map((item) => <div key={item.id} draggable className={`tab ${item.id === pane.activeTabId ? "selected" : ""}`} onDragStart={(event) => event.dataTransfer.setData("application/construct-tab", JSON.stringify({ paneId: pane.id, tabId: item.id }))} onClick={() => setPane(pane.id, (current) => ({ ...current, activeTabId: item.id }))} onContextMenu={(event) => { event.preventDefault(); setTabContext({ tab: item, paneId: pane.id, x: event.clientX, y: event.clientY }); }} title={item.relativePath}>
          <span className={item.dirty ? "dirty-dot" : "file-tab-icon"}>{item.dirty ? "●" : "#"}</span><span>{item.title}</span><button aria-label={`Fechar ${item.title}`} onClick={(event) => { event.stopPropagation(); closeTab(pane.id, item.id); }}>×</button>
        </div>)}</div>
        <button className="icon-button" title="Split vertically" onClick={() => { setActivePaneId(pane.id); splitPane("horizontal"); }}><Columns2 size={14} /></button>
        <button className="icon-button" title="Split horizontally" onClick={() => { setActivePaneId(pane.id); splitPane("vertical"); }}><Rows3 size={14} /></button>
        {getPaneIds(layout).length > 1 && <button className="icon-button" title="Close pane" onClick={() => closePane(pane.id)}><X size={14} /></button>}
      </div>
      {!tab ? <div className="empty-pane"><h2>Open a context file</h2><p>Choose a file from the sidebar or use <kbd>⌘P</kbd>.</p><button className="toolbar-button" onClick={openKnowledgeSearch}><SearchIcon size={13} /> Search knowledge</button>{activeLocation?.okfBundle && <button className="toolbar-button" onClick={() => { setSearchSession(null); setExploreFilters({ types: [] }); setExploreLocationId(activeLocation.id); }}>Explore this OKF bundle</button>}</div> : <>
        <div className="document-toolbar">
          <span className="document-path" title={tab.path}>{tab.relativePath}</span>
          {okf && <button className={`okf-status ${okf.isConformant ? "valid" : "invalid"}`} title="Open Knowledge Format details" onClick={() => setShowOkfInspector((current) => !current)}>OKF</button>}
          <div className="mode-switch">
            {(["preview", "edit", "review", "source", "diff"] as TabMode[]).map((mode) => (
              <button key={mode} className={tab.mode === mode ? "selected" : ""} disabled={mode === "diff" && !tab.git?.available} onClick={() => changeTabMode(pane.id, tab, mode)}>
                {modeLabels[mode]}{mode === "review" && review?.comments.length ? <span className="mode-count">{review.comments.length}</span> : null}
              </button>
            ))}
          </div>
          <button className="toolbar-button" disabled={!tab.dirty || tab.deleted} onClick={() => void saveTab(pane.id, tab.id)}>Save</button>
          <button className="icon-button" title={`Open ${activeTerminal?.label || "terminal"} here`} onClick={() => requestTerminal({ locationId: tab.locationId, relativeDirectory: relativeDirectoryForFile(tab.relativePath) })}><SquareTerminal size={14} /></button>
          <button className="icon-button" title="Reveal in Finder" onClick={() => void api.revealInFileManager(tab.path)}><Folder size={14} /></button>
        </div>
        {tab.conflict && <div className="conflict-banner"><span>This file changed outside the app.</span><button onClick={() => void reloadExternal()}>Reload external version</button><button onClick={() => updateTab(pane.id, tab.id, (current) => ({ ...current, conflict: false }))}>Keep my changes</button></div>}
        {tab.deleted && <div className="conflict-banner danger"><span>This file was removed outside the app.</span><button onClick={() => updateTab(pane.id, tab.id, (current) => ({ ...current, deleted: false }))}>Save again to this path</button></div>}
        {okf && showOkfInspector && <aside className="okf-inspector">
          <div className="okf-inspector-heading"><strong>Open Knowledge Format</strong><span className={okf.isConformant ? "okf-valid" : "okf-invalid"}>{okf.isConformant ? "Conformant" : "Needs attention"}</span></div>
          <div className="okf-kind">{okf.kind === "concept" ? "Concept document" : okf.kind === "index" ? "Directory index" : "Update log"}</div>
          <dl className="okf-metadata">
            {okf.metadata.type && <><dt>Type</dt><dd><button className="okf-metadata-link" onClick={() => { if (tabLocation) { setExploreFilters({ types: [okf.metadata.type!] }); setExploreLocationId(tabLocation.id); } }}>{okf.metadata.type}</button></dd></>}
            {okf.metadata.title && <><dt>Title</dt><dd>{okf.metadata.title}</dd></>}
            {okf.metadata.description && <><dt>Description</dt><dd>{okf.metadata.description}</dd></>}
            {okf.metadata.resource && <><dt>Resource</dt><dd>{/^https?:\/\//i.test(okf.metadata.resource) ? <a href={okf.metadata.resource} onClick={(event) => { event.preventDefault(); void api.openExternalUrl(okf.metadata.resource!); }}>{okf.metadata.resource}</a> : okf.metadata.resource}</dd></>}
            {okf.metadata.effectiveTimestamp && <><dt>Generated at</dt><dd>{okf.metadata.effectiveTimestamp}</dd></>}
            {okf.metadata.okfVersion && <><dt>OKF version</dt><dd>{okf.metadata.okfVersion}</dd></>}
            {okf.metadata.status && <><dt>Status</dt><dd>{okf.metadata.status}</dd></>}
            {okf.metadata.staleAfter && <><dt>Stale after</dt><dd>{okf.metadata.staleAfter}</dd></>}
            {okf.metadata.sources && <><dt>Sources</dt><dd title={formatOkfValue(okf.metadata.sources)}>{formatOkfValue(okf.metadata.sources)}</dd></>}
            {okf.metadata.generated && <><dt>Generated</dt><dd title={formatOkfValue(okf.metadata.generated)}>{formatOkfValue(okf.metadata.generated)}</dd></>}
            {okf.metadata.verified && <><dt>Verified</dt><dd title={formatOkfValue(okf.metadata.verified)}>{formatOkfValue(okf.metadata.verified)}</dd></>}
            {okf.metadata.extra.flatMap((entry) => [
              <dt key={`${entry.name}-term`}>{entry.name}</dt>,
              <dd key={`${entry.name}-value`} title={formatOkfValue(entry.value)}>{formatOkfValue(entry.value)}</dd>,
            ])}
          </dl>
          {!!okf.metadata.tags.length && <div className="okf-tags">{okf.metadata.tags.map((tag) => <button key={tag} onClick={() => { if (tabLocation) { setExploreFilters({ types: [], tag }); setExploreLocationId(tabLocation.id); } }}>#{tag}</button>)}</div>}
          {concept && <div className="okf-relations"><div><h3>Links to</h3>{outgoingConcepts.length ? outgoingConcepts.map((item) => <button key={item.path} onClick={() => openPath(item.path)}>{item.title}</button>) : <p>No links to concepts in this bundle.</p>}</div><div><h3>Referenced by</h3>{incomingConcepts.length ? incomingConcepts.map((item) => <button key={item.path} onClick={() => openPath(item.path)}>{item.title}</button>) : <p>No concepts reference this document.</p>}</div></div>}
          {!!okf.findings.length && <ul className="okf-issues">{okf.findings.map((item) => <li key={`${item.code}-${item.message}`} className={item.severity} title={item.code}>{item.message}</li>)}</ul>}
        </aside>}
        <div className="document-content">
          <DocumentModeSurface
            key={`${tab.id}:${tab.mode}`}
            tabId={tab.id}
            mode={tab.mode}
            consumeRestoreState={() => consumeDocumentRestoreState(tab.id, tab.mode)}
            onViewState={(state) => rememberDocumentViewState(tab.id, tab.mode, state)}
          >
            {tab.mode === "source" && <CodeEditor tabId={tab.id} value={tab.content} readOnly={tab.deleted} onChange={changeContent} onSave={() => void saveTab(pane.id, tab.id)} />}
            {tab.mode === "edit" && (
              <Suspense fallback={<div className="visual-editor-loading">Preparing visual editor…</div>}>
                <VisualEditor tabId={tab.id} value={tab.content} readOnly={tab.deleted} onChange={changeContent} onRequestSource={() => changeTabMode(pane.id, tab, "source")} />
              </Suspense>
            )}
            {tab.mode === "preview" && <MarkdownPreview content={tab.content} sourcePath={tab.path} bundleRoot={tabLocation?.okfBundle ? tabLocation.path : undefined} onOpenInternal={openPath} />}
            {tab.mode === "review" && <ReviewEditor content={tab.content} relativePath={tab.relativePath} sourcePath={tab.path} bundleRoot={tabLocation?.okfBundle ? tabLocation.path : undefined} readOnly={tab.deleted} onChange={changeContent} onOpenInternal={openPath} onRequestSource={() => changeTabMode(pane.id, tab, "source")} onNotify={notify} />}
            {tab.mode === "diff" && <DiffView tab={tab} />}
          </DocumentModeSurface>
        </div>
      </>}
    </section>;
  };

  if (!ready) return <main className="startup"><div className="startup-mark">✦</div><p>Preparing your workspace…</p></main>;

  return <main className={`app-shell ${sidebarHidden ? "sidebar-hidden" : ""}`} data-theme={theme} style={{ gridTemplateColumns: sidebarHidden ? "38px minmax(0, 1fr)" : `${sidebarWidth}px 5px minmax(0, 1fr)` }}>
    {sidebarHidden ? <aside className="sidebar-rail"><button className="sidebar-toggle" onClick={() => setSidebarHidden(false)} title="Show sidebar" aria-label="Show sidebar"><PanelLeftOpen size={16} /></button></aside> : <aside className="sidebar">
      <div className="sidebar-global-toolbar">
        <button className="sidebar-toggle" onClick={() => setSidebarHidden(true)} title="Hide sidebar" aria-label="Hide sidebar"><PanelLeftClose size={16} /></button>
        <span>CONSTRUCT</span>
        <button className="connect-agents-button" onClick={openMcpDialog} title="Connect agents"><Bot size={14} /><span>Agents</span></button>
        <button className="theme-button" onClick={() => setTheme((current) => current === "dark" ? "light" : "dark")} title={theme === "dark" ? "Use light theme" : "Use dark theme"} aria-label={theme === "dark" ? "Use light theme" : "Use dark theme"}>{theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}</button>
      </div>
      <div className="sidebar-panels">
        <section
          ref={(element) => { sidebarPanelRefs.current.locations = element; }}
          className={`sidebar-section locations-section ${collapsedSections.locations ? "collapsed" : ""}`}
          style={collapsedSections.locations ? undefined : { flexGrow: sidebarPanelSizes.locations }}
        >
          <div className="section-title"><button aria-expanded={!collapsedSections.locations} onClick={() => setCollapsedSections((current) => ({ ...current, locations: !current.locations }))}>{collapsedSections.locations ? <ChevronRight size={13} /> : <ChevronDown size={13} />}</button><MapPin size={13} /><span>LOCATIONS</span><button className="add-button" onClick={() => void addLocation()} title="Add folder"><CirclePlus size={15} /></button></div>
          {!collapsedSections.locations && <div className="sidebar-section-content"><div className="location-list">{locations.length ? locations.map((location) => <div key={location.id} draggable className={`location-row ${location.id === selectedLocationId ? "selected" : ""}`} onDragStart={(event) => event.dataTransfer.setData("application/construct-location", location.id)} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); const movedId = event.dataTransfer.getData("application/construct-location"); if (!movedId || movedId === location.id) return; setLocations((current) => { const moved = current.find((item) => item.id === movedId); if (!moved) return current; const remaining = current.filter((item) => item.id !== movedId); const index = remaining.findIndex((item) => item.id === location.id); remaining.splice(index, 0, moved); return remaining; }); }} onClick={() => setSelectedLocationId(location.id)} onContextMenu={(event) => { event.preventDefault(); setLocationContext({ location, x: event.clientX, y: event.clientY }); }} title={location.path}>
            <span className={`availability ${location.available ? "online" : "offline"}`} /><span className="location-name">{location.name}</span>{location.okfBundle && <span className="okf-toggle active" title="OKF bundle detected automatically">OKF</span>}<button className={`index-status ${indexStatuses[location.id]?.state || "notIndexed"}`} onClick={(event) => { event.stopPropagation(); void rebuildLocationIndex(location); }} title={indexStatusTitle(indexStatuses[location.id])} aria-label={`Rebuild index for ${location.name}`}><span /></button><button onClick={(event) => { event.stopPropagation(); const bounds = event.currentTarget.getBoundingClientRect(); setLocationContext({ location, x: bounds.right - 220, y: bounds.bottom }); }} title={`Actions for ${location.name}`} aria-label={`Actions for ${location.name}`}><MoreHorizontal size={14} /></button>
          </div>) : <div className="empty-sidebar">Add your project folders to get started.</div>}</div></div>}
        </section>
        {previousExpandedSidebarSection("files") && <div className="sidebar-panel-resizer" role="separator" aria-orientation="horizontal" aria-label="Resize Locations and Files" onPointerDown={(event) => resizeSidebarPanels(event, previousExpandedSidebarSection("files")!, "files")} />}
        <section
          ref={(element) => { sidebarPanelRefs.current.files = element; }}
          className={`sidebar-section files-section ${collapsedSections.files ? "collapsed" : ""}`}
          style={collapsedSections.files ? undefined : { flexGrow: sidebarPanelSizes.files }}
        >
          <div className="section-title"><button aria-expanded={!collapsedSections.files} onClick={() => setCollapsedSections((current) => ({ ...current, files: !current.files }))}>{collapsedSections.files ? <ChevronRight size={13} /> : <ChevronDown size={13} />}</button><Folder size={13} /><span>FILES</span>{activeLocation && <span className="section-subtitle" title={activeLocation.path}>{activeLocation.name}</span>}<button className="search-button" onClick={openKnowledgeSearch}><SearchIcon size={11} /> Search</button>{activeLocation?.okfBundle && <button className="explore-button" onClick={() => { setSearchSession(null); setExploreFilters({ types: [] }); setExploreLocationId(activeLocation.id); }}>Explore</button>}</div>
          {!collapsedSections.files && <div className="sidebar-section-content">{activeLocation ? <FileTree entries={filesByLocation[activeLocation.id] || []} onOpen={openFile} onContext={(event, file) => { event.preventDefault(); setFileContext({ file, locationId: activeLocation.id, x: event.clientX, y: event.clientY }); }} /> : <div className="empty-sidebar">Select a Location.</div>}</div>}
        </section>
        {previousExpandedSidebarSection("history") && <div className="sidebar-panel-resizer" role="separator" aria-orientation="horizontal" aria-label="Resize sidebar panels" onPointerDown={(event) => resizeSidebarPanels(event, previousExpandedSidebarSection("history")!, "history")} />}
        <section
          ref={(element) => { sidebarPanelRefs.current.history = element; }}
          className={`sidebar-section history-section ${collapsedSections.history ? "collapsed" : ""}`}
          style={collapsedSections.history ? undefined : { flexGrow: sidebarPanelSizes.history }}
        >
          <div className="section-title"><button aria-expanded={!collapsedSections.history} onClick={() => setCollapsedSections((current) => ({ ...current, history: !current.history }))}>{collapsedSections.history ? <ChevronRight size={13} /> : <ChevronDown size={13} />}</button><History size={13} /><span>HISTORY</span><select aria-label="Filter history" value={historyFilter} onChange={(event) => setHistoryFilter(event.target.value as HistoryKind | "all")}><option value="all">All</option><option value="created">New</option><option value="modified">Changed</option><option value="renamed">Renamed</option><option value="removed">Removed</option></select><button className="clear-history" title="Clear history" onClick={() => { if (window.confirm("Clear all local history? This will not alter any files.")) setHistory([]); }}><X size={13} /></button></div>
          {!collapsedSections.history && <div className="sidebar-section-content"><div className="history-list">{visibleHistory.length ? visibleHistory.map((event) => <button key={event.id} className="history-row" onClick={() => event.available ? openPath(event.path) : notify("This file is no longer available.")} title={event.previousPath ? `${event.previousPath} → ${event.path}` : event.path}>
            <span className={`history-kind ${event.kind}`}>{statusLabel(event.kind)}</span><span className="history-file">{basename(event.path)}</span><time>{formatWhen(event.observedAt)}</time>
          </button>) : <div className="empty-sidebar">Changes from your agents will appear here.</div>}</div></div>}
        </section>
      </div>
    </aside>}
    {!sidebarHidden && <div className="sidebar-resizer" onPointerDown={resizeSidebar} />}
    <section className="workspace">{searchSession ? <SearchWorkspace
      locations={locations}
      initialLocationId={searchSession.initialLocationId}
      indexStatuses={indexStatuses}
      recentSearches={recentSearches}
      rememberRecentSearches={rememberRecentSearches}
      focusSignal={searchSession.focusSignal}
      onRememberSearch={rememberKnowledgeSearch}
      onRecentSearches={setRecentSearches}
      onRememberRecentSearches={(remember) => { setRememberRecentSearches(remember); if (!remember) setRecentSearches([]); }}
      onOpen={openKnowledgeResult}
      onClose={() => setSearchSession(null)}
      onNotify={notify}
    /> : exploreLocation ? <BundleExplorer location={exploreLocation} index={okfIndexes[exploreLocation.id]} filters={exploreFilters} onFilters={setExploreFilters} onOpen={openExploreConcept} onOpenFinding={openExploreFinding} onRefreshHealth={refreshExploreHealth} onNotify={notify} onClose={() => setExploreLocationId(null)} /> : <SplitView node={layout} panes={panes} activePaneId={activePaneId} onActivate={setActivePaneId} onRatio={(node, ratio) => setLayout((current) => updateSplitRatio(current, node, ratio))}>{renderPane}</SplitView>}</section>
    {quickOpen && <div className="quick-open-backdrop" onMouseDown={() => setQuickOpen(false)}><div className="quick-open" onMouseDown={(event) => event.stopPropagation()}><input
      autoFocus
      placeholder="Open file…"
      value={query}
      onChange={(event) => {
        setQuery(event.target.value);
        setQuickOpenSelection(0);
      }}
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        if (event.key === "Escape") setQuickOpen(false);
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          setQuickOpenSelection((current) => moveQuickOpenSelection(
            current,
            fileResults.length,
            event.key === "ArrowDown" ? "next" : "previous",
          ));
        }
        const selectedFile = fileResults[activeQuickOpenIndex];
        if (event.key === "Enter" && selectedFile) {
          event.preventDefault();
          void openFile(selectedFile);
          setQuickOpen(false);
        }
      }}
    />
      <div className="quick-results">{fileResults.map((file, index) => <button
        key={file.path}
        ref={(element) => { quickOpenResultRefs.current[index] = element; }}
        className={index === activeQuickOpenIndex ? "active" : ""}
        aria-current={index === activeQuickOpenIndex ? "true" : undefined}
        onMouseEnter={() => setQuickOpenSelection(index)}
        onClick={() => { void openFile(file); setQuickOpen(false); }}
      ><span>{file.name}</span><small>{locations.find((location) => location.id === file.locationId)?.name} · {file.relativePath}</small></button>)}{!fileResults.length && <p>No files found.</p>}</div></div></div>}
    {locationContext && <div className="context-backdrop" onMouseDown={() => setLocationContext(null)}><div className="context-menu" style={{ left: locationContext.x, top: locationContext.y }} onMouseDown={(event) => event.stopPropagation()}>
      <button onClick={() => { requestTerminal({ locationId: locationContext.location.id, relativeDirectory: "" }); setLocationContext(null); }}><SquareTerminal size={13} /> Open terminal at Location</button>
      <button onClick={() => { openTerminalSettings(); setLocationContext(null); }}><Settings2 size={13} /> Choose terminal application…</button>
      <button onClick={() => { void api.revealInFileManager(locationContext.location.path); setLocationContext(null); }}><Folder size={13} /> Reveal in Finder</button>
      <button onClick={() => { void removeLocation(locationContext.location.id); setLocationContext(null); }}><X size={13} /> Remove Location</button>
    </div></div>}
    {fileContext && <div className="context-backdrop" onMouseDown={() => setFileContext(null)}><div className="context-menu" style={{ left: fileContext.x, top: fileContext.y }} onMouseDown={(event) => event.stopPropagation()}>
      <button onClick={() => { openFile(fileContext.file); setFileContext(null); }}>Open</button>
      <button onClick={() => { openFile(fileContext.file, true); setFileContext(null); }}>Open to the right</button>
      <button onClick={() => { requestTerminal({ locationId: fileContext.locationId, relativeDirectory: relativeDirectoryForFile(fileContext.file.relativePath) }); setFileContext(null); }}><SquareTerminal size={13} /> Open terminal here</button>
      <button onClick={() => { openTerminalSettings(); setFileContext(null); }}><Settings2 size={13} /> Choose terminal application…</button>
      <button onClick={() => { void navigator.clipboard.writeText(fileContext.file.path); setFileContext(null); notify("Path copied."); }}><Clipboard size={13} /> Copy path</button>
      <button onClick={() => { void api.revealInFileManager(fileContext.file.path); setFileContext(null); }}>Reveal in Finder</button>
    </div></div>}
    {tabContext && <div className="context-backdrop" onMouseDown={() => setTabContext(null)}><div className="context-menu" style={{ left: tabContext.x, top: tabContext.y }} onMouseDown={(event) => event.stopPropagation()}>
      <button onClick={() => { void reloadTab(tabContext.paneId, tabContext.tab.id); setTabContext(null); }}>Reload from disk</button>
      <button onClick={() => { requestTerminal({ locationId: tabContext.tab.locationId, relativeDirectory: relativeDirectoryForFile(tabContext.tab.relativePath) }); setTabContext(null); }}><SquareTerminal size={13} /> Open terminal here</button>
      <button onClick={() => { openTerminalSettings(); setTabContext(null); }}><Settings2 size={13} /> Choose terminal application…</button>
      <button onClick={() => { void navigator.clipboard.writeText(tabContext.tab.path); setTabContext(null); notify("Path copied."); }}><Clipboard size={13} /> Copy path</button>
      <button onClick={() => { void api.revealInFileManager(tabContext.tab.path); setTabContext(null); }}>Reveal in Finder</button>
    </div></div>}
    {mcpDialog && <div className="modal-backdrop" onMouseDown={() => setMcpDialog(null)}><div className="mcp-access-modal" role="dialog" aria-modal="true" aria-labelledby="mcp-access-title" onKeyDown={(event) => { if (event.key === "Escape") setMcpDialog(null); }} onMouseDown={(event) => event.stopPropagation()}>
      <h2 id="mcp-access-title">Connect agents</h2>
      <p>Copy a ready-to-paste MCP configuration and choose which registered Locations the external client may read.</p>
      <div className="mcp-access-options" role="radiogroup" aria-label="Agent access scope">
        <label className={mcpDialog.mode === "current" ? "selected" : ""}>
          <input type="radio" name="mcp-access" checked={mcpDialog.mode === "current"} disabled={!activeLocation} onChange={() => setMcpDialog({ mode: "current", locationIds: activeLocation ? [activeLocation.id] : [] })} />
          <span><strong>Current Location</strong><small>{activeLocation ? activeLocation.name : "Select a Location first"}</small></span>
        </label>
        <label className={mcpDialog.mode === "custom" ? "selected" : ""}>
          <input type="radio" name="mcp-access" checked={mcpDialog.mode === "custom"} disabled={!locations.length} onChange={() => setMcpDialog((current) => current ? { ...current, mode: "custom" } : current)} />
          <span><strong>Choose Locations</strong><small>Grant access only to the folders selected below.</small></span>
        </label>
        {mcpDialog.mode === "custom" && <div className="mcp-location-options">{locations.map((location) => <label key={location.id}>
          <input type="checkbox" checked={mcpDialog.locationIds.includes(location.id)} onChange={() => setMcpDialog((current) => {
            if (!current) return current;
            const locationIds = current.locationIds.includes(location.id)
              ? current.locationIds.filter((id) => id !== location.id)
              : [...current.locationIds, location.id];
            return { ...current, locationIds };
          })} />
          <span>{location.name}</span>
        </label>)}</div>}
        <label className={mcpDialog.mode === "all" ? "selected" : ""}>
          <input type="radio" name="mcp-access" checked={mcpDialog.mode === "all"} disabled={!locations.length} onChange={() => setMcpDialog((current) => current ? { ...current, mode: "all" } : current)} />
          <span><strong>All Locations</strong><small>Also grants access to Locations registered in the future.</small></span>
        </label>
      </div>
      <p className="mcp-access-warning">Construct exposes read-only knowledge tools. Retrieved content leaves Construct’s control when the external client sends it to its configured model.</p>
      <div className="mcp-access-actions"><button onClick={() => setMcpDialog(null)}>Cancel</button><button className="primary-button" disabled={!locations.length || (mcpDialog.mode !== "all" && mcpDialog.mode !== "current" && !mcpDialog.locationIds.length) || (mcpDialog.mode === "current" && !activeLocation)} onClick={() => void copyMcpConfiguration()}>Copy configuration</button></div>
    </div></div>}
    {terminalPicker && <div className="modal-backdrop" onMouseDown={() => setTerminalPicker(null)}><div className="terminal-picker-modal" role="dialog" aria-modal="true" aria-labelledby="terminal-picker-title" onKeyDown={(event) => { if (event.key === "Escape") setTerminalPicker(null); }} onMouseDown={(event) => event.stopPropagation()}>
      <h2 id="terminal-picker-title">Choose terminal application</h2>
      <p>{terminalPicker.target ? "Construct will remember your choice and open this directory." : "Construct will use this application for future terminal actions."}</p>
      <div className="terminal-application-list">{terminalApplications.map((application, index) => <button key={application.id} autoFocus={application.id === terminalApplicationId || (!terminalApplicationId && index === 0)} className={application.id === terminalApplicationId ? "selected" : ""} onClick={() => chooseTerminal(application)}><SquareTerminal size={16} /><span><strong>{application.label}</strong><small>{application.id === terminalApplicationId ? "Current selection" : "Installed"}</small></span></button>)}</div>
      <div className="terminal-picker-actions"><button onClick={() => setTerminalPicker(null)}>Cancel</button></div>
    </div></div>}
    {pendingClose && <div className="modal-backdrop"><div className="confirm-modal"><h2>Save changes?</h2><p>This file has unsaved changes.</p><div><button onClick={() => setPendingClose(null)}>Cancel</button><button className="danger-button" onClick={() => { discardTab(pendingClose.paneId, pendingClose.tabId); setPendingClose(null); }}>Don’t save</button><button className="primary-button" onClick={() => { const request = pendingClose; setPendingClose(null); void saveTab(request.paneId, request.tabId).then((saved) => { if (saved) discardTab(request.paneId, request.tabId); }); }}>Save</button></div></div></div>}
    {notice && <div className="toast">{notice}</div>}
  </main>;
}

function DiffView({ tab }: { tab: DocumentTab }) {
  const requestKey = `${tab.path}\0${tab.dirty ? tab.content : ""}`;
  const [result, setResult] = useState<{ key: string; diff: string; message: string | null }>({
    key: "",
    diff: "",
    message: null,
  });
  useEffect(() => {
    let cancelled = false;
    void api.getGitDiff(tab.path, tab.dirty ? tab.content : undefined)
      .then((next) => {
        if (!cancelled) setResult({ key: requestKey, diff: next.diff, message: next.message });
      })
      .catch((error) => {
        if (!cancelled) setResult({ key: requestKey, diff: "", message: String(error) });
      });
    return () => {
      cancelled = true;
    };
  }, [requestKey, tab.content, tab.dirty, tab.path]);
  if (result.key !== requestKey) return <div className="diff-view empty-pane">Generating diff…</div>;
  return <div className="diff-view">{result.message && <p className="diff-message">{result.message}</p>}{result.diff ? <pre>{result.diff.split("\n").map((line, index) => <span key={index} className={line.startsWith("+") ? "addition" : line.startsWith("-") ? "removal" : line.startsWith("@@") ? "hunk" : ""}>{line}{"\n"}</span>)}</pre> : <div className="empty-pane"><p>No differences from HEAD.</p></div>}</div>;
}
