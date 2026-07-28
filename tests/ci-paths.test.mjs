import assert from "node:assert/strict";
import test from "node:test";

import { classifyCiPaths } from "../scripts/classify-ci-changes.mjs";

test("documentation changes do not compile the application", () => {
  assert.deepEqual(
    classifyCiPaths(["README.md", "docs/user-guide.md"]),
    {
      docs: true,
      web: false,
      rust: false,
      bundle: false,
      code: false,
      paths: ["README.md", "docs/user-guide.md"],
    },
  );
});

test("the standalone documentation checker stays on the lightweight path", () => {
  const result = classifyCiPaths(["scripts/check-docs.mjs"]);
  assert.equal(result.docs, true);
  assert.equal(result.web, false);
  assert.equal(result.rust, false);
  assert.equal(result.bundle, false);
  assert.equal(result.code, false);
});

test("frontend changes run only the web validation on pull requests", () => {
  const result = classifyCiPaths(["src/SearchWorkspace.tsx"]);
  assert.equal(result.web, true);
  assert.equal(result.rust, false);
  assert.equal(result.bundle, false);
  assert.equal(result.code, true);
});

test("GitHub Action changes use JavaScript and lightweight YAML validation", () => {
  assert.deepEqual(
    classifyCiPaths([
      "okf-lint-action/action.yml",
      "okf-lint-action/run.mjs",
    ]),
    {
      docs: true,
      web: true,
      rust: false,
      bundle: false,
      code: true,
      paths: [
        "okf-lint-action/action.yml",
        "okf-lint-action/run.mjs",
      ],
    },
  );
});

test("native source changes run Rust validation without a PR bundle", () => {
  const result = classifyCiPaths(["src-tauri/src/index.rs"]);
  assert.equal(result.web, false);
  assert.equal(result.rust, true);
  assert.equal(result.bundle, false);
  assert.equal(result.code, true);
});

test("bundle configuration changes request the full application build", () => {
  const result = classifyCiPaths(["src-tauri/tauri.conf.json"]);
  assert.equal(result.rust, true);
  assert.equal(result.bundle, true);
});

test("shared OKF fixtures exercise both frontend and Rust consumers", () => {
  const result = classifyCiPaths(["tests/fixtures/okf/v02/index.md"]);
  assert.equal(result.docs, false);
  assert.equal(result.web, true);
  assert.equal(result.rust, true);
  assert.equal(result.bundle, false);
});

test("CI control changes exercise every path once", () => {
  const result = classifyCiPaths([".github/workflows/ci.yml"]);
  assert.equal(result.docs, true);
  assert.equal(result.web, true);
  assert.equal(result.rust, true);
  assert.equal(result.bundle, true);
});

test("unknown files take both fast validation paths", () => {
  const result = classifyCiPaths(["new-project-control-file"]);
  assert.equal(result.docs, false);
  assert.equal(result.web, true);
  assert.equal(result.rust, true);
  assert.equal(result.bundle, false);
});
