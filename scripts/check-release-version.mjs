import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function cargoPackageVersion(contents) {
  const packageStart = contents.indexOf("[package]");
  if (packageStart < 0) return null;
  const afterHeader = contents.slice(packageStart + "[package]".length);
  const nextSection = afterHeader.search(/\n\[/);
  const packageSection = nextSection < 0
    ? afterHeader
    : afterHeader.slice(0, nextSection);
  return packageSection.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1] || null;
}

export function projectVersions(root = process.cwd()) {
  const packageJson = readJson(`${root}/package.json`);
  const packageLock = readJson(`${root}/package-lock.json`);
  const tauriConfig = readJson(`${root}/src-tauri/tauri.conf.json`);
  const cargoToml = readFileSync(`${root}/src-tauri/Cargo.toml`, "utf8");
  return {
    "package.json": packageJson.version,
    "package-lock.json": packageLock.version,
    "package-lock.json root package": packageLock.packages?.[""]?.version,
    "src-tauri/Cargo.toml": cargoPackageVersion(cargoToml),
    "src-tauri/tauri.conf.json": tauriConfig.version,
  };
}

export function releaseVersionErrors(tag, versions) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(tag || "")) {
    return [`Release tag "${tag || ""}" must use v<major>.<minor>.<patch>.`];
  }
  const expected = tag.slice(1);
  return Object.entries(versions)
    .filter(([, version]) => version !== expected)
    .map(([source, version]) => `${source} has version "${version ?? "missing"}"; expected "${expected}".`);
}

export function checkReleaseVersion(tag, root = process.cwd()) {
  const versions = projectVersions(root);
  return {
    versions,
    errors: releaseVersionErrors(tag, versions),
  };
}

function run() {
  const tag = process.argv[2] || process.env.RELEASE_TAG;
  const result = checkReleaseVersion(tag);
  if (result.errors.length) {
    for (const error of result.errors) console.error(`Release version error: ${error}`);
    process.exitCode = 1;
    return;
  }
  console.log(`Release ${tag} matches every project version source.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) run();
