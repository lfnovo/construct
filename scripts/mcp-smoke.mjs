import { spawn, spawnSync } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const binary = resolve(process.argv[2] || "src-tauri/target/debug/construct");
const root = await mkdtemp(join(tmpdir(), "construct-mcp-smoke-"));
const dataDir = join(root, "data");
const sourceDir = join(root, "source");
await mkdir(dataDir, { recursive: true });
await mkdir(join(sourceDir, "projects"), { recursive: true });
await writeFile(join(sourceDir, "index.md"), "---\nokf_version: 0.2\n---\n# Smoke bundle\n");
await writeFile(join(sourceDir, "alpha.md"), "---\ntype: Project\ntitle: Alpha\ntags: [smoke, retrieval]\n---\n# Alpha\n\nOrbital retrieval. See [Beta](beta.md).\n");
await writeFile(join(sourceDir, "beta.md"), "---\ntype: Person\ntitle: Beta\ntags: [smoke]\n---\n# Beta\n\nSupporting context.\n");
await writeFile(join(sourceDir, "projects", "log.md"), "# Log\n\n## 2026-07-26\n\n- MCP smoke test ready.\n");
await writeFile(
  join(dataDir, "workspace.json"),
  JSON.stringify({
    locations: [{
      id: "smoke-location",
      path: sourceDir,
      name: "Smoke Location",
      available: true,
      okfBundle: true,
    }],
  }),
);

const child = spawn(binary, [
  "mcp",
  "serve",
  "--data-dir",
  dataDir,
  "--allow",
  "smoke-location",
], { stdio: ["pipe", "pipe", "pipe"] });
const lines = createInterface({ input: child.stdout });
const pending = new Map();
let stderr = "";
child.stderr.on("data", (chunk) => { stderr += chunk.toString(); });
lines.on("line", (line) => {
  const message = JSON.parse(line);
  const waiter = pending.get(String(message.id));
  if (waiter) {
    pending.delete(String(message.id));
    waiter.resolve(message);
  }
});

let nextId = 1;
function request(method, params) {
  const id = nextId++;
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  return new Promise((resolvePromise, reject) => {
    const timeout = setTimeout(() => {
      pending.delete(String(id));
      reject(new Error(`Timed out waiting for ${method}. stderr: ${stderr}`));
    }, 20_000);
    pending.set(String(id), {
      resolve(message) {
        clearTimeout(timeout);
        resolvePromise(message);
      },
    });
  });
}

function structured(message) {
  if (message.error) throw new Error(JSON.stringify(message.error));
  if (message.result?.isError) throw new Error(message.result.content?.[0]?.text || "Tool failed");
  return message.result?.structuredContent;
}

try {
  await request("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "construct-smoke", version: "1" },
  });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);

  const tools = await request("tools/list", {});
  if (tools.result.tools.length !== 8) throw new Error("Expected eight MCP tools.");

  const listed = structured(await request("tools/call", {
    name: "construct_list_locations",
    arguments: {},
  }));
  if (listed.locations[0].id !== "smoke-location") throw new Error("Location allowlist failed.");

  const denied = await request("tools/call", {
    name: "construct_get_location_overview",
    arguments: { locationId: "not-allowed" },
  });
  if (
    denied.result?.isError !== true
    || denied.result?.structuredContent?.error?.code !== "location_not_allowed"
  ) {
    throw new Error("Allowlist errors must include a stable structured code.");
  }

  const overview = structured(await request("tools/call", {
    name: "construct_get_location_overview",
    arguments: { locationId: "smoke-location" },
  }));
  if (overview.recentLogs[0].scope !== "projects") throw new Error("Nested OKF log was not found.");

  const search = structured(await request("tools/call", {
    name: "construct_search_knowledge",
    arguments: { locationIds: ["smoke-location"], query: "orbital", limit: 10 },
  }));
  if (search.results[0].relativePath !== "alpha.md") throw new Error("Knowledge search failed.");

  const read = structured(await request("tools/call", {
    name: "construct_read_document",
    arguments: { locationId: "smoke-location", relativePath: "alpha.md" },
  }));
  if (!read.body.includes("Orbital retrieval")) throw new Error("Document read failed.");

  const related = structured(await request("tools/call", {
    name: "construct_get_related_documents",
    arguments: { locationId: "smoke-location", relativePath: "alpha.md" },
  }));
  if (related.documents[0].relativePath !== "beta.md") throw new Error("Related lookup failed.");

  const context = structured(await request("tools/call", {
    name: "construct_build_context_pack",
    arguments: {
      query: "smoke",
      documents: [{ locationId: "smoke-location", relativePath: "alpha.md", reason: "Body match" }],
      maxCharacters: 4_000,
    },
  }));
  if (context.items.length !== 1) throw new Error("Context pack failed.");

  const activity = structured(await request("tools/call", {
    name: "construct_get_location_activity",
    arguments: { locationId: "smoke-location" },
  }));
  const alpha = activity.documents.find((document) => document.relativePath === "alpha.md");
  if (alpha.servedCount !== 1 || alpha.contextCount !== 1) {
    throw new Error("Hot-memory counters were not kept separately.");
  }

  const output = JSON.stringify({ listed, overview, search, read, related, context, activity });
  if (output.includes(sourceDir) || output.includes(dataDir)) {
    throw new Error("An absolute local path leaked into a normal MCP result.");
  }
  process.stdout.write("Construct MCP smoke passed.\n");
} finally {
  child.kill("SIGTERM");
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  spawnSync("pkill", ["-f", `${binary} service --data-dir ${dataDir}`]);
  await rm(root, { recursive: true, force: true });
}
