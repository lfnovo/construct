import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const ROOT_DOCUMENTS = new Set([
  "AGENTS.md",
  "CHANGELOG.md",
  "CONTRIBUTING.md",
  "LICENSE",
  "README.md",
  "SECURITY.md",
]);

const WEB_CONFIG = new Set([
  "eslint.config.js",
  "index.html",
  "package-lock.json",
  "package.json",
  "tsconfig.json",
  "vite.config.ts",
]);

const BUNDLE_CONFIG = new Set([
  "package-lock.json",
  "package.json",
  "src-tauri/Cargo.lock",
  "src-tauri/Cargo.toml",
  "src-tauri/build.rs",
  "src-tauri/tauri.conf.json",
]);

const FORCE_ALL = new Set([
  ".github/workflows/ci.yml",
  ".github/workflows/release.yml",
  "scripts/classify-ci-changes.mjs",
  "tests/ci-paths.test.mjs",
]);

function normalizePath(file) {
  return file.replaceAll("\\", "/").replace(/^\.\//, "");
}

function startsWith(file, prefix) {
  return file === prefix || file.startsWith(`${prefix}/`);
}

export function classifyCiPaths(inputPaths) {
  const paths = [...new Set(inputPaths.map(normalizePath).filter(Boolean))].sort();
  const result = {
    docs: false,
    web: false,
    rust: false,
    bundle: false,
  };

  for (const file of paths) {
    if (FORCE_ALL.has(file)) {
      result.docs = true;
      result.web = true;
      result.rust = true;
      result.bundle = true;
      continue;
    }

    const docs = ROOT_DOCUMENTS.has(file)
      || startsWith(file, "docs")
      || startsWith(file, ".github/ISSUE_TEMPLATE")
      || file === ".github/pull_request_template.md"
      || file === ".github/dependabot.yml"
      || file === "scripts/check-docs.mjs";
    const fixtures = startsWith(file, "tests/fixtures");
    const web = startsWith(file, "src")
      || fixtures
      || /^tests\/[^/]+\.test\.mjs$/.test(file)
      || WEB_CONFIG.has(file)
      || file === "scripts/check-release-version.mjs";
    const rust = startsWith(file, "src-tauri")
      || fixtures
      || file === "rust-toolchain.toml"
      || file === "scripts/mcp-smoke.mjs";
    const bundle = BUNDLE_CONFIG.has(file)
      || startsWith(file, "src-tauri/capabilities")
      || startsWith(file, "src-tauri/icons")
      || file === "src-tauri/app-icon.svg";

    result.docs ||= docs;
    result.web ||= web;
    result.rust ||= rust;
    result.bundle ||= bundle;

    if (!docs && !web && !rust && !bundle) {
      // Unknown project files take the safe fast path instead of silently
      // bypassing validation. The expensive bundle remains opt-in.
      result.web = true;
      result.rust = true;
    }
  }

  return {
    ...result,
    code: result.web || result.rust || result.bundle,
    paths,
  };
}

function changedPaths(base, head) {
  const output = execFileSync(
    "git",
    ["diff", "--name-only", "-z", base, head],
    { encoding: "utf8" },
  );
  return output.split("\0").filter(Boolean);
}

function parseArguments(arguments_) {
  const options = {
    all: false,
    githubOutput: false,
    base: "",
    head: "",
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--all") options.all = true;
    else if (argument === "--github-output") options.githubOutput = true;
    else if (argument === "--base") options.base = arguments_[index += 1] || "";
    else if (argument === "--head") options.head = arguments_[index += 1] || "";
    else throw new Error(`Unknown argument: ${argument}`);
  }
  if (!options.all && (!options.base || !options.head)) {
    throw new Error("Supply --all or both --base and --head.");
  }
  return options;
}

function printGithubOutput(result) {
  for (const key of ["docs", "web", "rust", "bundle", "code"]) {
    process.stdout.write(`${key}=${result[key]}\n`);
  }
  process.stdout.write(`changed_count=${result.paths.length}\n`);
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const result = options.all
    ? {
      docs: true,
      web: true,
      rust: true,
      bundle: true,
      code: true,
      paths: [],
    }
    : classifyCiPaths(changedPaths(options.base, options.head));
  if (options.githubOutput) printGithubOutput(result);
  else process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
