import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const config = JSON.parse(
  await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);

test("uses the public Construct identity", () => {
  assert.equal(config.productName, "Construct");
  assert.equal(config.identifier, "com.luisnovo.construct");
  assert.equal(config.app.windows[0].title, "Construct");
});

test("bundles native application icons", () => {
  assert.ok(config.bundle.icon.includes("icons/icon.icns"));
  assert.ok(config.bundle.icon.includes("icons/icon.ico"));
  assert.ok(config.bundle.icon.includes("icons/128x128@2x.png"));
});
