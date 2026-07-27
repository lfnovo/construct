import assert from "node:assert/strict";
import test from "node:test";
import {
  buildHealthAgentPrompt,
  filterHealthFindings,
  findingsForScope,
  groupHealthFindings,
  summarizeHealth,
} from "../src/health.ts";

const findings = [
  {
    code: "OKF_FRONTMATTER_REQUIRED",
    severity: "error",
    relativePath: "AGENTS.md",
    message: "Concept documents need YAML frontmatter.",
  },
  {
    code: "OKF_TYPE_REQUIRED",
    severity: "error",
    relativePath: "concepts/customer.md",
    message: "The required type field is missing.",
    range: { startLine: 2, startColumn: 1, endLine: 2, endColumn: 5 },
  },
  {
    code: "OKF_LINK_BROKEN",
    severity: "warning",
    relativePath: "index.md",
    message: "The internal link does not resolve.",
  },
];

test("repository policy excludes only explicitly ignored documents", () => {
  assert.deepEqual(
    findingsForScope(findings, "policy", ["AGENTS.md"]).map((finding) => finding.relativePath),
    ["concepts/customer.md", "index.md"],
  );
  assert.equal(findingsForScope(findings, "policy").length, 3);
  assert.equal(findingsForScope(findings, "all").length, 3);
});

test("summarizes and filters findings without changing their order", () => {
  const scoped = findingsForScope(findings, "policy", ["AGENTS.md"]);
  assert.deepEqual(summarizeHealth(scoped), { errors: 1, warnings: 1, info: 0 });
  assert.deepEqual(
    filterHealthFindings(scoped, "error", "customer").map((finding) => finding.code),
    ["OKF_TYPE_REQUIRED"],
  );
});

test("groups errors before warnings and larger groups before smaller groups", () => {
  const grouped = groupHealthFindings([
    ...findings,
    { ...findings[1], relativePath: "concepts/revenue.md" },
  ]);
  assert.deepEqual(grouped.map(([code]) => code), [
    "OKF_TYPE_REQUIRED",
    "OKF_FRONTMATTER_REQUIRED",
    "OKF_LINK_BROKEN",
  ]);
});

test("serializes a self-contained agent handoff with source locations", () => {
  const prompt = buildHealthAgentPrompt(
    "knowledge",
    12,
    "policy",
    findingsForScope(findings, "policy", ["AGENTS.md"]),
  );
  assert.match(prompt, /Review and fix the OKF findings/);
  assert.match(prompt, /scope="repository policy \(\.constructignore applied\)"/);
  assert.match(prompt, /location="concepts\/customer\.md:2:1"/);
  assert.doesNotMatch(prompt, /location="AGENTS\.md"/);
});
