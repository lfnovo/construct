import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { chmod, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const REPOSITORY = "lfnovo/construct";

export function checksumForArchive(manifest, archive) {
  for (const line of manifest.split(/\r?\n/)) {
    const match = line.match(/^([0-9a-fA-F]{64}) [ *](.+)$/);
    if (match?.[2] === archive) return match[1].toLowerCase();
  }
  throw new Error(`${archive} is missing from SHA256SUMS.`);
}

async function download(url) {
  const response = await fetch(url, {
    headers: { "user-agent": "construct-okf-lint-action" },
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`Download failed (${response.status}) for ${url}.`);
  }
  return Buffer.from(await response.arrayBuffer());
}

async function sha256(file) {
  const hash = createHash("sha256");
  hash.update(await readFile(file));
  return hash.digest("hex");
}

function extract(archivePath, cacheDir) {
  const arguments_ = archivePath.endsWith(".tar.gz")
    ? ["-xzf", archivePath, "-C", cacheDir]
    : ["-xf", archivePath, "-C", cacheDir];
  const result = spawnSync("tar", arguments_, {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(
      `Could not extract ${path.basename(archivePath)}: ${result.stderr.trim()}`,
    );
  }
}

async function main() {
  const archive = process.env.CONSTRUCT_ARCHIVE;
  const binary = process.env.CONSTRUCT_BINARY;
  const cacheDir = process.env.CONSTRUCT_CACHE_DIR;
  const releaseTag = process.env.CONSTRUCT_RELEASE_TAG;
  if (!archive || !binary || !cacheDir || !releaseTag) {
    throw new Error("The action did not resolve a complete CLI artifact.");
  }

  await mkdir(cacheDir, { recursive: true });
  const baseUrl = `https://github.com/${REPOSITORY}/releases/download/${releaseTag}`;
  const manifest = (await download(`${baseUrl}/SHA256SUMS`)).toString("utf8");
  const expected = checksumForArchive(manifest, archive);
  const archivePath = path.join(cacheDir, archive);

  let actual;
  try {
    actual = await sha256(archivePath);
  } catch {
    actual = "";
  }
  if (actual !== expected) {
    await rm(archivePath, { force: true });
    await writeFile(archivePath, await download(`${baseUrl}/${archive}`));
    actual = await sha256(archivePath);
  }
  if (actual !== expected) {
    throw new Error(
      `Checksum mismatch for ${archive}: expected ${expected}, received ${actual}.`,
    );
  }

  await rm(binary, { force: true });
  extract(archivePath, cacheDir);
  if (process.platform !== "win32") await chmod(binary, 0o755);
  process.stdout.write(`Installed verified Construct CLI ${releaseTag}.\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`Construct OKF lint: ${error.message}\n`);
    process.exitCode = 2;
  });
}
