import { spawnSync } from "node:child_process";
import { appendFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const VALID_THRESHOLDS = new Set(["error", "warning", "never"]);

function escapeCommand(value) {
  return String(value)
    .replaceAll("%", "%25")
    .replaceAll("\r", "%0D")
    .replaceAll("\n", "%0A")
    .replaceAll(":", "%3A")
    .replaceAll(",", "%2C");
}

function escapeMarkdown(value) {
  return String(value).replaceAll("|", "\\|").replaceAll("\n", " ");
}

function annotationPath(root, relativePath, workspace) {
  const absolute = path.resolve(workspace, root, relativePath);
  const relative = path.relative(workspace, absolute);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) return null;
  return relative.split(path.sep).join("/");
}

export function validateReport(report) {
  if (!report || report.schemaVersion !== 1 || !Array.isArray(report.findings)) {
    throw new Error(
      `Unsupported Construct lint JSON schema: ${report?.schemaVersion ?? "missing"}.`,
    );
  }
  return report;
}

export function renderAnnotation(finding, { root = ".", workspace = "." } = {}) {
  const file = annotationPath(root, finding.relativePath, workspace);
  const severity = finding.severity === "error"
    ? "error"
    : finding.severity === "warning"
      ? "warning"
      : "notice";
  const properties = [];
  if (file) properties.push(`file=${escapeCommand(file)}`);
  if (finding.range?.start?.line) {
    properties.push(`line=${finding.range.start.line}`);
    properties.push(`col=${finding.range.start.column || 1}`);
    if (finding.range.end?.line) properties.push(`endLine=${finding.range.end.line}`);
    if (finding.range.end?.column) properties.push(`endColumn=${finding.range.end.column}`);
  }
  properties.push(`title=${escapeCommand(finding.code)}`);
  return `::${severity} ${properties.join(",")}::${escapeCommand(finding.message)}`;
}

export function renderSummary(report, threshold) {
  const summary = report.summary;
  const rows = report.findings
    .map((finding) => {
      const location = finding.range?.start?.line
        ? `${finding.relativePath}:${finding.range.start.line}`
        : finding.relativePath;
      return `| ${escapeMarkdown(finding.severity)} | \`${escapeMarkdown(finding.code)}\` | \`${escapeMarkdown(location)}\` | ${escapeMarkdown(finding.message)} |`;
    })
    .join("\n");
  return [
    "## Construct OKF lint",
    "",
    `**${summary.documents} documents · ${summary.errors} errors · ${summary.warnings} warnings · ${summary.info} info**`,
    "",
    `Failure threshold: \`${threshold}\`${summary.truncated ? " · findings truncated by the CLI output limit" : ""}`,
    "",
    ...(rows
      ? [
        "| Severity | Code | Location | Finding |",
        "| --- | --- | --- | --- |",
        rows,
        "",
      ]
      : ["No findings.", ""]),
  ].join("\n");
}

async function main() {
  const binary = process.env.CONSTRUCT_BINARY;
  const lintPath = process.env.CONSTRUCT_LINT_PATH || ".";
  const threshold = process.env.CONSTRUCT_FAIL_ON || "error";
  const annotations = (process.env.CONSTRUCT_ANNOTATIONS || "true").toLowerCase();
  if (!binary) throw new Error("The verified Construct CLI path is unavailable.");
  if (!VALID_THRESHOLDS.has(threshold)) {
    throw new Error(`Invalid fail-on value "${threshold}".`);
  }
  if (!["true", "false"].includes(annotations)) {
    throw new Error(`Invalid annotations value "${annotations}".`);
  }

  const result = spawnSync(
    binary,
    ["okf", "lint", lintPath, "--format", "json", "--fail-on", threshold, "--no-color"],
    { encoding: "utf8" },
  );
  if (result.error) throw result.error;
  if (result.stderr) process.stderr.write(result.stderr);

  let report;
  try {
    report = validateReport(JSON.parse(result.stdout));
  } catch (error) {
    if (result.stdout) process.stdout.write(result.stdout);
    throw error;
  }

  if (annotations === "true") {
    const workspace = process.env.GITHUB_WORKSPACE || process.cwd();
    for (const finding of report.findings) {
      process.stdout.write(`${renderAnnotation(finding, { root: lintPath, workspace })}\n`);
    }
  }
  if (process.env.GITHUB_STEP_SUMMARY) {
    await appendFile(
      process.env.GITHUB_STEP_SUMMARY,
      `${renderSummary(report, threshold)}\n`,
    );
  }
  process.stdout.write(
    `Construct OKF lint: ${report.summary.errors} errors, ${report.summary.warnings} warnings, ${report.summary.info} info.\n`,
  );
  process.exitCode = Number.isInteger(result.status) ? result.status : 2;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`Construct OKF lint: ${error.message}\n`);
    process.exitCode = 2;
  });
}
