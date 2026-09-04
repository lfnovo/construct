import { spawn } from "node:child_process";
import { resolve } from "node:path";

const [channel = "", command = ""] = process.argv.slice(2);
if (!new Set(["dev", "release"]).has(channel) || !new Set(["dev", "build"]).has(command)) {
  console.error("Usage: node scripts/build-desktop.mjs <dev|release> <dev|build>");
  process.exit(2);
}
if (command === "dev" && channel !== "dev") {
  console.error("The development server must use the dev channel.");
  process.exit(2);
}

const executable = process.platform === "win32" ? "tauri.cmd" : "tauri";
const config = resolve(`src-tauri/tauri.${channel}.conf.json`);
const args = [command, "--config", config];
if (command === "build") args.push("--bundles", "app");

const child = spawn(executable, args, {
  env: { ...process.env, CONSTRUCT_CHANNEL: channel },
  stdio: "inherit",
});
child.on("error", (error) => {
  console.error(`Could not start the Tauri ${command} command: ${error.message}`);
  process.exitCode = 1;
});
child.on("exit", (code, signal) => {
  if (signal) process.exitCode = 1;
  else process.exitCode = code ?? 1;
});
