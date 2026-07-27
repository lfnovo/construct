import type { OkfFinding } from "./okf.ts";

export type HealthScope = "policy" | "all";
export type HealthSeverity = OkfFinding["severity"] | "all";

export type HealthSummary = {
  errors: number;
  warnings: number;
  info: number;
};

function normalizePath(relativePath: string) {
  return relativePath.replace(/\\/g, "/");
}

export function findingsForScope(
  findings: OkfFinding[],
  scope: HealthScope,
  ignoredPaths: string[] = [],
) {
  const ignored = new Set(ignoredPaths.map(normalizePath));
  return scope === "all"
    ? findings
    : findings.filter((finding) => !ignored.has(normalizePath(finding.relativePath)));
}

export function filterHealthFindings(
  findings: OkfFinding[],
  severity: HealthSeverity,
  query: string,
) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return findings.filter((finding) => {
    if (severity !== "all" && finding.severity !== severity) return false;
    if (!normalizedQuery) return true;
    return [
      finding.code,
      finding.relativePath,
      finding.message,
    ].some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
  });
}

export function summarizeHealth(findings: OkfFinding[]): HealthSummary {
  return findings.reduce<HealthSummary>((summary, finding) => {
    if (finding.severity === "error") summary.errors += 1;
    else if (finding.severity === "warning") summary.warnings += 1;
    else summary.info += 1;
    return summary;
  }, { errors: 0, warnings: 0, info: 0 });
}

export function groupHealthFindings(findings: OkfFinding[]) {
  const groups = new Map<string, OkfFinding[]>();
  for (const finding of findings) {
    const group = groups.get(finding.code) || [];
    group.push(finding);
    groups.set(finding.code, group);
  }
  return [...groups.entries()].sort((left, right) => {
    const severityRank = { error: 0, warning: 1, info: 2 };
    const severityOrder = severityRank[left[1][0].severity] - severityRank[right[1][0].severity];
    return severityOrder || right[1].length - left[1].length || left[0].localeCompare(right[0]);
  });
}

function findingLocation(finding: OkfFinding) {
  const line = finding.range?.startLine;
  const column = finding.range?.startColumn;
  if (!line) return finding.relativePath;
  return `${finding.relativePath}:${line}${column ? `:${column}` : ""}`;
}

export function buildHealthAgentPrompt(
  locationName: string,
  documents: number,
  scope: HealthScope,
  findings: OkfFinding[],
) {
  const summary = summarizeHealth(findings);
  const scopeLabel = scope === "policy"
    ? "repository policy (.constructignore applied)"
    : "all Markdown (.constructignore bypassed)";
  const findingBlocks = findings.map((finding) => [
    `<violation code="${finding.code}" severity="${finding.severity}" location="${findingLocation(finding)}">`,
    finding.message,
    "</violation>",
  ].join("\n")).join("\n\n");

  return [
    `Review and fix the OKF findings in the "${locationName}" bundle.`,
    "",
    "Preserve the author's content and all unknown frontmatter fields. Do not invent metadata values or impose a closed type taxonomy. Inspect each document before choosing a missing type. Do not rewrite the bundle automatically beyond the findings you verify.",
    "",
    `<okf-lint scope="${scopeLabel}" documents="${documents}" errors="${summary.errors}" warnings="${summary.warnings}" info="${summary.info}">`,
    findingBlocks || "No findings in this scope.",
    "</okf-lint>",
    "",
    "After making changes, run the OKF linter again and report the remaining findings.",
  ].join("\n");
}
