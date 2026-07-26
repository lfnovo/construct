import { splitMarkdownDocument } from "./markdownDocument.ts";
import { splitReviewDocument } from "./review.ts";

export type OkfMetadata = {
  type?: string;
  title?: string;
  description?: string;
  resource?: string;
  tags: string[];
  timestamp?: string;
  okfVersion?: string;
  extra: Record<string, string | string[]>;
};

export type OkfInspection = {
  kind: "concept" | "index" | "log";
  metadata: OkfMetadata;
  issues: Array<{ level: "error" | "warning"; message: string }>;
  isConformant: boolean;
};

export type OkfConcept = {
  id: string;
  path: string;
  relativePath: string;
  type: string;
  title: string;
  description?: string;
  tags: string[];
  timestamp?: string;
  outgoingPaths: string[];
  incomingPaths: string[];
};

export type OkfBundleIndex = {
  status: "scanning" | "ready" | "error";
  concepts: OkfConcept[];
  signature: string;
  error?: string;
};

type FrontmatterResult = { hasFrontmatter: boolean; data: Record<string, string | string[]>; error?: string };

const emptyMetadata = (): OkfMetadata => ({ tags: [], extra: {} });

function unquote(value: string) {
  if (value.length < 2 || !['"', "'"].includes(value[0])) return value.trim();
  if (value.at(-1) !== value[0]) throw new Error("A quoted value is not closed.");
  if (value[0] === '"') {
    try { return JSON.parse(value); } catch { throw new Error("A quoted value is invalid."); }
  }
  return value.slice(1, -1).replace(/''/g, "'");
}

function splitInlineList(value: string) {
  if (!value.startsWith("[") || !value.endsWith("]")) return [unquote(value)];
  const inner = value.slice(1, -1).trim();
  if (!inner) return [];
  const values: string[] = [];
  let current = "";
  let quote = "";
  for (const character of inner) {
    if ((character === '"' || character === "'") && (!quote || quote === character)) quote = quote ? "" : character;
    if (character === "," && !quote) { values.push(unquote(current.trim())); current = ""; }
    else current += character;
  }
  if (quote) throw new Error("A list value is not closed.");
  values.push(unquote(current.trim()));
  return values;
}

function parseFrontmatter(content: string): FrontmatterResult {
  const start = content.match(/^---\r?\n/);
  if (!start) return { hasFrontmatter: false, data: {} };
  const closing = content.slice(start[0].length).search(/\r?\n---\s*(?:\r?\n|$)/);
  if (closing < 0) return { hasFrontmatter: true, data: {}, error: "The YAML frontmatter block is not closed." };
  const source = content.slice(start[0].length, start[0].length + closing).replace(/\r/g, "");
  const data: Record<string, string | string[]> = {};
  let listKey: string | null = null;
  let blockKey: string | null = null;
  let blockIndent = 0;
  try {
    for (const line of source.split("\n")) {
      if (!line.trim() || line.trimStart().startsWith("#")) continue;
      const listItem = line.match(/^\s+-\s+(.+)$/);
      if (listItem && listKey) {
        const existing = data[listKey];
        data[listKey] = [...(Array.isArray(existing) ? existing : []), unquote(listItem[1].trim())];
        continue;
      }
      if (blockKey && line.match(/^\s+/) && line.search(/\S/) >= blockIndent) {
        data[blockKey] = `${data[blockKey] || ""}${data[blockKey] ? "\n" : ""}${line.trim()}`;
        continue;
      }
      blockKey = null;
      const field = line.match(/^([A-Za-z][A-Za-z0-9_-]*):(?:\s*(.*))?$/);
      if (!field) throw new Error(`Cannot read frontmatter line: ${line}`);
      const [, key, rawValue = ""] = field;
      const value = rawValue.trim();
      listKey = null;
      if (!value) { data[key] = []; listKey = key; continue; }
      if (/^[|>][+-]?$/.test(value)) { data[key] = ""; blockKey = key; blockIndent = line.search(/\S/) + 2; continue; }
      data[key] = value.startsWith("[") ? splitInlineList(value) : unquote(value);
    }
    return { hasFrontmatter: true, data };
  } catch (error) {
    return { hasFrontmatter: true, data, error: error instanceof Error ? error.message : String(error) };
  }
}

export function inspectOkfDocument(content: string, relativePath: string, isBundleRoot = false): OkfInspection {
  const filename = relativePath.split(/[\\/]/).at(-1)?.toLowerCase() || "";
  const kind = filename === "index.md" ? "index" : filename === "log.md" ? "log" : "concept";
  const frontmatter = parseFrontmatter(content);
  const metadata = emptyMetadata();
  for (const [key, value] of Object.entries(frontmatter.data)) {
    if (key === "type" && typeof value === "string") metadata.type = value;
    else if (key === "title" && typeof value === "string") metadata.title = value;
    else if (key === "description" && typeof value === "string") metadata.description = value;
    else if (key === "resource" && typeof value === "string") metadata.resource = value;
    else if (key === "tags") metadata.tags = Array.isArray(value) ? value : value ? [value] : [];
    else if (key === "timestamp" && typeof value === "string") metadata.timestamp = value;
    else if (key === "okf_version" && typeof value === "string") metadata.okfVersion = value;
    else metadata.extra[key] = value;
  }
  const issues: OkfInspection["issues"] = [];
  if (frontmatter.error) issues.push({ level: "error", message: frontmatter.error });
  if (kind === "concept") {
    if (!frontmatter.hasFrontmatter) issues.push({ level: "error", message: "Concept documents need YAML frontmatter." });
    else if (!metadata.type?.trim()) issues.push({ level: "error", message: "The required type field is missing." });
  } else if (kind === "log") {
    if (frontmatter.hasFrontmatter) issues.push({ level: "warning", message: "log.md normally has no frontmatter." });
    if (!/^##\s+\d{4}-\d{2}-\d{2}\s*$/m.test(content)) issues.push({ level: "warning", message: "Use ISO date headings (YYYY-MM-DD) for log entries." });
  } else {
    if (frontmatter.hasFrontmatter && !(isBundleRoot && metadata.okfVersion && Object.keys(frontmatter.data).every((key) => key === "okf_version"))) {
      issues.push({ level: "warning", message: "index.md normally has no frontmatter; a root index may declare only okf_version." });
    }
  }
  return { kind, metadata, issues, isConformant: issues.every((issue) => issue.level !== "error") };
}

export function resolveOkfLink(sourcePath: string, bundleRoot: string | undefined, target: string) {
  const clean = target.split("#")[0];
  if (!clean) return sourcePath;
  if (bundleRoot && clean.startsWith("/")) return `${bundleRoot.replace(/[\\/]$/, "")}/${clean.slice(1)}`;
  try { return decodeURIComponent(new URL(clean, `file://${sourcePath}`).pathname); } catch { return clean; }
}

export function withoutFrontmatter(content: string) {
  const review = splitReviewDocument(content);
  if (!review.error) return review.body;
  const parts = splitMarkdownDocument(content);
  return parts.error ? content : parts.body;
}

export function extractOkfLinks(content: string, sourcePath: string, bundleRoot: string) {
  const links = new Set<string>();
  const body = withoutFrontmatter(content);
  for (const match of body.matchAll(/(?<!!)\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)/g)) {
    const target = match[1];
    if (target.startsWith("#") || /^(https?:|mailto:|tel:|data:)/i.test(target)) continue;
    const resolved = resolveOkfLink(sourcePath, bundleRoot, target);
    if (/\.md$/i.test(resolved)) links.add(resolved);
  }
  return [...links];
}
