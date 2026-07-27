import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";

const ROOT_DOCUMENTS = [
  "CHANGELOG.md",
  "CONTRIBUTING.md",
  "README.md",
  "SECURITY.md",
];

function walkMarkdown(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) return walkMarkdown(path);
      return extname(entry.name).toLowerCase() === ".md" ? [path] : [];
    });
}

function linkTargets(markdown) {
  const inline = [...markdown.matchAll(/!?\[[^\]]*]\(([^)]+)\)/g)]
    .map((match) => match[1]);
  const references = [...markdown.matchAll(/^\s*\[[^\]]+]:\s*(\S+)/gm)]
    .map((match) => match[1]);
  return [...inline, ...references];
}

function cleanTarget(rawTarget) {
  const trimmed = rawTarget.trim();
  const target = trimmed.startsWith("<")
    ? trimmed.slice(1, trimmed.indexOf(">"))
    : trimmed.split(/\s+["'(]/, 1)[0];
  return target.split("#", 1)[0];
}

function isExternal(target) {
  return !target
    || target.startsWith("#")
    || /^(?:https?:|mailto:|obsidian:|tauri:)/.test(target);
}

function checkLocalLinks(files) {
  const failures = [];
  for (const file of files) {
    const markdown = readFileSync(file, "utf8");
    for (const rawTarget of linkTargets(markdown)) {
      const target = cleanTarget(rawTarget);
      if (isExternal(target)) continue;
      let decoded;
      try {
        decoded = decodeURIComponent(target);
      } catch {
        failures.push(`${file}: invalid encoded link ${rawTarget}`);
        continue;
      }
      const destination = resolve(dirname(file), decoded);
      if (!existsSync(destination)) {
        failures.push(`${file}: missing local link ${rawTarget}`);
      }
    }
  }
  return failures;
}

function checkDocumentationIndex() {
  const index = readFileSync("docs/README.md", "utf8");
  return walkMarkdown("docs")
    .filter((file) => dirname(file) === "docs" && file !== "docs/README.md")
    .filter((file) => !index.includes(`(${relative("docs", file)})`))
    .map((file) => `docs/README.md: missing top-level document ${file}`);
}

const files = [
  ...ROOT_DOCUMENTS.filter(existsSync),
  ...walkMarkdown("docs"),
];
const failures = [
  ...checkLocalLinks(files),
  ...checkDocumentationIndex(),
];

if (failures.length) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    `Documentation checks passed for ${files.length} Markdown files.`,
  );
}
