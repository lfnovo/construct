import { existsSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

const channel = process.argv[2] || "dev";
if (!new Set(["dev", "release"]).has(channel)) throw new Error("Channel must be `dev` or `release`.");
const bundle = resolve(process.argv[3] || (
  channel === "release"
    ? "src-tauri/target/release/bundle/macos/Construct.app"
    : "src-tauri/target/release/bundle/macos/Construct Dev.app"
));
const expected = channel === "release"
  ? { productName: "Construct", identifier: "com.luisnovo.construct", cli: "construct" }
  : { productName: "Construct Dev", identifier: "com.luisnovo.construct.dev", cli: "construct-dev" };

const binary = process.argv[4] ? resolve(process.argv[4]) : null;
let binaryIdentity = null;
if (binary) {
  binaryIdentity = JSON.parse(execFileSync(binary, ["identity"], { encoding: "utf8" }));
  const actual = {
    channel: binaryIdentity.channel,
    productName: binaryIdentity.productName,
    identifier: binaryIdentity.bundleIdentifier,
    cli: binaryIdentity.cliCommand,
  };
  if (actual.channel !== channel) throw new Error(`channel is ${JSON.stringify(actual.channel)}; expected ${JSON.stringify(channel)}.`);
  for (const [key, value] of Object.entries(actual)) {
    if (key === "channel") continue;
    if (value !== expected[key]) throw new Error(`${key} is ${JSON.stringify(value)}; expected ${JSON.stringify(expected[key])}.`);
  }
}

if (!existsSync(bundle)) throw new Error(`Expected ${channel} bundle at ${bundle}.`);
const infoPath = resolve(bundle, "Contents/Info.plist");
const info = readFileSync(infoPath, "utf8");
function plistValue(key) {
  const match = info.match(new RegExp(`<key>${key}</key>\\s*<string>([^<]+)</string>`));
  return match?.[1] || null;
}
const actual = {
  productName: plistValue("CFBundleName") || plistValue("CFBundleDisplayName"),
  identifier: plistValue("CFBundleIdentifier"),
};
for (const [key, value] of Object.entries(actual)) {
  if (value !== expected[key]) throw new Error(`${key} is ${JSON.stringify(value)}; expected ${JSON.stringify(expected[key])}.`);
}
console.log(JSON.stringify({ channel, bundle, binary, ...actual, cliCommand: expected.cli, ...(binaryIdentity ? { binaryIdentity } : {}) }, null, 2));
