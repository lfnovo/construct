import { splitMarkdownDocument } from "./markdownDocument.ts";
import { splitReviewDocument } from "./review.ts";

export type OkfValue =
  | { kind: "null" }
  | { kind: "boolean"; value: boolean }
  | { kind: "integer"; value: string }
  | { kind: "unsignedInteger"; value: string }
  | { kind: "float"; value: string }
  | { kind: "string"; value: string }
  | { kind: "sequence"; items: OkfValue[] }
  | { kind: "mapping"; entries: Array<{ key: OkfValue; value: OkfValue }> }
  | { kind: "tagged"; tag: string; value: OkfValue };

export type OkfNamedValue = {
  name: string;
  value: OkfValue;
};

export type OkfMetadata = {
  type?: string;
  title?: string;
  description?: string;
  resource?: string;
  tags: string[];
  timestamp?: string;
  effectiveTimestamp?: string;
  okfVersion?: string;
  status?: string;
  staleAfter?: string;
  sources?: OkfValue;
  generated?: OkfValue;
  verified?: OkfValue;
  extra: OkfNamedValue[];
  raw?: OkfValue;
};

export type OkfFinding = {
  code: string;
  severity: "error" | "warning" | "info";
  message: string;
  relativePath: string;
  range?: {
    startLine: number;
    startColumn: number;
    endLine: number;
    endColumn: number;
  };
};

export type OkfLink = {
  target: string;
  fragment?: string;
  resolvedPath?: string;
  origin: "markdown" | "metadata";
  field?: string;
  status: "candidate" | "resolved" | "unresolved" | "external" | "fragment" | "outsideBundle";
  range?: OkfFinding["range"];
};

export type OkfInspection = {
  kind: "concept" | "index" | "log";
  relativePath: string;
  hasFrontmatter: boolean;
  metadata: OkfMetadata;
  links: OkfLink[];
  findings: OkfFinding[];
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

export type OkfBundleSnapshot = {
  detected: boolean;
  declaredVersion?: string;
  documentCount: number;
  findingCount: number;
  findings: OkfFinding[];
  concepts: OkfConcept[];
};

export type OkfBundleIndex = {
  status: "scanning" | "ready" | "error";
  concepts: OkfConcept[];
  signature: string;
  declaredVersion?: string;
  documentCount?: number;
  findingCount?: number;
  findings?: OkfFinding[];
  error?: string;
};

function okfValueToPlain(value: OkfValue): unknown {
  switch (value.kind) {
    case "null":
      return null;
    case "boolean":
    case "string":
      return value.value;
    case "integer":
    case "unsignedInteger": {
      const number = Number(value.value);
      return Number.isSafeInteger(number) ? number : value.value;
    }
    case "float": {
      const number = Number(value.value);
      return Number.isFinite(number) ? number : value.value;
    }
    case "sequence":
      return value.items.map(okfValueToPlain);
    case "mapping":
      return Object.fromEntries(value.entries.map((entry) => [
        String(okfValueToPlain(entry.key)),
        okfValueToPlain(entry.value),
      ]));
    case "tagged":
      return { tag: value.tag, value: okfValueToPlain(value.value) };
  }
}

export function formatOkfValue(value: OkfValue) {
  const plain = okfValueToPlain(value);
  if (typeof plain === "string") return plain;
  return JSON.stringify(plain);
}

/**
 * Preview navigation still resolves local links in the webview. OKF inspection,
 * normalization, findings, and graph inputs are authoritative in the Rust core.
 */
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
