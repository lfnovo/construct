import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, ChevronRight, CirclePlus, Clipboard, Columns2, FileText, Folder, FolderOpen, History, MapPin, Moon, PanelLeftClose, PanelLeftOpen, PanelTop, Rows3, Sun, X } from "lucide-react";
import { api } from "./api";
import { CodeEditor } from "./CodeEditor";
import { MarkdownPreview } from "./MarkdownPreview";
import { inspectOkfDocument } from "./okf";
import type {
  DocumentTab, FileEntry, FileFingerprint, FileSystemChange, HistoryEvent, HistoryKind,
  LayoutNode, LocationRecord, Pane, SavedPane, SavedWorkspace, TabMode,
} from "./types";

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

type TreeNode = { children: Map<string, TreeNode>; entry?: FileEntry };

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
        <button className="folder-row" style={{ paddingLeft: 7 + depth * 15 }} key={key} onClick={() => setExpanded((current) => { const next = new Set(current); isOpen ? next.delete(key) : next.add(key); return next; })}>
          {isOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />} {isOpen ? <FolderOpen className="folder-icon" size={14} /> : <Folder className="folder-icon" size={14} />}<span>{name}</span>
        </button>,
        ...(isOpen ? render(child, key, depth + 1) : []),
      ];
    });
  return <div className="file-tree">{entries.length ? render(tree, "") : <p className="empty-sidebar">No Markdown files found.</p>}</div>;
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
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({});
  const [theme, setTheme] = useState<"dark" | "light">("dark");
  const [ready, setReady] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [quickOpen, setQuickOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [historyFilter, setHistoryFilter] = useState<HistoryKind | "all">("all");
  const [fileContext, setFileContext] = useState<{ file: FileEntry; x: number; y: number } | null>(null);
  const [pendingClose, setPendingClose] = useState<{ paneId: string; tabId: string } | null>(null);
  const locationsRef = useRef(locations);
  const filesRef = useRef(filesByLocation);
  const panesRef = useRef(panes);
  const refreshTimer = useRef<number | undefined>(undefined);
  locationsRef.current = locations;
  filesRef.current = filesByLocation;
  panesRef.current = panes;

  const notify = useCallback((message: string) => { setNotice(message); window.setTimeout(() => setNotice((current) => current === message ? null : current), 4200); }, []);

  const addHistory = useCallback((events: HistoryEvent[]) => {
    if (!events.length) return;
    const cutoff = Date.now() - 30 * 86_400_000;
    setHistory((previous) => [...events, ...previous].filter((event) => event.observedAt >= cutoff).slice(0, 5000));
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

  const detectOkfBundle = useCallback(async (location: LocationRecord, entries: FileEntry[]) => {
    const rootIndex = entries.find((entry) => entry.relativePath === "index.md");
    if (!rootIndex) return;
    try {
      const { content } = await api.readMarkdownFile(rootIndex.path);
      const detected = Boolean(inspectOkfDocument(content, rootIndex.relativePath, true).metadata.okfVersion);
      setLocations((current) => current.map((item) => {
        if (item.id !== location.id || item.okfMode === "manual" || item.okfMode === "disabled") return item;
        return detected ? { ...item, okfBundle: true, okfMode: "auto" } : { ...item, okfBundle: false, okfMode: "auto" };
      }));
    } catch { /* a failed probe must not affect the Location */ }
  }, []);

  const refreshLocation = useCallback(async (location: LocationRecord, source: HistoryEvent["source"] = "external") => {
    try {
      const entries = await api.listMarkdownFiles(location.path);
      setFilesByLocation((current) => ({ ...current, [location.id]: entries }));
      reconcile(location, entries, source);
      void detectOkfBundle(location, entries);
      const entriesByPath = new Map(entries.map((entry) => [entry.path, entry]));
      const candidates = Object.values(panesRef.current).flatMap((pane) => pane.tabs.filter((tab) => tab.locationId === location.id).map((tab) => ({ paneId: pane.id, tab })));
      setPanes((current) => Object.fromEntries(Object.entries(current).map(([paneId, pane]) => [paneId, {
        ...pane,
        tabs: pane.tabs.flatMap((tab) => {
          if (tab.locationId !== location.id) return [tab];
          const entry = entriesByPath.get(tab.path);
          if (entry) return [tab];
          return tab.dirty ? [{ ...tab, deleted: true }] : [];
        }),
        activeTabId: pane.tabs.some((tab) => tab.id === pane.activeTabId && (tab.locationId !== location.id || entriesByPath.has(tab.path) || tab.dirty)) ? pane.activeTabId : pane.tabs.find((tab) => tab.locationId !== location.id || entriesByPath.has(tab.path) || tab.dirty)?.id || null,
      }])) as Record<string, Pane>);
      for (const { paneId, tab } of candidates) {
        const entry = entriesByPath.get(tab.path);
        if (!entry || entry.modifiedAtMs === tab.diskModifiedAtMs) continue;
        void api.readMarkdownFile(tab.path).then((contents) => setPanes((current) => ({ ...current, [paneId]: {
          ...current[paneId], tabs: current[paneId].tabs.map((currentTab) => currentTab.id !== tab.id ? currentTab : currentTab.dirty
            ? { ...currentTab, conflict: true, diskModifiedAtMs: contents.modifiedAtMs }
            : { ...currentTab, content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, conflict: false, deleted: false }),
        } })));
      }
      setLocations((current) => current.map((item) => item.id === location.id ? { ...item, available: true } : item));
    } catch {
      setLocations((current) => current.map((item) => item.id === location.id ? { ...item, available: false } : item));
      setFilesByLocation((current) => ({ ...current, [location.id]: [] }));
    }
  }, [detectOkfBundle, reconcile]);

  const refreshAll = useCallback((source: HistoryEvent["source"] = "external") => Promise.all(locationsRef.current.map((location) => refreshLocation(location, source))), [refreshLocation]);

  const configureLocations = useCallback(async (next: LocationRecord[]) => {
    try {
      const watched = await api.setWatchedLocations(next.map((location) => location.path));
      setLocations((current) => current.map((location) => ({ ...location, available: watched.includes(location.path) })));
      return watched;
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
      return [];
    }
  }, [notify]);

  const setPane = useCallback((paneId: string, updater: (pane: Pane) => Pane) => {
    setPanes((current) => ({ ...current, [paneId]: updater(current[paneId]) }));
  }, []);

  const findTab = useCallback((paneId = activePaneId) => {
    const pane = panes[paneId];
    return pane?.tabs.find((tab) => tab.id === pane.activeTabId) || null;
  }, [activePaneId, panes]);

  const openFile = useCallback(async (file: FileEntry, newPane = false) => {
    let targetId = activePaneId;
    if (newPane) {
      const nextId = crypto.randomUUID();
      setPanes((current) => ({ ...current, [nextId]: emptyPane(nextId) }));
      setLayout((current) => updateLayoutPane(current, activePaneId, { type: "split", direction: "horizontal", ratio: 0.5, first: { type: "pane", paneId: activePaneId }, second: { type: "pane", paneId: nextId } }));
      targetId = nextId;
      setActivePaneId(nextId);
    }
    const pane = panes[targetId];
    const existing = pane?.tabs.find((tab) => tab.path === file.path);
    if (existing) { setPane(targetId, (current) => ({ ...current, activeTabId: existing.id })); return; }
    const location = locationsRef.current.find((item) => file.path.startsWith(item.path));
    if (!location) return notify("This file does not belong to an available location.");
    try {
      const [contents, git] = await Promise.all([api.readMarkdownFile(file.path), api.getGitInfo(file.path)]);
      const tab: DocumentTab = { id: crypto.randomUUID(), path: file.path, locationId: location.id, title: file.name, relativePath: file.relativePath, mode: "preview", content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, dirty: false, conflict: false, deleted: false, git };
      setPane(targetId, (current) => ({ ...current, tabs: [...current.tabs, tab], activeTabId: tab.id }));
    } catch (error) { notify(error instanceof Error ? error.message : String(error)); }
  }, [activePaneId, notify, panes, setPane]);

  const openPath = useCallback((path: string) => {
    const file = Object.values(filesRef.current).flat().find((item) => item.path === path);
    if (file) void openFile(file);
    else notify("The linked file does not exist in an available Location.");
  }, [notify, openFile]);

  const updateTab = useCallback((paneId: string, tabId: string, updater: (tab: DocumentTab) => DocumentTab) => {
    setPane(paneId, (pane) => ({ ...pane, tabs: pane.tabs.map((tab) => tab.id === tabId ? updater(tab) : tab) }));
  }, [setPane]);

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
    if (locationsRef.current.some((location) => location.path === selected)) return notify("This folder has already been added.");
    const location: LocationRecord = { id: crypto.randomUUID(), path: selected, name: basename(selected), available: true };
    const next = [...locationsRef.current, location];
    setLocations(next);
    await configureLocations(next);
    setSelectedLocationId(location.id);
    await refreshLocation(location, "reconciliation");
  }, [configureLocations, notify, refreshLocation]);

  const removeLocation = useCallback(async (locationId: string) => {
    const location = locationsRef.current.find((item) => item.id === locationId);
    if (!location || !window.confirm(`Remove “${location.name}” from Agent Context? Its files will not be deleted.`)) return;
    const next = locationsRef.current.filter((item) => item.id !== locationId);
    setLocations(next);
    setSelectedLocationId((current) => current === locationId ? next[0]?.id || null : current);
    setFilesByLocation((current) => { const { [locationId]: _, ...rest } = current; return rest; });
    await configureLocations(next);
  }, [configureLocations]);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const saved = await api.loadState();
        if (!mounted) return;
        const restoredLocations = saved.locations || [];
        const restoredPanes = saved.panes?.length ? saved.panes : [{ ...defaultPane, tabs: [] } as SavedPane];
        const restoredLayout = saved.layout || defaultLayout;
        setLocations(restoredLocations);
        setHistory((saved.history || []).filter((event) => event.observedAt >= Date.now() - 30 * 86_400_000));
        setFingerprints(saved.fingerprints || {});
        setSelectedLocationId(saved.selectedLocationId || restoredLocations[0]?.id || null);
        setSidebarWidth(saved.sidebarWidth || 295);
        setSidebarHidden(saved.sidebarHidden || false);
        setCollapsedSections(saved.collapsedSections || {});
        setTheme(saved.theme || "dark");
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
            } catch { /* arquivo ausente permanece fora da restauração */ }
          }
          hydrated[savedPane.id] = { id: savedPane.id, tabs, activeTabId: tabs.some((tab) => tab.id === savedPane.activeTabId) ? savedPane.activeTabId : tabs.at(-1)?.id || null };
        }
        if (!Object.keys(hydrated).length) hydrated.main = defaultPane;
        setPanes(hydrated);
        await Promise.all(restoredLocations.map((location) => refreshLocation(location, "reconciliation")));
      } catch (error) { notify(error instanceof Error ? error.message : String(error)); }
      finally { if (mounted) setReady(true); }
    })();
    return () => { mounted = false; };
  }, [configureLocations, notify, refreshLocation]);

  useEffect(() => {
    const unlisten = listen<FileSystemChange>("filesystem-change", () => {
      window.clearTimeout(refreshTimer.current);
      refreshTimer.current = window.setTimeout(() => { void refreshAll("external"); }, 450);
    });
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [refreshAll]);

  useEffect(() => {
    if (!ready) return;
    const handle = window.setTimeout(() => {
      const savedPanes: SavedPane[] = Object.values(panes).map((pane) => ({ ...pane, tabs: pane.tabs.map(({ id, path, locationId, title, relativePath, mode }) => ({ id, path, locationId, title, relativePath, mode })) }));
      const state: SavedWorkspace = { locations, history, fingerprints, panes: savedPanes, layout, activePaneId, selectedLocationId, sidebarWidth, sidebarHidden, collapsedSections, theme };
      void api.saveState(state).catch(() => undefined);
    }, 450);
    return () => window.clearTimeout(handle);
  }, [activePaneId, collapsedSections, fingerprints, history, layout, locations, panes, ready, selectedLocationId, sidebarHidden, sidebarWidth, theme]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.metaKey) return;
      if (event.key.toLowerCase() === "p") { event.preventDefault(); setQuickOpen(true); }
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
  }, [activePaneId, closeTab, findTab, saveTab]);

  const activeLocation = locations.find((location) => location.id === selectedLocationId) || null;
  const fileResults = useMemo(() => Object.entries(filesByLocation).flatMap(([locationId, files]) => files.map((file) => ({ ...file, locationId }))).filter((file) => `${file.name} ${file.relativePath}`.toLowerCase().includes(query.toLowerCase())).slice(0, 100), [filesByLocation, query]);
  const visibleHistory = history.filter((event) => historyFilter === "all" || event.kind === historyFilter);

  const resizeSidebar = (event: React.PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const onMove = (move: PointerEvent) => setSidebarWidth(Math.max(220, Math.min(520, move.clientX)));
    const onUp = () => { window.removeEventListener("pointermove", onMove); window.removeEventListener("pointerup", onUp); };
    window.addEventListener("pointermove", onMove); window.addEventListener("pointerup", onUp);
  };

  const renderPane = (pane: Pane, active: boolean) => {
    const tab = pane.tabs.find((item) => item.id === pane.activeTabId) || null;
    const tabLocation = tab ? locationsRef.current.find((location) => location.id === tab.locationId) : null;
    const okf = tab && tabLocation?.okfBundle ? inspectOkfDocument(tab.content, tab.relativePath, tab.relativePath === "index.md") : null;
    const changeContent = (content: string) => tab && updateTab(pane.id, tab.id, (current) => ({ ...current, content, dirty: content !== current.baseContent }));
    const reloadExternal = () => tab && api.readMarkdownFile(tab.path).then((contents) => updateTab(pane.id, tab.id, (current) => ({ ...current, content: contents.content, baseContent: contents.content, lineEnding: contents.lineEnding, diskModifiedAtMs: contents.modifiedAtMs, dirty: false, conflict: false, deleted: false }))).catch((error) => notify(String(error)));
    return <section className={`editor-pane ${active ? "active" : ""}`}>
      <div className="tab-bar" onDragOver={(event) => event.preventDefault()} onDrop={(event) => {
        const payload = event.dataTransfer.getData("application/agent-context-tab");
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
        <div className="tabs-scroll">{pane.tabs.map((item) => <div key={item.id} draggable className={`tab ${item.id === pane.activeTabId ? "selected" : ""}`} onDragStart={(event) => event.dataTransfer.setData("application/agent-context-tab", JSON.stringify({ paneId: pane.id, tabId: item.id }))} onClick={() => setPane(pane.id, (current) => ({ ...current, activeTabId: item.id }))} title={item.relativePath}>
          <span className={item.dirty ? "dirty-dot" : "file-tab-icon"}>{item.dirty ? "●" : "#"}</span><span>{item.title}</span><button aria-label={`Fechar ${item.title}`} onClick={(event) => { event.stopPropagation(); closeTab(pane.id, item.id); }}>×</button>
        </div>)}</div>
        <button className="icon-button" title="Split vertically" onClick={() => { setActivePaneId(pane.id); splitPane("horizontal"); }}><Columns2 size={14} /></button>
        <button className="icon-button" title="Split horizontally" onClick={() => { setActivePaneId(pane.id); splitPane("vertical"); }}><Rows3 size={14} /></button>
        {getPaneIds(layout).length > 1 && <button className="icon-button" title="Close pane" onClick={() => closePane(pane.id)}><X size={14} /></button>}
      </div>
      {!tab ? <div className="empty-pane"><h2>Open a context file</h2><p>Choose a file from the sidebar or use <kbd>⌘P</kbd>.</p></div> : <>
        <div className="document-toolbar">
          <span className="document-path" title={tab.path}>{tab.relativePath}</span>
          {okf && <button className={`okf-status ${okf.isConformant ? "valid" : "invalid"}`} title="Open Knowledge Format details" onClick={() => setShowOkfInspector((current) => !current)}>OKF</button>}
          <div className="mode-switch">
            {(["preview", "source", "diff"] as TabMode[]).map((mode) => <button key={mode} className={tab.mode === mode ? "selected" : ""} disabled={mode === "diff" && !tab.git?.available} onClick={() => updateTab(pane.id, tab.id, (current) => ({ ...current, mode }))}>{mode === "preview" ? "Preview" : mode === "source" ? "Source" : "Diff"}</button>)}
          </div>
          <button className="toolbar-button" disabled={!tab.dirty || tab.deleted} onClick={() => void saveTab(pane.id, tab.id)}>Save</button>
          <button className="icon-button" title="Reveal in Finder" onClick={() => void api.revealInFileManager(tab.path)}><Folder size={14} /></button>
        </div>
        {tab.conflict && <div className="conflict-banner"><span>This file changed outside the app.</span><button onClick={() => void reloadExternal()}>Reload external version</button><button onClick={() => updateTab(pane.id, tab.id, (current) => ({ ...current, conflict: false }))}>Keep my changes</button></div>}
        {tab.deleted && <div className="conflict-banner danger"><span>This file was removed outside the app.</span><button onClick={() => updateTab(pane.id, tab.id, (current) => ({ ...current, deleted: false }))}>Save again to this path</button></div>}
        {okf && showOkfInspector && <aside className="okf-inspector">
          <div className="okf-inspector-heading"><strong>Open Knowledge Format</strong><span className={okf.isConformant ? "okf-valid" : "okf-invalid"}>{okf.isConformant ? "Conformant" : "Needs attention"}</span></div>
          <div className="okf-kind">{okf.kind === "concept" ? "Concept document" : okf.kind === "index" ? "Directory index" : "Update log"}</div>
          <dl className="okf-metadata">
            {okf.metadata.type && <><dt>Type</dt><dd>{okf.metadata.type}</dd></>}
            {okf.metadata.title && <><dt>Title</dt><dd>{okf.metadata.title}</dd></>}
            {okf.metadata.description && <><dt>Description</dt><dd>{okf.metadata.description}</dd></>}
            {okf.metadata.resource && <><dt>Resource</dt><dd><a href={okf.metadata.resource} onClick={(event) => { event.preventDefault(); void api.openExternalUrl(okf.metadata.resource!); }}>{okf.metadata.resource}</a></dd></>}
            {okf.metadata.timestamp && <><dt>Timestamp</dt><dd>{okf.metadata.timestamp}</dd></>}
            {okf.metadata.okfVersion && <><dt>OKF version</dt><dd>{okf.metadata.okfVersion}</dd></>}
          </dl>
          {!!okf.metadata.tags.length && <div className="okf-tags">{okf.metadata.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}
          {!!okf.issues.length && <ul className="okf-issues">{okf.issues.map((issue) => <li key={issue.message} className={issue.level}>{issue.message}</li>)}</ul>}
        </aside>}
        <div className="document-content">
          {tab.mode === "source" && <CodeEditor tabId={tab.id} value={tab.content} readOnly={tab.deleted} onChange={changeContent} onSave={() => void saveTab(pane.id, tab.id)} />}
          {tab.mode === "preview" && <MarkdownPreview content={tab.content} sourcePath={tab.path} bundleRoot={tabLocation?.okfBundle ? tabLocation.path : undefined} onOpenInternal={openPath} />}
          {tab.mode === "diff" && <DiffView tab={tab} />}
        </div>
      </>}
    </section>;
  };

  if (!ready) return <main className="startup"><div className="startup-mark">✦</div><p>Preparing your workspace…</p></main>;

  return <main className={`app-shell ${sidebarHidden ? "sidebar-hidden" : ""}`} data-theme={theme} style={{ gridTemplateColumns: sidebarHidden ? "38px minmax(0, 1fr)" : `${sidebarWidth}px 5px minmax(0, 1fr)` }}>
    {sidebarHidden ? <aside className="sidebar-rail"><button className="sidebar-toggle" onClick={() => setSidebarHidden(false)} title="Show sidebar" aria-label="Show sidebar"><PanelLeftOpen size={16} /></button></aside> : <aside className="sidebar">
      <section className="sidebar-section locations-section">
        <div className="section-title"><button onClick={() => setCollapsedSections((current) => ({ ...current, locations: !current.locations }))}>{collapsedSections.locations ? <ChevronRight size={13} /> : <ChevronDown size={13} />}</button><MapPin size={13} /><span>LOCATIONS</span><button className="sidebar-toggle" onClick={() => setSidebarHidden(true)} title="Hide sidebar" aria-label="Hide sidebar"><PanelLeftClose size={16} /></button><button className="theme-button" onClick={() => setTheme((current) => current === "dark" ? "light" : "dark")} title={theme === "dark" ? "Use light theme" : "Use dark theme"}>{theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}</button><button className="add-button" onClick={() => void addLocation()} title="Add folder"><CirclePlus size={15} /></button></div>
        {!collapsedSections.locations && <div className="location-list">{locations.length ? locations.map((location) => <div key={location.id} draggable className={`location-row ${location.id === selectedLocationId ? "selected" : ""}`} onDragStart={(event) => event.dataTransfer.setData("application/agent-context-location", location.id)} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); const movedId = event.dataTransfer.getData("application/agent-context-location"); if (!movedId || movedId === location.id) return; setLocations((current) => { const moved = current.find((item) => item.id === movedId); if (!moved) return current; const remaining = current.filter((item) => item.id !== movedId); const index = remaining.findIndex((item) => item.id === location.id); remaining.splice(index, 0, moved); return remaining; }); }} onClick={() => setSelectedLocationId(location.id)} title={location.path}>
          <span className={`availability ${location.available ? "online" : "offline"}`} /><span className="location-name">{location.name}</span>{location.okfBundle && <button className="okf-toggle active" onClick={(event) => { event.stopPropagation(); setLocations((current) => current.map((item) => item.id === location.id ? { ...item, okfBundle: false, okfMode: "disabled" } : item)); }} title={location.okfMode === "auto" ? "OKF bundle detected automatically — click to disable" : "Remove OKF bundle marker"}>OKF</button>}<button onClick={(event) => { event.stopPropagation(); void removeLocation(location.id); }} title="Remove location"><X size={14} /></button>
        </div>) : <div className="empty-sidebar">Add your project folders to get started.</div>}</div>}
      </section>
      <section className="sidebar-section files-section">
        <div className="section-title"><button onClick={() => setCollapsedSections((current) => ({ ...current, files: !current.files }))}>{collapsedSections.files ? <ChevronRight size={13} /> : <ChevronDown size={13} />}</button><Folder size={13} /><span>FILES</span>{activeLocation && <span className="section-subtitle" title={activeLocation.path}>{activeLocation.name}</span>}</div>
        {!collapsedSections.files && (activeLocation ? <FileTree entries={filesByLocation[activeLocation.id] || []} onOpen={openFile} onContext={(event, file) => { event.preventDefault(); setFileContext({ file, x: event.clientX, y: event.clientY }); }} /> : <div className="empty-sidebar">Select a Location.</div>)}
      </section>
      <section className="sidebar-section history-section">
        <div className="section-title"><button onClick={() => setCollapsedSections((current) => ({ ...current, history: !current.history }))}>{collapsedSections.history ? <ChevronRight size={13} /> : <ChevronDown size={13} />}</button><History size={13} /><span>HISTORY</span><select aria-label="Filter history" value={historyFilter} onChange={(event) => setHistoryFilter(event.target.value as HistoryKind | "all")}><option value="all">All</option><option value="created">New</option><option value="modified">Changed</option><option value="renamed">Renamed</option><option value="removed">Removed</option></select><button className="clear-history" title="Clear history" onClick={() => { if (window.confirm("Clear all local history? This will not alter any files.")) setHistory([]); }}><X size={13} /></button></div>
        {!collapsedSections.history && <div className="history-list">{visibleHistory.length ? visibleHistory.map((event) => <button key={event.id} className="history-row" onClick={() => event.available ? openPath(event.path) : notify("Este arquivo não está mais disponível.")} title={event.previousPath ? `${event.previousPath} → ${event.path}` : event.path}>
          <span className={`history-kind ${event.kind}`}>{statusLabel(event.kind)}</span><span className="history-file">{basename(event.path)}</span><time>{formatWhen(event.observedAt)}</time>
        </button>) : <div className="empty-sidebar">Changes from your agents will appear here.</div>}</div>}
      </section>
    </aside>}
    {!sidebarHidden && <div className="sidebar-resizer" onPointerDown={resizeSidebar} />}
    <section className="workspace"><SplitView node={layout} panes={panes} activePaneId={activePaneId} onActivate={setActivePaneId} onRatio={(node, ratio) => setLayout((current) => updateSplitRatio(current, node, ratio))}>{renderPane}</SplitView></section>
    {quickOpen && <div className="quick-open-backdrop" onMouseDown={() => setQuickOpen(false)}><div className="quick-open" onMouseDown={(event) => event.stopPropagation()}><input autoFocus placeholder="Open file…" value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") setQuickOpen(false); if (event.key === "Enter" && fileResults[0]) { openFile(fileResults[0]); setQuickOpen(false); } }} />
      <div className="quick-results">{fileResults.map((file) => <button key={file.path} onClick={() => { openFile(file); setQuickOpen(false); }}><span>{file.name}</span><small>{locations.find((location) => location.id === file.locationId)?.name} · {file.relativePath}</small></button>)}{!fileResults.length && <p>No files found.</p>}</div></div></div>}
    {fileContext && <div className="context-backdrop" onMouseDown={() => setFileContext(null)}><div className="context-menu" style={{ left: fileContext.x, top: fileContext.y }} onMouseDown={(event) => event.stopPropagation()}>
      <button onClick={() => { openFile(fileContext.file); setFileContext(null); }}>Open</button>
      <button onClick={() => { openFile(fileContext.file, true); setFileContext(null); }}>Open to the right</button>
      <button onClick={() => { void navigator.clipboard.writeText(fileContext.file.path); setFileContext(null); notify("Path copied."); }}><Clipboard size={13} /> Copy path</button>
      <button onClick={() => { void api.revealInFileManager(fileContext.file.path); setFileContext(null); }}>Reveal in Finder</button>
    </div></div>}
    {pendingClose && <div className="modal-backdrop"><div className="confirm-modal"><h2>Save changes?</h2><p>This file has unsaved changes.</p><div><button onClick={() => setPendingClose(null)}>Cancel</button><button className="danger-button" onClick={() => { discardTab(pendingClose.paneId, pendingClose.tabId); setPendingClose(null); }}>Don’t save</button><button className="primary-button" onClick={() => { const request = pendingClose; setPendingClose(null); void saveTab(request.paneId, request.tabId).then((saved) => { if (saved) discardTab(request.paneId, request.tabId); }); }}>Save</button></div></div></div>}
    {notice && <div className="toast">{notice}</div>}
  </main>;
}

function DiffView({ tab }: { tab: DocumentTab }) {
  const [loading, setLoading] = useState(true);
  const [diff, setDiff] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  useEffect(() => {
    setLoading(true);
    void api.getGitDiff(tab.path, tab.dirty ? tab.content : undefined).then((result) => { setDiff(result.diff); setMessage(result.message); }).catch((error) => setMessage(String(error))).finally(() => setLoading(false));
  }, [tab.content, tab.dirty, tab.path]);
  if (loading) return <div className="diff-view empty-pane">Generating diff…</div>;
  return <div className="diff-view">{message && <p className="diff-message">{message}</p>}{diff ? <pre>{diff.split("\n").map((line, index) => <span key={index} className={line.startsWith("+") ? "addition" : line.startsWith("-") ? "removal" : line.startsWith("@@") ? "hunk" : ""}>{line}{"\n"}</span>)}</pre> : <div className="empty-pane"><p>No differences from HEAD.</p></div>}</div>;
}
