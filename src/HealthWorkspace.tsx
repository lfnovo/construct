import { useMemo, useState } from "react";
import { Clipboard, RefreshCw, Search, ShieldCheck } from "lucide-react";
import {
  buildHealthAgentPrompt,
  filterHealthFindings,
  findingsForScope,
  groupHealthFindings,
  summarizeHealth,
  type HealthScope,
  type HealthSeverity,
} from "./health";
import type { OkfFinding } from "./okf";

type Props = {
  locationName: string;
  documents: number;
  findings: OkfFinding[];
  ignoredPaths: string[];
  onOpen: (relativePath: string) => void;
  onRefresh: () => Promise<void>;
  onNotify: (message: string) => void;
};

const severityLabels: Record<Exclude<HealthSeverity, "all">, string> = {
  error: "Errors",
  warning: "Warnings",
  info: "Info",
};

function locationLabel(finding: OkfFinding) {
  const line = finding.range?.startLine;
  return line ? `${finding.relativePath}:${line}` : finding.relativePath;
}

export function HealthWorkspace({
  locationName,
  documents,
  findings,
  ignoredPaths,
  onOpen,
  onRefresh,
  onNotify,
}: Props) {
  const [scope, setScope] = useState<HealthScope>("policy");
  const [severity, setSeverity] = useState<HealthSeverity>("all");
  const [query, setQuery] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const scopedFindings = useMemo(
    () => findingsForScope(findings, scope, ignoredPaths),
    [findings, ignoredPaths, scope],
  );
  const summary = useMemo(() => summarizeHealth(scopedFindings), [scopedFindings]);
  const visibleFindings = useMemo(
    () => filterHealthFindings(scopedFindings, severity, query),
    [query, scopedFindings, severity],
  );
  const groups = useMemo(() => groupHealthFindings(visibleFindings), [visibleFindings]);
  const ignoredFindingCount = findings.length - findingsForScope(
    findings,
    "policy",
    ignoredPaths,
  ).length;

  const refresh = async () => {
    setRefreshing(true);
    try {
      await onRefresh();
    } catch (error) {
      onNotify(`Could not run the linter: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setRefreshing(false);
    }
  };

  const copyForAgent = async () => {
    try {
      await navigator.clipboard.writeText(buildHealthAgentPrompt(
        locationName,
        documents,
        scope,
        scopedFindings,
      ));
      onNotify(`Copied ${scopedFindings.length} finding${scopedFindings.length === 1 ? "" : "s"} for an agent.`);
    } catch (error) {
      onNotify(`Could not copy the lint report: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  return <div className="health-workspace">
    <div className="health-toolbar">
      <div className="health-scope-switch" aria-label="Lint scope">
        <button className={scope === "policy" ? "selected" : ""} onClick={() => setScope("policy")}>Repository policy</button>
        <button className={scope === "all" ? "selected" : ""} onClick={() => setScope("all")}>All Markdown</button>
      </div>
      <div className="health-toolbar-actions">
        <button onClick={() => void copyForAgent()}><Clipboard size={13} /> Copy for agent</button>
        <button disabled={refreshing} onClick={() => void refresh()}><RefreshCw className={refreshing ? "spinning" : ""} size={13} /> {refreshing ? "Running…" : "Run lint"}</button>
      </div>
    </div>

    <div className="health-summary" aria-label="Lint summary">
      <button className={`error ${severity === "error" ? "selected" : ""}`} onClick={() => setSeverity((current) => current === "error" ? "all" : "error")}><strong>{summary.errors}</strong><span>Errors</span></button>
      <button className={`warning ${severity === "warning" ? "selected" : ""}`} onClick={() => setSeverity((current) => current === "warning" ? "all" : "warning")}><strong>{summary.warnings}</strong><span>Warnings</span></button>
      <button className={`info ${severity === "info" ? "selected" : ""}`} onClick={() => setSeverity((current) => current === "info" ? "all" : "info")}><strong>{summary.info}</strong><span>Info</span></button>
      <div><strong>{documents}</strong><span>Documents scanned</span></div>
    </div>

    <div className="health-filter">
      <Search size={14} />
      <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Filter by path, rule, or message…" />
      {(query || severity !== "all") && <button onClick={() => { setQuery(""); setSeverity("all"); }}>Clear</button>}
    </div>

    {scope === "policy" && ignoredPaths.length > 0 && <p className="health-scope-note">
      {ignoredPaths.length} document{ignoredPaths.length === 1 ? "" : "s"} {ignoredPaths.length === 1 ? "is" : "are"} excluded from OKF conformance by .constructignore
      {ignoredFindingCount > 0 ? `, hiding ${ignoredFindingCount} finding${ignoredFindingCount === 1 ? "" : "s"}` : ""}. Choose All Markdown for a strict inspection.
    </p>}

    {!visibleFindings.length ? <div className="health-empty">
      <ShieldCheck size={26} />
      <strong>{scopedFindings.length ? "No findings match these filters." : "No findings in this scope."}</strong>
      <p>{scopedFindings.length ? "Clear the filters to see the full lint report." : "This saved bundle passes the currently implemented checks."}</p>
    </div> : <div className="health-groups">
      {groups.map(([code, codeFindings]) => <section key={code}>
        <header><div><span className={`health-severity ${codeFindings[0].severity}`}>{severityLabels[codeFindings[0].severity]}</span><h3>{code}</h3></div><strong>{codeFindings.length}</strong></header>
        <div>{codeFindings.map((finding, index) => <button key={`${finding.relativePath}-${finding.range?.startLine || 0}-${index}`} onClick={() => onOpen(finding.relativePath)}>
          <div><strong>{locationLabel(finding)}</strong><p>{finding.message}</p></div>
          <span>Open</span>
        </button>)}</div>
      </section>)}
    </div>}
  </div>;
}
