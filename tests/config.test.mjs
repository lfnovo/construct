import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const config = JSON.parse(
  await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
const devConfig = JSON.parse(
  await readFile(new URL("../src-tauri/tauri.dev.conf.json", import.meta.url), "utf8"),
);
const releaseConfig = JSON.parse(
  await readFile(new URL("../src-tauri/tauri.release.conf.json", import.meta.url), "utf8"),
);

test("defaults every direct Tauri build to the development identity", () => {
  assert.equal(config.productName, "Construct Dev");
  assert.equal(config.identifier, "com.luisnovo.construct.dev");
  assert.equal(config.app.windows[0].title, "Construct Dev");
  assert.deepEqual(devConfig, {
    $schema: "https://schema.tauri.app/config/2",
    productName: "Construct Dev",
    identifier: "com.luisnovo.construct.dev",
    app: { windows: [{ title: "Construct Dev" }] },
  });
});

test("keeps the released identity explicit and unchanged", () => {
  assert.equal(releaseConfig.productName, "Construct");
  assert.equal(releaseConfig.identifier, "com.luisnovo.construct");
  assert.equal(releaseConfig.app.windows[0].title, "Construct");
});

test("bundles native application icons", () => {
  assert.ok(config.bundle.icon.includes("icons/icon.icns"));
  assert.ok(config.bundle.icon.includes("icons/icon.ico"));
  assert.ok(config.bundle.icon.includes("icons/128x128@2x.png"));
});
