import assert from "node:assert/strict";
import test from "node:test";

import { checksumForArchive } from "../okf-lint-action/install.mjs";
import { normalizeVersion, resolveArtifact } from "../okf-lint-action/prepare.mjs";
import {
  renderAnnotation,
  renderSummary,
  validateReport,
} from "../okf-lint-action/run.mjs";

test("action resolves immutable artifacts for every published CLI platform", () => {
  assert.deepEqual(
    resolveArtifact({
      os: "Linux",
      arch: "X64",
      version: "v0.1.5",
      toolCache: "/tools",
    }),
    {
      archive: "construct_0.1.5_x86_64-unknown-linux-gnu.tar.gz",
      binary: "/tools/construct-okf-lint/0.1.5/x86_64-unknown-linux-gnu/construct",
      cacheDir: "/tools/construct-okf-lint/0.1.5/x86_64-unknown-linux-gnu",
      releaseTag: "v0.1.5",
      version: "0.1.5",
    },
  );
  assert.match(
    resolveArtifact({
      os: "Windows",
      arch: "X64",
      version: "0.1.5",
      toolCache: "C:\\tools",
    }).archive,
    /x86_64-pc-windows-msvc\.zip$/,
  );
});

test("action rejects mutable or unsupported release coordinates", () => {
  assert.throws(() => normalizeVersion("latest"), /immutable semantic version/);
  assert.throws(
    () => resolveArtifact({
      os: "Linux",
      arch: "ARM64",
      version: "0.1.5",
      toolCache: "/tools",
    }),
    /does not publish a CLI/,
  );
});

test("checksum parser selects the exact archive entry", () => {
  const hash = "a".repeat(64);
  assert.equal(
    checksumForArchive(`${hash}  construct_0.1.5_x86_64-unknown-linux-gnu.tar.gz\n`, "construct_0.1.5_x86_64-unknown-linux-gnu.tar.gz"),
    hash,
  );
  assert.throws(() => checksumForArchive(`${hash}  another.tar.gz\n`, "missing.tar.gz"));
});

test("action rejects unknown CLI JSON schemas", () => {
  assert.throws(
    () => validateReport({ schemaVersion: 2, findings: [] }),
    /Unsupported Construct lint JSON schema/,
  );
});

test("action renders source-aware annotations and a job summary", () => {
  const finding = {
    code: "OKF001",
    severity: "error",
    relativePath: "people/luis.md",
    range: {
      start: { line: 2, column: 1 },
      end: { line: 2, column: 5 },
    },
    message: "Missing type.",
  };
  assert.equal(
    renderAnnotation(finding, { root: "knowledge", workspace: "/repo" }),
    "::error file=knowledge/people/luis.md,line=2,col=1,endLine=2,endColumn=5,title=OKF001::Missing type.",
  );
  const summary = renderSummary({
    summary: {
      documents: 3,
      errors: 1,
      warnings: 0,
      info: 0,
      truncated: false,
    },
    findings: [finding],
  }, "error");
  assert.match(summary, /3 documents · 1 errors/);
  assert.match(summary, /people\/luis\.md:2/);
});
