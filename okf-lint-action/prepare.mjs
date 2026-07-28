import process from "node:process";
import { pathToFileURL } from "node:url";
import path from "node:path";

const TARGETS = new Map([
  ["Linux:X64", { target: "x86_64-unknown-linux-gnu", extension: "tar.gz" }],
  ["macOS:X64", { target: "x86_64-apple-darwin", extension: "tar.gz" }],
  ["macOS:ARM64", { target: "aarch64-apple-darwin", extension: "tar.gz" }],
  ["Windows:X64", { target: "x86_64-pc-windows-msvc", extension: "zip" }],
]);

export function normalizeVersion(input) {
  const version = input.trim().replace(/^v/, "");
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(
      `Invalid Construct version "${input}". Pin an immutable semantic version such as 0.1.5.`,
    );
  }
  return version;
}

export function resolveArtifact({ os, arch, version, toolCache }) {
  const target = TARGETS.get(`${os}:${arch}`);
  if (!target) {
    throw new Error(
      `Construct OKF lint does not publish a CLI for ${os} ${arch}.`,
    );
  }
  const normalizedVersion = normalizeVersion(version);
  const archive = `construct_${normalizedVersion}_${target.target}.${target.extension}`;
  const cacheDir = path.join(
    toolCache,
    "construct-okf-lint",
    normalizedVersion,
    target.target,
  );
  const binaryName = os === "Windows" ? "construct.exe" : "construct";
  return {
    archive,
    binary: path.join(cacheDir, binaryName),
    cacheDir,
    releaseTag: `v${normalizedVersion}`,
    version: normalizedVersion,
  };
}

async function main() {
  if (!process.env.GITHUB_OUTPUT) {
    throw new Error("GITHUB_OUTPUT is unavailable.");
  }
  const result = resolveArtifact({
    os: process.env.RUNNER_OS ?? "",
    arch: process.env.RUNNER_ARCH ?? "",
    version: process.env.CONSTRUCT_VERSION ?? "",
    toolCache: process.env.RUNNER_TOOL_CACHE ?? "",
  });
  const fs = await import("node:fs/promises");
  await fs.appendFile(
    process.env.GITHUB_OUTPUT,
    Object.entries(result)
      .map(([key, value]) => {
        const outputKey = key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
        return `${outputKey}=${value}\n`;
      })
      .join(""),
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`Construct OKF lint: ${error.message}\n`);
    process.exitCode = 2;
  });
}
